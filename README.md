# About
`gitdrive` continuously synchronizes modifications to git-tracked files, taking
care of conflicts should they arise. It's intended to be a poor man's Google
docs -- giving a combination of save-as-you-type functionality and simple
synchronization of documents/files between multiple devices.

Personally I use it to keep my collection of [org-mode](https://orgmode.org/)
consistent across computers.

Any changes to tracked files in a local git repository (the 'watch dir') are
periodically synced with its upstream remote in an attempt to keep the local
repo up-to-date with the remote and persist changes as quickly as possible.
The remote needs to be set up for password-less push or automation will fail.


# Install
To install the `gitdrive` binary under `$HOME/.cargo/bin` run:

    cargo install --path .

# Run
To start syncing `origin/master` of a git directory run:

    gitdrive /path/to/git-root-dir
    
The `--branch` and `--remote` flags can be used to change the remote and branch
being synced to something other than the default `origin/master`.

## Logging
By default, only `INFO`-level logs are output. To control the amount of log
output use the `RUST_LOG` environment variable (`trace`, `debug`, `info`,
`warn`, `error`).

    RUST_LOG=debug cargo run -- <git directory>


## Run as user-level systemd service
A convenient option for having `gitdrive` always sync a certain directory
whenever the user is logged in is to execute `gitdrive` as a `systemd`
(user-level) unit.

1. Modify the `systemd` unit template under
   [systemd/gitdrive.service](systemd/gitdrive.service).

2. Place it under `~/.config/systemd/user/` and run

        # start service
        systemctl --user start  gitdrive.service
        # autostart service on future logins
        systemctl --user enable gitdrive.service

This will result in `gitdrive` being launched when the `systemd --user` instance
starts up (usually when the user logs in for the first time) and it will be
stopped, together with all other user systemd units, when the last user session
exits.

To see output when `gitdrive` runs as a `systemd` unit, run:

     journalctl --user -f -u gitdrive
