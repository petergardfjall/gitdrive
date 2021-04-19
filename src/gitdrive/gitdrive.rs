extern crate chrono;

use super::shell;

use chrono::Utc;
use std::fmt;
use std::path::Path;

type Result<T> = std::result::Result<T, GitDriveError>;

#[derive(Debug)]
pub enum GitDriveError {
    NoSuchDir { path: String },
    NoGitRepo { path: String },
    RemoteNotFound { remote: String },
    BranchNotFound { branch: String },
    Exec(shell::Error),
    ParseError { message: String },
}

impl std::error::Error for GitDriveError {}

impl fmt::Display for GitDriveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            GitDriveError::NoSuchDir { path } => write!(f, "no such directory: {}", path),
            GitDriveError::NoGitRepo { path } => write!(f, "not a git directory: {}", path),
            GitDriveError::RemoteNotFound { remote } => {
                write!(f, "remote does not exist: {}", remote)
            }
            GitDriveError::BranchNotFound { branch } => {
                write!(f, "branch does not exist: {}", branch)
            }
            GitDriveError::Exec(e) => {
                write!(f, "execution error: {:?}", e)
            }
            GitDriveError::ParseError { message } => {
                write!(f, "parse error: {:?}", message)
            }
        }
    }
}

impl From<shell::Error> for GitDriveError {
    fn from(err: shell::Error) -> GitDriveError {
        GitDriveError::Exec(err)
    }
}

impl From<std::num::ParseIntError> for GitDriveError {
    fn from(err: std::num::ParseIntError) -> GitDriveError {
        GitDriveError::ParseError {
            message: err.to_string(),
        }
    }
}

pub struct GitDriveOpts<'a> {
    pub watch_dir: &'a str,
    pub remote: &'a str,
    pub branch: &'a str,
    pub hostname: &'a str,
}

impl<'a> GitDriveOpts<'a> {
    pub fn validate(&self) -> Result<()> {
        let watch_path = Path::new(self.watch_dir);
        if !watch_path.is_dir() {
            return Err(GitDriveError::NoSuchDir {
                path: self.watch_dir.to_owned(),
            });
        }

        // must be a git repo
        if !watch_path.join(".git").is_dir() {
            return Err(GitDriveError::NoGitRepo {
                path: self.watch_dir.to_owned(),
            });
        }

        // remote must exist: .git/refs/remotes/<remote>
        if !watch_path
            .join(".git/refs/remotes")
            .join(self.remote)
            .is_dir()
        {
            return Err(GitDriveError::RemoteNotFound {
                remote: self.remote.to_owned(),
            });
        }

        // branch must exist: .git/refs/heads/<branch>
        if !watch_path
            .join(".git/refs/heads")
            .join(self.branch)
            .is_file()
        {
            return Err(GitDriveError::BranchNotFound {
                branch: self.branch.to_owned(),
            });
        }
        Ok(())
    }
}

pub struct GitDrive<'a> {
    executer: shell::Executer<'a>,
    opts: GitDriveOpts<'a>,
}

impl<'a> GitDrive<'a> {
    pub fn new(opts: GitDriveOpts<'a>) -> Result<GitDrive<'a>> {
        opts.validate()?;

        Ok(GitDrive {
            executer: shell::Executer::new(&opts.watch_dir),
            opts,
        })
    }

    pub fn sync(&self) -> Result<()> {
        log::info!("syncing {} ...", self.opts.watch_dir);

        //
        // commit local changes
        //
        self.executer
            .exec(&format!("git checkout {}", self.opts.branch))?;
        let local_changes = self.executer.exec(&format!("git ls-files --modified"))?;
        if local_changes.trim() != "" {
            log::info!("committing local changes ...");
            self.executer
                .exec(&format!("git ls-files --modified | xargs git add"))?;
            self.executer.exec(&format!(
                "git commit -m '{}: {}'",
                self.opts.hostname,
                Utc::now().to_rfc3339()
            ))?;
        } else {
            log::info!("no local changes");
        }

        //
        // rebase on remote changes (and resolve any conflicts)
        //
        let remote_reachable = self.has_connectivity();
        if !remote_reachable {
            log::info!("remote unreachable, cannot rebase local changes ...");
            return Ok(());
        }

        // fetch remote changes
        self.executer.exec(&format!(
            "git fetch {} {}",
            self.opts.remote, self.opts.branch
        ))?;
        // remote changes?
        let new_upstream_commits = self
            .executer
            .exec(&format!(
                "git rev-list --count {}..{}/{}",
                self.opts.branch, self.opts.remote, self.opts.branch
            ))?
            .trim()
            .parse::<i32>()?;
        let has_remote_changes = new_upstream_commits > 0;
        if has_remote_changes {
            log::info!("rebasing onto remote changes ...");
            // favor our changes on conflict
            self.executer.exec(&format!(
                "git rebase {}/{} || true",
                self.opts.remote, self.opts.branch
            ))?;
            self.resolve_conflicts()?;
        } else {
            log::info!("no remote changes");
        }

        //
        // push local changes to remote
        //
        // local changes?
        let new_local_commits = self
            .executer
            .exec(&format!(
                "git rev-list --count {remote}/{branch}..{branch}",
                branch = self.opts.branch,
                remote = self.opts.remote
            ))?
            .trim()
            .parse::<i32>()?;
        let has_local_changes = new_local_commits > 0;

        if has_local_changes {
            self.executer.exec(&format!(
                "git push {remote} {branch}",
                remote = self.opts.remote,
                branch = self.opts.branch
            ))?;
        }

        Ok(())
    }

    fn has_connectivity(&self) -> bool {
        !self.executer.exec("git ls-remote --exit-code -h").is_err()
    }

    fn resolve_conflicts(&self) -> Result<()> {
        // while there still are conflicts we resolve them in our favor
        loop {
            let out = self
                .executer
                .exec(&format!("git diff --name-only --diff-filter=U"))?;
            let conflicts: Vec<&str> = out.lines().collect();
            if conflicts.is_empty() {
                return Ok(());
            }

            for conflict in conflicts {
                let file = conflict.trim();
                self.executer
                    .exec(&format!("git show :1:{f} > {f}.common", f = file))?;
                self.executer
                    .exec(&format!("git show :2:{f} > {f}.ours", f = file))?;
                self.executer
                    .exec(&format!("git show :3:{f} > {f}.theirs", f = file))?;

                // resolve conflict in favor our changes
                let strategy = "--theirs";
                self.executer.exec(&format!(
                    "git merge-file -p {strategy} {f}.ours {f}.common {f}.theirs > {f}",
                    strategy = strategy,
                    f = file
                ))?;

                // mark resolved
                self.executer.exec(&format!("git add {f}", f = file))?;
                // cleanup
                self.executer
                    .exec(&format!("rm {f}.ours {f}.common {f}.theirs", f = file))?;
            }
            self.executer.exec("git rebase --continue")?;
        }
    }
}
