//! Small helpers around the operating system: running commands, atomic writes, process detection.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

/// Run a command, return (exit code, trimmed stdout). A command that cannot start or times out gives (1, "").
pub fn sh(cmd: &str, args: &[&str]) -> (i32, String) {
    sh_timeout(cmd, args, Duration::from_secs(10))
}

pub fn sh_timeout(cmd: &str, args: &[&str], timeout: Duration) -> (i32, String) {
    let Ok(mut child) = Command::new(cmd).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else {
        return (1, String::new());
    };
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(15)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return (1, String::new());
            }
        }
    }
    match child.wait_with_output() {
        Ok(o) => (o.status.code().unwrap_or(1), String::from_utf8_lossy(&o.stdout).trim().to_string()),
        Err(_) => (1, String::new()),
    }
}

pub fn which(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(name)).find(|p| p.is_file() && fs::metadata(p).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)).map(|p| p.to_string_lossy().into_owned())
}

/// Write a file through a temp file + rename so a crash can never leave it half-written.
pub fn atomic_write(path: &Path, text: &str) -> Result<()> {
    let dir = path.parent().context("path has no parent")?;
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let mode = fs::metadata(path).map(|m| m.permissions().mode() & 0o777).unwrap_or(0o644);
    let tmp = dir.join(format!(".{}.{}.tmp", path.file_name().and_then(|n| n.to_str()).unwrap_or("file"), std::process::id()));
    {
        let mut f = fs::File::create(&tmp).with_context(|| format!("creating {}", tmp.display()))?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("writing {}", path.display()));
    }
    Ok(())
}

/// True if any process runs a binary whose path contains one of the needles.
pub fn exe_running(needles: &[&str]) -> bool {
    let Ok(rd) = fs::read_dir("/proc") else { return false };
    for e in rd.flatten() {
        let name = e.file_name();
        let Some(n) = name.to_str() else { continue };
        if !n.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        if let Ok(exe) = fs::read_link(e.path().join("exe")) {
            let s = exe.to_string_lossy();
            if needles.iter().any(|x| s.contains(x)) {
                return true;
            }
        }
    }
    false
}

pub struct Owner {
    pub pid: String,
    pub comm: String,
    pub unit: String,
}

/// Who owns org.freedesktop.Notifications right now.
pub fn owner() -> Option<Owner> {
    owner_of("org.freedesktop.Notifications")
}

/// Who owns a D-Bus name on the session bus right now.
pub fn owner_of(name: &str) -> Option<Owner> {
    let (rc, out) = sh("busctl", &["--user", "status", name]);
    if rc != 0 {
        return None;
    }
    let get = |k: &str| out.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).unwrap_or("").to_string();
    Some(Owner { pid: get("PID"), comm: get("Comm"), unit: get("UserUnit") })
}

/// notifd holds the notification name: Quickshell itself (older installs, `notify dev`), or `notify serve` in front of it.
pub fn notifd_owns() -> bool {
    owner().map(|o| is_notifd(&o)).unwrap_or(false)
}

/// Units whose Quickshell is a desktop shell with its own notification server, not notifd (uQ's `uq-shell`).
const OTHER_QUICKSHELL_UNITS: &[&str] = &["uq-shell.service"];

/// Whether this owner is notifd: `notify serve`, or a Quickshell that is not another shell's (uQ's shell is a
/// Quickshell too, and holds the name with its built-in server until it sees notifd).
pub fn is_notifd(o: &Owner) -> bool {
    o.comm == "notify" || ((o.comm == "qs" || o.comm == "quickshell") && !OTHER_QUICKSHELL_UNITS.contains(&o.unit.as_str()))
}

pub fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// A new folder only this user can use (mode 0700, created fresh: an existing one is refused, never reused), in
/// $XDG_RUNTIME_DIR or else ~/.cache. For downloads and test daemons, which a shared /tmp would let other users tamper with.
pub fn private_dir(prefix: &str) -> Result<std::path::PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cache")))
        .context("neither XDG_RUNTIME_DIR nor HOME is set")?;
    fs::create_dir_all(&base)?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = base.join(format!("{prefix}-{}-{nanos}", std::process::id()));
    fs::DirBuilder::new().mode(0o700).create(&dir).with_context(|| format!("creating {}", dir.display()))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(comm: &str, unit: &str) -> Owner {
        Owner { pid: "1".into(), comm: comm.into(), unit: unit.into() }
    }

    #[test]
    fn notifd_is_notify_serve_or_its_quickshell() {
        assert!(is_notifd(&o("notify", "notifd.service")));
        assert!(is_notifd(&o("qs", "notifd.service")));
        assert!(is_notifd(&o("quickshell", ""))); // `notify dev`, older installs
    }

    #[test]
    fn uq_shell_is_not_notifd() {
        assert!(!is_notifd(&o("qs", "uq-shell.service")));
        assert!(!is_notifd(&o("quickshell", "uq-shell.service")));
        assert!(!is_notifd(&o("plasmashell", "plasma-plasmashell.service")));
        assert!(!is_notifd(&o("mako", "")));
    }
}
