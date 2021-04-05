use log;
use std::fmt;
use std::path::Path;
use std::process::Command;
use std::str;

type ExecResult<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
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

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self {
            Error::NonZeroExit { .. } => None,
            Error::IO { ref err, .. } => Some(err.as_ref()),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            Error::NonZeroExit {
                cmd,
                stderr,
                status,
            } => write!(f, "{}: non-zero exit ({}):\n{}", cmd, status, stderr),
            Error::IO { cmd, err } => write!(f, "{}: i/o error: {}", cmd, err),
        }
    }
}

pub struct Executer<'a> {
    work_dir: &'a str,
}

impl<'a> Executer<'a> {
    pub fn new(work_dir: &'a str) -> Executer {
        Executer { work_dir }
    }

    pub fn exec(&self, cmd: &str) -> ExecResult<String> {
        log::debug!("{}", cmd);

        let output = Command::new("/bin/sh")
            .current_dir(Path::new(&self.work_dir))
            .arg("-c")
            .arg(cmd)
            .output()
            .map_err(|err| Error::IO {
                cmd: String::from(cmd),
                err: Box::new(err),
            })?;

        if !output.status.success() {
            return Err(Error::NonZeroExit {
                cmd: String::from(cmd),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                status: output.status,
            });
        }
        let stdout = String::from_utf8(output.stdout).map_err(|e| Error::IO {
            cmd: String::from(cmd),
            err: Box::new(e),
        })?;
        log::trace!("stdout: {}", stdout);
        Ok(stdout)
    }
}
