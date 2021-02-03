package main

import (
	"flag"
	"fmt"
	"os"
	"time"

	"github.com/petergardfjall/gitdrive/pkg/gitdrive"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
)

var (
	opts = gitdrive.SyncerOpts{
		Remote: "origin",
		Branch: "master",

		Once:     false,
		Interval: 1 * time.Minute,
		WatchDir: os.Getenv("PWD"),

		UseLibNotify:        true,
		NotifyDedupInterval: 1 * time.Hour,
	}
)

func init() {
	log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stdout, TimeFormat: time.RFC3339})

	flag.Usage = func() {
		fmt.Fprintf(os.Stdout, `Continuously sync modifications to git-tracked files: a poor man's Google docs.

Any changes to tracked files in a local git repository (the "watch dir") are
periodically synced with its upstream remote in an attempt to keep the local
repo up-to-date with the remote and persist changes as quickly as possible.
The remote needs to be set up for password-less push or automation will fail.
`)
		fmt.Fprintf(os.Stdout, "usage: %s [OPTIONS]\n\n", os.Args[0])
		fmt.Fprintf(os.Stdout, "OPTIONS:\n\n")
		flag.PrintDefaults()
	}

	flag.BoolVar(&opts.Once, "once", opts.Once, "just sync with remote once, without entering watch mode")

	flag.BoolVar(&opts.UseLibNotify, "use-libnotify", opts.UseLibNotify, "use libnotify (notify-send) to notify user of errors")
	flag.DurationVar(&opts.NotifyDedupInterval, "notify-dedup-interval", opts.NotifyDedupInterval, "time duration during which duplicate events are suppressed")

	flag.DurationVar(&opts.Interval, "interval", opts.Interval, "time between successive sync attempts")
	flag.StringVar(&opts.WatchDir, "watch-dir", opts.WatchDir, "directory to watch")
	flag.StringVar(&opts.Remote, "remote", opts.Remote, "remote sync repository")
	flag.StringVar(&opts.Branch, "branch", opts.Branch, "local branch to sync with remote")
}

func main() {

	flag.Parse()

	syncer, err := gitdrive.NewSyncer(opts)
	if err != nil {
		log.Fatal().Msgf(err.Error())
	}

	if err := syncer.Run(); err != nil {
		log.Fatal().Msgf("syncer: %v", err)
	}
}
