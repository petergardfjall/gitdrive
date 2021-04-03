extern crate chrono;

use chrono::Utc;
use log;
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::process::Command;
use std::str;

type ExecResult<T> = std::result::Result<T, ExecError>;

#[derive(Debug)]
pub enum ExecError {
    NonZeroExit {
        cmd: String,
        stderr: String,
        status: std::process::ExitStatus,
    },
    IO {
        cmd: String,
        err: Box<dyn std::error::Error>,
    },
}

impl Error for ExecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self {
            ExecError::NonZeroExit { .. } => None,
            ExecError::IO { ref err, .. } => Some(err.as_ref()),
        }
    }
}

impl fmt::Display for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            ExecError::NonZeroExit {
                cmd,
                stderr,
                status,
            } => write!(f, "{}: non-zero exit ({}):\n{}", cmd, status, stderr),
            ExecError::IO { cmd, err } => write!(f, "{}: i/o error: {}", cmd, err),
        }
    }
}

struct Executer<'a> {
    work_dir: &'a str,
}

impl<'a> Executer<'a> {
    fn new(work_dir: &'a str) -> Executer {
        Executer { work_dir }
    }

    fn exec(&self, cmd: &str) -> ExecResult<String> {
        log::debug!("{}", cmd);

        let output = Command::new("/bin/sh")
            .current_dir(Path::new(&self.work_dir))
            .arg("-c")
            .arg(cmd)
            .output()
            .map_err(|err| ExecError::IO {
                cmd: String::from(cmd),
                err: Box::new(err),
            })?;

        if !output.status.success() {
            return Err(ExecError::NonZeroExit {
                cmd: String::from(cmd),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                status: output.status,
            });
        }
        let stdout = String::from_utf8(output.stdout).map_err(|e| ExecError::IO {
            cmd: String::from(cmd),
            err: Box::new(e),
        })?;
        log::trace!("{}", stdout);
        Ok(stdout)
    }
}

type Result<T> = std::result::Result<T, GitDriveError>;

#[derive(Debug)]
pub enum GitDriveError {
    NoSuchDir { path: String },
    NoGitRepo { path: String },
    RemoteNotFound { remote: String },
    BranchNotFound { branch: String },
    Exec(ExecError),
    ParseError { message: String },
}

impl Error for GitDriveError {}

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

impl From<ExecError> for GitDriveError {
    fn from(err: ExecError) -> GitDriveError {
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
    executer: Executer<'a>,
    opts: GitDriveOpts<'a>,
}

impl<'a> GitDrive<'a> {
    pub fn new(opts: GitDriveOpts<'a>) -> Result<GitDrive<'a>> {
        opts.validate()?;

        Ok(GitDrive {
            executer: Executer::new(&opts.watch_dir),
            opts,
        })
    }

    pub fn sync(&self) -> Result<()> {
        log::info!("syncing ...");

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
