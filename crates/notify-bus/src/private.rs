//! The private bus Quickshell runs on: a dbus-daemon of our own, with no service files (so nothing is ever activated on it)
//! and a socket in a folder only we can read.

use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use anyhow::{Context, Result, bail};

pub struct PrivateBus {
    pub child: Child,
    pub address: String,
    pub dir: PathBuf,
}

impl PrivateBus {
    pub fn start(runtime: &std::path::Path) -> Result<PrivateBus> {
        let dir = runtime.join(format!("notifd-bus-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
        let socket = dir.join("bus");
        let config = dir.join("bus.conf");
        std::fs::write(&config, config_text(&socket.display().to_string()))?;
        let mut child = Command::new("dbus-daemon")
            .args(["--nofork", "--nopidfile", "--print-address=1"])
            .arg(format!("--config-file={}", config.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .context("starting dbus-daemon (package dbus)")?;
        let mut line = String::new();
        BufReader::new(child.stdout.take().expect("piped")).read_line(&mut line)?;
        let address = line.trim().to_string();
        if address.is_empty() {
            let _ = child.kill();
            bail!("dbus-daemon did not print its address");
        }
        Ok(PrivateBus { child, address, dir })
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A session-type bus that only we use: anyone who can reach the socket (only us: the folder is 0700) may do anything.
pub fn config_text(socket: &str) -> String {
    format!(
        r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path={socket}</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#
    )
}
