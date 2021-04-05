extern crate clap;
extern crate env_logger;
extern crate log;

mod gitdrive;
use gitdrive::{GitDrive, GitDriveOpts};

use hostname;
use std::error::Error;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    // enable by `RUST_LOG=<level>`
    env_logger::init();

    let cli = clap::App::new("gitdrive")
        .version("0.0.1")
        .author("Peter Gardfjäll")
        .about(
            "Continuously sync modifications to git-tracked files: a poor man's Google docs.

Any changes to tracked files in a local git repository (the 'watch dir') are
periodically synced with its upstream remote in an attempt to keep the local
repo up-to-date with the remote and persist changes as quickly as possible.
The remote needs to be set up for password-less push or automation will fail.
",
        )
        .arg(
            clap::Arg::with_name("watch-dir")
                .default_value(".")
                .index(1)
                .required(false)
                .help("directory to watch"),
        )
        .arg(
            clap::Arg::with_name("branch")
                .long("branch")
                .takes_value(true)
                .default_value("master")
                .help("local branch to sync with remote"),
        )
        .arg(
            clap::Arg::with_name("interval")
                .long("period")
                .takes_value(true)
                .default_value("30")
                .help("time in seconds between successive sync attempts"),
        )
        .arg(
            clap::Arg::with_name("once")
                .long("once")
                .takes_value(false)
                .help("just sync with remote once, without entering watch mode"),
        )
        .arg(
            clap::Arg::with_name("remote")
                .long("remote")
                .takes_value(true)
                .default_value("origin")
                .help("remote sync repository"),
        );
    // .arg(
    //     clap::Arg::with_name("use-libnotify")
    //         .long("use-libnotify")
    //         .takes_value(false)
    //         .help("use libnotify (notify-send) to notify user of errors"),
    // )
    // .arg(
    //     clap::Arg::with_name("notify-dedup-interval")
    //         .long("notify-dedup-interval")
    //         .takes_value(true)
    //         .default_value("3600")
    //         .help("time duration (in seconds) during which duplicate events are suppressed"),
    // );

    let matches = cli.get_matches();
    let watch_dir = matches.value_of("watch-dir").unwrap();
    let remote = matches.value_of("remote").unwrap();
    let branch = matches.value_of("branch").unwrap();
    let interval = Duration::from_secs(
        matches
            .value_of("interval")
            .unwrap()
            .parse::<u64>()
            .unwrap(),
    );

    let host = hostname::get()?;
    let host = host.to_str().expect("failed to convert hostname to string");

    let gitdrive = GitDrive::new(GitDriveOpts {
        watch_dir,
        remote,
        branch,
        hostname: &host,
    })?;

    loop {
        if let Err(error) = gitdrive.sync() {
            panic!("sync failed: {:?}", error);
        }

        if matches.is_present("once") {
            return Ok(());
        }
        sleep(interval);
    }
}
