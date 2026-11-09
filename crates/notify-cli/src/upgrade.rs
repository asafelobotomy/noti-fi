//! `notify upgrade`: look for a newer release on GitHub and, with `--apply`, install it.
//! Nothing is contacted unless you run this. The plan is shown first (and is all that happens without `--apply`).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use notify_update::{CURRENT, Curl, Release, install_prefix, is_newer};
use serde_json::json;

use crate::ctx::Ctx;
use crate::out::{INFO, OK, Step, run_plan, slot};
use crate::paths::paths;

/// Where this program is installed, as a folder the user can write to.
fn writable_prefix() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("cannot tell where this program is")?;
    let exe = fs::canonicalize(&exe).unwrap_or(exe);
    let prefix = install_prefix(&exe).ok_or_else(|| anyhow!("{} is not in a `bin` folder, so it looks like a build rather than an installed copy. Install from a release archive first (see the README), then run this from there.", exe.display()))?;
    let probe = prefix.join("bin").join(".notify-upgrade-probe");
    fs::write(&probe, b"").map_err(|e| anyhow!("cannot write to {} ({e}). For a system-wide install, update with your package tool or run this with run0.", prefix.join("bin").display()))?;
    let _ = fs::remove_file(&probe);
    Ok(prefix)
}

fn describe(r: &Release) -> String {
    format!("noti-fi {} is available (you have {CURRENT}): {}", r.version, r.page)
}

pub fn upgrade(ctx: &Ctx, apply: bool) -> Result<()> {
    let out = ctx.out;
    // fail early, before any network use, when this copy cannot be replaced
    let prefix = if apply { Some(writable_prefix()?) } else { None };

    let rel = notify_update::latest(&Curl).map_err(|e| anyhow!("could not check for updates: {e}"))?;
    let newer = is_newer(CURRENT, &rel.version);
    if !newer {
        if out.json && !apply {
            out.document(&json!({"current": CURRENT, "latest": rel.version, "newer": false, "page": rel.page}));
        } else if out.json {
            out.result(true, "already up to date");
        } else {
            out.line(OK, &format!("noti-fi {CURRENT} is up to date (the latest release is {})", rel.version), None);
        }
        return Ok(());
    }
    if !apply {
        if out.json {
            out.document(&json!({"current": CURRENT, "latest": rel.version, "newer": true, "page": rel.page, "notes": rel.notes}));
        } else {
            out.line(INFO, &describe(&rel), None);
            out.text("  `notify upgrade --apply` will: download it, check its checksum, replace notify and notify-panel here, then refresh the daemon");
            out.text("  (notifications pause for about a second). Restart the panel afterwards to use the new version.");
        }
        return Ok(());
    }

    let prefix = prefix.expect("checked above");
    let bytes = slot(Vec::<u8>::new());
    let skipped = slot(None::<String>);
    let skipped2 = skipped.clone();
    let (b1, b2) = (bytes.clone(), bytes.clone());
    let (rel1, rel2) = (rel.clone(), rel.clone());
    let new_notify = prefix.join("bin/notify");
    let daemon_installed = paths().takeover_state.exists();
    let json_mode = out.json;
    let mut steps = vec![
        Step::new(format!("download {} and check its checksum", rel.archive_name), move || {
            *b1.borrow_mut() = notify_update::download_verified(&Curl, &rel1).map_err(|e| anyhow!(e))?;
            Ok(())
        }),
        Step::new(format!("replace notify, notify-panel and the launcher in {}", prefix.display()), {
            let prefix = prefix.clone();
            move || {
                let work = crate::sys::private_dir("notify-upgrade")?;
                let r = notify_update::install_archive(&b2.borrow(), &rel2.version, &prefix, &work, notify_update::host_qt_version().as_deref()).map_err(|e| anyhow!(e));
                let _ = fs::remove_dir_all(&work);
                *skipped2.borrow_mut() = r?.panel_skipped;
                Ok(())
            }
        }),
    ];
    if daemon_installed {
        steps.push(Step::new("refresh the daemon with the new version (notify update)", move || {
            let mut c = Command::new(&new_notify);
            c.arg("update");
            if json_mode {
                c.arg("--json");
            }
            let st = c.status().context("running the new notify update")?;
            if st.success() { Ok(()) } else { bail!("the new version was installed, but refreshing the daemon failed; run `notify update` to see why") }
        }));
    } else {
        steps.push(Step::note("(the daemon is not installed on this machine, so there is nothing to refresh)"));
    }
    out.line(INFO, &describe(&rel), None);
    run_plan(&out, "upgrade", steps, false)?;
    match skipped.borrow().as_deref() {
        None => out.result(true, &format!("updated to noti-fi {}. Restart the panel (noti-fi) to use it.", rel.version)),
        Some(why) => out.result(true, &format!("updated notify to {}, but notify-panel was NOT replaced: {why}. Build the panel for this machine with `makepkg -si` (packaging/PKGBUILD) or `cargo build --release -p notify-panel`.", rel.version)),
    }
    Ok(())
}
