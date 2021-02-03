package gitdrive

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path"
	"strings"
	"time"

	"github.com/petergardfjall/gitdrive/pkg/executer"
	"github.com/petergardfjall/gitdrive/pkg/notify"
	"github.com/rs/zerolog/log"
)

type Executer interface {
	Execf(fmt string, value ...interface{}) (output string, err error)
}

type SyncerOpts struct {
	Remote string `json:"remote"`
	Branch string `json:"branch"`

	WatchDir string        `json:"watchDir"`
	Once     bool          `json:"once"`
	Interval time.Duration `json:"interval"`

	UseLibNotify        bool          `json:"useLibNotify"`
	NotifyDedupInterval time.Duration `json:"notifyDedupInterval"`
}

func (o SyncerOpts) String() string {
	d, err := json.MarshalIndent(o, "", "  ")
	if err != nil {
		return fmt.Sprintf("%#v", o)
	}
	return string(d)
}

type Syncer struct {
	id string

	executer Executer
	opts     SyncerOpts

	notifier notify.Notifier
}

func NewSyncer(o SyncerOpts) (*Syncer, error) {
	watcherID, err := os.Hostname()
	if err != nil {
		return nil, err
	}
	watcherID += ":" + o.WatchDir

	var notifier notify.Notifier = notify.NewNoOp()
	if o.UseLibNotify {
		notifier = notify.NewLibNotify()
	}
	notifier = notify.NewDedup(notifier, o.NotifyDedupInterval)

	s := &Syncer{
		id:       watcherID,
		executer: executer.NewShell(o.WatchDir),
		opts:     o,
		notifier: notifier,
	}
	log.Debug().Msgf("options:\n%s", o)

	if err := s.validate(); err != nil {
		return nil, err
	}
	return s, nil
}

func (s *Syncer) WithExecuter(e Executer) *Syncer {
	s.executer = e
	return s
}

func (s *Syncer) validate() error {
	o := s.opts
	// watch-dir must be a directory
	fi, err := os.Stat(o.WatchDir)
	if err != nil {
		return err
	}
	if !fi.IsDir() {
		return fmt.Errorf("%s: not a directory", o.WatchDir)
	}

	// git must be on PATH
	if _, err := exec.LookPath("git"); err != nil {
		return err
	}
	// watch-dir must be a git repo
	_, err = os.Stat(path.Join(o.WatchDir, ".git"))
	if err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("%s: not a git repository", o.WatchDir)
		}
		return err
	}

	// remote must exist: .git/refs/remotes/<remote>
	remoteRef := path.Join(o.WatchDir, ".git", "refs", "remotes", o.Remote)
	fi, err = os.Stat(remoteRef)
	if err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("%s: remote does not exist", o.Remote)
		}
		return err
	}

	// branch must exist: .git/refs/heads/<branch>
	branchRef := path.Join(o.WatchDir, ".git", "refs", "heads", o.Branch)
	fi, err = os.Stat(branchRef)
	if err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("%s: branch does not exist", o.Branch)
		}
		return err
	}

	return nil
}

func (s *Syncer) hasConnectivity() bool {
	_, err := s.executer.Execf("git ls-remote --exit-code -h")
	return err == nil
}

func (s *Syncer) Run() error {
	for {
		if err := s.sync(); err != nil {
			log.Err(err).Msg("sync failed")
		}
		s.notifier.Notify(notify.Event("sync"), "finished sync of %s", s.opts.WatchDir)

		if s.opts.Once {
			return nil
		}

		time.Sleep(s.opts.Interval)
	}
}

func (s *Syncer) sync() error {
	log.Info().Msg("syncing ...")

	//
	// commit local changes
	//
	_, err := s.executer.Execf("git checkout %s", s.opts.Branch)
	if err != nil {
		return fmt.Errorf("sync failed: %w", err)
	}
	modified, err := s.executer.Execf("git ls-files --modified")
	if err != nil {
		return err
	}
	if strings.TrimSpace(modified) != "" {
		log.Info().Msg("commiting local changes ...")
		// add any already tracked files that have been modified
		if _, err := s.executer.Execf("git ls-files --modified | xargs git add"); err != nil {
			return err
		}
		if _, err := s.executer.Execf("git commit -m '%s: %s'", s.id, timestamp()); err != nil {
			return err
		}
	} else {
		log.Info().Msg("no local changes")
	}

	connectivity := s.hasConnectivity()

	//
	// rebase on remote changes
	//
	if !connectivity {
		log.Debug().Msg("remote unreachable, cannot rebase local changes ...")
		return nil
	}
	log.Debug().Msg("fetching remote changes ...")
	if _, err := s.executer.Execf("git fetch %s %s", s.opts.Remote, s.opts.Branch); err != nil {
		return err
	}
	out, err := s.executer.Execf("git rev-list --count %s..%s/%s", s.opts.Branch, s.opts.Remote, s.opts.Branch)
	if err != nil {
		return err
	}
	hasRemoteChanges := strings.TrimSpace(out) != "0"
	if !hasRemoteChanges {
		log.Info().Msg("no remote changes")
	} else {
		log.Info().Msg("rebasing onto remote changes ...")
		if _, err := s.executer.Execf("git rebase %s/%s || true", s.opts.Remote, s.opts.Branch); err != nil {
			return err
		}
		if err := s.resolveConflicts(); err != nil {
			log.Error().Msgf("aborting rebase after failure to resolve conflicts: %v", err)
			if _, err := s.executer.Execf("git rebase --abort"); err != nil {
				return err
			}
		}
	}

	//
	// push local changes
	//
	out, err = s.executer.Execf("git rev-list --count %s/%s..%s", s.opts.Remote, s.opts.Branch, s.opts.Branch)
	if err != nil {
		return err
	}
	hasLocalChanges := strings.TrimSpace(out) != "0"
	if connectivity && hasLocalChanges {
		log.Debug().Msg("pushing local changes ...")
		s.executer.Execf("git push %s %s", s.opts.Remote, s.opts.Branch)
	}

	return nil
}

// name ...
func (s *Syncer) resolveConflicts() error {
	for {
		conflicts, err := s.executer.Execf("git diff --name-only --diff-filter=U")
		if err != nil {
			return err
		}
		if strings.TrimSpace(conflicts) == "" {
			// no more conflicts
			return nil
		}

		files := strings.Split(conflicts, "\n")
		for _, file := range files {
			if file == "" {
				continue
			}
			log.Debug().Msgf("resolving conflict in %s ...", file)
			if _, err := s.executer.Execf("git show :1:%s > %s.common", file, file); err != nil {
				return err
			}
			if _, err := s.executer.Execf("git show :2:%s > %s.ours", file, file); err != nil {
				return err
			}
			if _, err := s.executer.Execf("git show :3:%s > %s.theirs", file, file); err != nil {
				return err
			}

			// resolve in favor of local changes
			strategy := "--theirs"
			if _, err := s.executer.Execf("git merge-file -p %s %s.ours %s.common %s.theirs > %s", strategy, file, file, file, file); err != nil {
				return err
			}

			// mark resolved
			if _, err := s.executer.Execf("git add %s", file); err != nil {
				return err
			}

			_ = os.Remove(file + ".ours")
			_ = os.Remove(file + ".common")
			_ = os.Remove(file + ".theirs")
		}

		if _, err := s.executer.Execf("git rebase --continue"); err != nil {
			return err
		}
	}
}

func timestamp() string {
	return time.Now().Format(time.RFC3339)
}
