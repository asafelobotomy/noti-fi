//! Development helpers: a throwaway daemon on a private bus, and the regression suite.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::ctx::Ctx;
use crate::paths::paths;
use crate::payload::Payload;

/// The project checkout this binary was built from, if it still exists.
pub fn project_root() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.join("notifd/shell.qml").exists().then(|| root.canonicalize().unwrap_or(root))
}

/// Run a throwaway daemon on a private D-Bus session (never touches the real one).
pub fn dev(ctx: &Ctx) -> Result<()> {
    let p = paths();
    // prefer live files from the checkout so edits show up on restart; otherwise the embedded copy
    let dir = match (&ctx.payload_dir, project_root()) {
        (Some(d), _) => d.clone(),
        (None, Some(root)) => root.join("notifd"),
        (None, None) => {
            let pl = Payload::embedded();
            pl.write_files(&p.dev_payload)?;
            crate::sys::atomic_write(&p.dev_payload.join("rules.json"), &pl.rules)?;
            p.dev_payload.clone()
        }
    };
    fs::create_dir_all(p.dev_bus.parent().unwrap())?;
    fs::create_dir_all(&p.dev_state)?;
    let _ = fs::remove_file(&p.dev_sock);
    println!("dev daemon starting from {} on a private bus; Ctrl+C stops it.", dir.display());
    println!("In another terminal try:  notify --dev test    notify --dev dnd on");
    let status = Command::new("dbus-run-session")
        .args(["--", "bash", "-c", "echo \"$DBUS_SESSION_BUS_ADDRESS\" > \"$1\"; exec qs -p \"$2\"", "notifd-dev"])
        .arg(&p.dev_bus)
        .arg(&dir)
        .env("QT_NO_XDG_DESKTOP_PORTAL", "1")
        .env("XDG_STATE_HOME", &p.dev_state)
        .env("NOTIFD_SOCKET", &p.dev_sock)
        .status()
        .context("running dbus-run-session (is dbus installed?)")?;
    let _ = fs::remove_file(&p.dev_bus);
    let _ = fs::remove_file(&p.dev_sock);
    if status.success() || status.code().is_none() { Ok(()) } else { bail!("the dev daemon exited with {status}") }
}

/// Run the regression suite (needs the project checkout).
pub fn selftest() -> Result<()> {
    let Some(root) = project_root() else { bail!("selftest needs the project checkout (it was not found at {})", env!("CARGO_MANIFEST_DIR")) };
    let t = root.join("tests/test_notifd.py");
    if !t.exists() {
        bail!("tests/test_notifd.py not found in {}", root.display());
    }
    let st = Command::new("python3").arg("-I").arg(&t).status().context("running python3")?;
    if st.success() { Ok(()) } else { bail!("the regression suite failed") }
}
