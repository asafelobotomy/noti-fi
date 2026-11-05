//! Connecting to the running daemon, with errors that say what to do.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use notify_client::{Client, Error};

use crate::paths::paths;

pub fn socket_path(dev: bool) -> PathBuf {
    if dev { paths().dev_sock } else { notify_client::default_socket_path() }
}

/// Connect, or explain why not.
pub fn connect(dev: bool) -> Result<Client> {
    let path = socket_path(dev);
    Client::connect(&path).map_err(|e| match e {
        Error::Connect { .. } => anyhow!(
            "notifd is not running (no control socket at {}). {}",
            path.display(),
            if dev {
                "Start one with: notify dev"
            } else if paths().takeover_state.exists() {
                "Start it with: systemctl --user start notifd"
            } else {
                "It is not installed: see what that changes with `notify takeover --dry-run`, then run `notify takeover --now`"
            }
        ),
        Error::VersionMismatch { .. } => anyhow!("{e}"),
        other => anyhow!("{other}"),
    })
}

/// Connect if the daemon is there; no complaints otherwise.
pub fn try_connect(dev: bool) -> Option<Client> {
    Client::connect(socket_path(dev)).ok()
}
