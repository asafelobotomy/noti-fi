//! Installing, updating and removing notifd as a user service. Every change is previewable (--dry-run),
//! reported step by step, and reversible with `notify restore`.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use notify_client::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ctx::Ctx;
use crate::units::{dbus_text, fallback_unit_text, unit_text};
use crate::integrations;
use crate::lint;
use crate::out::{Out, Step, run_plan, slot};
use crate::paths::{Paths, paths};
use crate::payload::{self, Payload};
use crate::sys::{atomic_write, notifd_owns, owner, sh, sh_timeout, sleep_ms, which};

pub const PORTAL_MARK: &str = "# managed by notify: routes portal notifications through notifd";

// ---------------------------------------------------------------- generated files (identical to the Python CLI's)

pub fn exe_path() -> String {
    let installed = crate::paths::home().join(".local/bin/notify");
    if installed.exists() {
        return installed.to_string_lossy().into_owned();
    }
    std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "notify".into())
}

// ---------------------------------------------------------------- state record

#[derive(Serialize, Deserialize, Default)]
pub struct TakeoverState {
    pub unit: String,
    pub fallback_unit: String,
    pub dbus: String,
    pub portal: Option<String>,
    #[serde(default)]
    pub autodnd: bool,
}

fn read_state(p: &Paths) -> Option<TakeoverState> {
    serde_json::from_str(&fs::read_to_string(&p.takeover_state).ok()?).ok()
}

/// Signal a whole process group; it may already be gone, which is fine.
pub fn kill_group(sig: &str, pgid: u32) {
    let _ = Command::new("kill").args([sig, &format!("-{pgid}")]).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

pub fn systemctl(ctx: &Ctx, args: &[&str]) -> i32 {
    if ctx.no_systemctl {
        return 0;
    }
    let mut a = vec!["--user"];
    a.extend_from_slice(args);
    sh_timeout("systemctl", &a, Duration::from_secs(30)).0
}

// ---------------------------------------------------------------- portal routing

pub fn portal_enable(ctx: &Ctx) -> Result<bool> {
    let p = paths();
    if p.portal.exists() && !fs::read_to_string(&p.portal)?.contains(PORTAL_MARK) {
        ctx.out.text(&format!("  ! {} already exists and was not made by notify; leaving it alone", p.portal.display()));
        return Ok(false);
    }
    if !Path::new("/usr/share/xdg-desktop-portal/portals/gtk.portal").exists() {
        ctx.out.text("  ! xdg-desktop-portal-gtk is not installed (it provides the notification route notifd needs)");
        return Ok(false);
    }
    let mut base = fs::read_to_string(&p.portal_system).unwrap_or_else(|_| "[preferred]\ndefault=*\n".to_string());
    let key = "org.freedesktop.impl.portal.Notification=";
    if base.lines().any(|l| l.starts_with(key)) {
        base = base.lines().map(|l| if l.starts_with(key) { format!("{key}gtk") } else { l.to_string() }).collect::<Vec<_>>().join("\n") + "\n";
    } else {
        base = base.trim_end_matches('\n').to_string() + &format!("\n{key}gtk\n");
    }
    atomic_write(&p.portal, &format!("{PORTAL_MARK}\n{base}"))?;
    systemctl(ctx, &["restart", "xdg-desktop-portal.service"]);
    Ok(true)
}

pub fn portal_disable(ctx: &Ctx) -> Result<bool> {
    let p = paths();
    if p.portal.exists() && fs::read_to_string(&p.portal)?.contains(PORTAL_MARK) {
        fs::remove_file(&p.portal)?;
        systemctl(ctx, &["restart", "xdg-desktop-portal.service"]);
        return Ok(true);
    }
    Ok(false)
}

pub fn portal(ctx: &Ctx, remove: bool, dry: bool) -> Result<()> {
    let p = paths();
    let title = format!("{}{} portal routing", if dry { "DRY RUN - " } else { "" }, if remove { "remove" } else { "enable" });
    ctx.out.text(&title);
    if remove {
        ctx.out.text(&format!("  remove {} (if made by notify) and restart xdg-desktop-portal; Flatpak notifications go back to Plasma", p.portal.display()));
        if !dry {
            let done = portal_disable(ctx)?;
            ctx.out.result(true, if done { "  done" } else { "  nothing to remove" });
        }
        return Ok(());
    }
    ctx.out.text(&format!("  write {}: Notification backend plasmanotify -> gtk (the GTK backend sends through org.freedesktop.Notifications)", p.portal.display()));
    ctx.out.text("  restart xdg-desktop-portal (apps keep running; portal dialogs may blink for a second)");
    if dry {
        ctx.out.result(true, "\nDry run only; nothing was changed.");
        return Ok(());
    }
    if portal_enable(ctx)? {
        ctx.out.result(true, "portal routing enabled");
        Ok(())
    } else {
        bail!("portal routing was not enabled")
    }
}

// ---------------------------------------------------------------- smoke test and health

/// Start a daemon from `files` on a private D-Bus session and check it comes up and answers on its socket.
/// This runs BEFORE the live daemon is touched, so a broken version can never take notifications down.
pub fn preflight(payload: &Payload, rules_text: &str) -> bool {
    let tmp = std::env::temp_dir().join(format!("notifd-preflight-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    let dir = tmp.join("p");
    if fs::create_dir_all(&dir).is_err() || fs::create_dir_all(tmp.join("state")).is_err() {
        return false;
    }
    let ok = (|| -> Result<bool> {
        payload.write_files(&dir)?;
        atomic_write(&dir.join("rules.json"), rules_text)?;
        let sock = tmp.join("control.sock");
        use std::os::unix::process::CommandExt;
        let mut child = Command::new("dbus-run-session")
            .args(["--", "bash", "-c", "exec qs -p \"$1\"", "notifd-smoke"])
            .arg(&dir)
            .env("QT_NO_XDG_DESKTOP_PORTAL", "1")
            .env("XDG_STATE_HOME", tmp.join("state"))
            .env("NOTIFD_SOCKET", &sock)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()?;
        let end = Instant::now() + Duration::from_secs(30);
        let mut up = false;
        while Instant::now() < end {
            if matches!(child.try_wait(), Ok(Some(_))) {
                break; // it exited: a broken QML file exits immediately
            }
            if let Ok(mut c) = Client::connect(&sock) {
                if c.ping().is_ok() {
                    up = true;
                    break;
                }
            }
            sleep_ms(300);
        }
        let pgid = child.id();
        kill_group("-TERM", pgid);
        for _ in 0..80 {
            if matches!(child.try_wait(), Ok(Some(_))) {
                break;
            }
            sleep_ms(100);
        }
        kill_group("-KILL", pgid);
        let _ = child.wait();
        Ok(up)
    })()
    .unwrap_or(false);
    let _ = fs::remove_dir_all(&tmp);
    ok
}

/// notifd owns the name and answers (on its socket, or via `qs ipc` for versions that predate the socket).
pub fn healthy(abandoned_if_missing: Option<&Path>, timeout: Duration) -> bool {
    let end = Instant::now() + timeout;
    let payload = paths().payload;
    while Instant::now() < end {
        if notifd_owns() {
            if let Ok(mut c) = Client::connect(notify_client::default_socket_path()) {
                if c.ping().is_ok() {
                    return true;
                }
            }
            let (rc, out) = sh("qs", &["-p", &payload.to_string_lossy(), "ipc", "call", "notifd", "status"]);
            if rc == 0 && !out.is_empty() {
                return true;
            }
        }
        if let Some(f) = abandoned_if_missing {
            if !f.exists() {
                return false;
            }
        }
        sleep_ms(500);
    }
    false
}

// ---------------------------------------------------------------- takeover

pub fn takeover(ctx: &Ctx, now: bool, dry: bool, no_portal: bool, no_autodnd: bool) -> Result<()> {
    let p = paths();
    let pl = Payload::load(ctx.payload_dir.as_deref())?;
    let rules_v: Value = serde_json::from_str(&pl.rules)?;
    let errs = lint::lint_config(&rules_v);
    if !errs.is_empty() {
        bail!("refusing: the default rules.json is invalid: {}", errs.join("; "));
    }
    if which("qs").is_none() && !ctx.no_systemctl {
        bail!("refusing: quickshell (qs) is not installed");
    }
    if p.takeover_state.exists() {
        bail!("notifd takeover is already installed (use 'notify update' to change it, or 'notify restore' first)");
    }
    ctx.out.text(&format!("{}takeover plan", if dry { "DRY RUN - " } else { "" }));
    let pl = std::rc::Rc::new(pl);
    let nosys = ctx.no_systemctl;
    let mut steps: Vec<Step> = Vec::new();

    {
        let (pl, dir) = (pl.clone(), p.payload.clone());
        steps.push(Step::new(format!("copy daemon ({}) to {}", payload::file_names().collect::<Vec<_>>().join(", "), p.payload.display()), move || {
            fs::create_dir_all(&dir)?;
            pl.write_files(&dir)
        }));
    }
    {
        let (pl, rules, payload_dir) = (pl.clone(), p.rules.clone(), p.payload.clone());
        steps.push(Step::new(format!("config at {} (kept if it exists), linked into the daemon folder", p.rules.display()), move || {
            if !rules.exists() {
                atomic_write(&rules, &pl.rules)?;
            }
            let link = payload_dir.join("rules.json");
            if link.symlink_metadata().is_ok() {
                fs::remove_file(&link)?;
            }
            symlink(&rules, &link)?;
            Ok(())
        }));
    }
    {
        let (unit, payload_dir) = (p.unit.clone(), p.payload.clone());
        steps.push(Step::new(format!("write user unit {} (starts before plasmashell)", p.unit.display()), move || atomic_write(&unit, &unit_text(&payload_dir))));
    }
    {
        let f = p.fallback_unit.clone();
        steps.push(Step::new(format!("write fallback unit {} (hands back to Plasma if notifd keeps failing)", p.fallback_unit.display()), move || atomic_write(&f, &fallback_unit_text())));
    }
    {
        let f = p.dbus.clone();
        steps.push(Step::new(format!("write D-Bus activation override {} (restarts notifd if it ever stops)", p.dbus.display()), move || atomic_write(&f, dbus_text())));
    }
    let autodnd_done = slot(false);
    if !no_autodnd {
        let d = autodnd_done.clone();
        steps.push(Step::new("install the KWin script for automatic Do Not Disturb (full-screen windows)", move || {
            if !nosys {
                *d.borrow_mut() = integrations::autodnd_install();
            }
            Ok(())
        }));
    }
    let portal_done = slot(false);
    if !no_portal {
        let d = portal_done.clone();
        let c2 = ctx.clone();
        steps.push(Step::new(format!("route Flatpak/portal notifications through notifd ({})", p.portal.display()), move || {
            *d.borrow_mut() = portal_enable(&c2)?;
            Ok(())
        }));
    }
    steps.push(Step::new("systemctl --user daemon-reload", move || {
        if !nosys {
            sh_timeout("systemctl", &["--user", "daemon-reload"], Duration::from_secs(30));
        }
        Ok(())
    }));
    steps.push(Step::new("systemctl --user enable notifd.service (active from next login)", move || {
        if !nosys {
            sh_timeout("systemctl", &["--user", "enable", "notifd.service"], Duration::from_secs(30));
        }
        Ok(())
    }));
    {
        let (st, pd, ad) = (p.takeover_state.clone(), portal_done.clone(), autodnd_done.clone());
        let (unit, fb, dbus, portal) = (p.unit.clone(), p.fallback_unit.clone(), p.dbus.clone(), p.portal.clone());
        steps.push(Step::new(format!("record what was installed in {} (restore removes exactly these)", p.takeover_state.display()), move || {
            let state = TakeoverState {
                unit: unit.display().to_string(),
                fallback_unit: fb.display().to_string(),
                dbus: dbus.display().to_string(),
                portal: if *pd.borrow() { Some(portal.display().to_string()) } else { None },
                autodnd: *ad.borrow(),
            };
            atomic_write(&st, &serde_json::to_string_pretty(&state)?)
        }));
    }
    if now {
        let c2 = ctx.clone();
        steps.push(Step::new("NOW: stop plasmashell, start notifd, start plasmashell (panel disappears ~5s); auto-rollback if notifd is not the owner", move || {
            let sys = |a: &[&str]| systemctl(&c2, a);
            sys(&["stop", "plasma-plasmashell.service"]);
            let rc = sys(&["start", "notifd.service"]);
            sys(&["start", "plasma-plasmashell.service"]);
            if !c2.no_systemctl && (rc != 0 || !notifd_owns()) {
                c2.out.text("  ! takeover did not take effect; rolling back");
                do_restore(&c2, true, false)?;
                bail!("takeover did not take effect and was rolled back");
            }
            Ok(())
        }));
    } else {
        steps.push(Step::note("(no --now: nothing running changes; notifd takes over at next login)"));
    }
    run_plan(&ctx.out, "takeover", steps, dry)?;
    ctx.out.result(true, if dry { "\nDry run only; nothing was changed." } else { "\nDone. Undo any time with: notify restore" });
    Ok(())
}

// ---------------------------------------------------------------- update

fn merge_new_defaults(src: &str, dst: &Path) -> Vec<String> {
    let (Ok(s), Ok(dtext)) = (serde_json::from_str::<Value>(src), fs::read_to_string(dst)) else { return vec![] };
    let Ok(mut d) = serde_json::from_str::<Value>(&dtext) else { return vec![] };
    let (Some(so), Some(dobj)) = (s.as_object(), d.as_object_mut()) else { return vec![] };
    let added: Vec<String> = so.keys().filter(|k| !dobj.contains_key(*k)).cloned().collect();
    if !added.is_empty() {
        for k in &added {
            dobj.insert(k.clone(), so[k].clone());
        }
        let _ = atomic_write(dst, &(serde_json::to_string_pretty(&d).unwrap_or_default() + "\n"));
    }
    added
}

pub fn update(ctx: &Ctx) -> Result<()> {
    let p = paths();
    if !p.takeover_state.exists() {
        bail!("takeover is not installed; run 'notify takeover' first");
    }
    let pl = Payload::load(ctx.payload_dir.as_deref())?;
    let out: Out = ctx.out;
    out.text(&format!("  new version: {}", pl.origin));
    if !ctx.no_systemctl {
        out.text("  smoke-testing the new version on a private bus (your live notifications are not touched)...");
        let rules_for_test = fs::read_to_string(&p.rules).unwrap_or_else(|_| pl.rules.clone());
        if !preflight(&pl, &rules_for_test) {
            bail!("the new version failed its smoke test, so NOTHING was changed. Fix it, or run `notify dev` to see the errors.");
        }
        out.text("  smoke test passed");
    }
    // keep the working version so a bad update can be undone
    fs::create_dir_all(&p.prev)?;
    for n in payload::file_names() {
        if p.payload.join(n).exists() {
            fs::copy(p.payload.join(n), p.prev.join(n))?;
        }
    }
    pl.write_files(&p.payload)?;
    for (n, _) in &pl.files {
        out.text(&format!("  updated {n}"));
    }
    for (path, text) in [(&p.unit, unit_text(&p.payload)), (&p.fallback_unit, fallback_unit_text()), (&p.dbus, dbus_text().to_string())] {
        if fs::read_to_string(path).ok().as_deref() != Some(&text) {
            atomic_write(path, &text)?;
            out.text(&format!("  refreshed {}", path.file_name().and_then(|n| n.to_str()).unwrap_or("")));
        }
    }
    let added = merge_new_defaults(&pl.rules, &p.rules);
    if !added.is_empty() {
        out.text(&format!("  added new settings to your rules.json (existing values untouched): {}", added.join(", ")));
    }
    if ctx.no_systemctl {
        out.result(true, "");
        return Ok(());
    }
    systemctl(ctx, &["daemon-reload"]);
    systemctl(ctx, &["restart", "notifd.service"]);
    out.text("  restarted notifd.service (notifications pause for about a second)");
    if healthy(Some(&p.takeover_state), Duration::from_secs(15)) {
        out.result(true, "  health check passed");
        return Ok(());
    }
    if !p.takeover_state.exists() {
        bail!("notifd kept crashing and the automatic fallback has handed notifications back to Plasma. Fix the problem, then re-enable with: notify takeover --now");
    }
    out.text("  ! notifd did not come back healthy; rolling back to the previous version");
    for n in payload::file_names() {
        if p.prev.join(n).exists() {
            fs::copy(p.prev.join(n), p.payload.join(n))?;
        }
    }
    systemctl(ctx, &["reset-failed", "notifd.service"]);
    systemctl(ctx, &["restart", "notifd.service"]);
    let back = healthy(None, Duration::from_secs(15));
    bail!("the update was rolled back: {}", if back { "notifd is healthy again on the previous version" } else { "notifd is STILL NOT HEALTHY - run: notify restore" })
}

// ---------------------------------------------------------------- restore

/// The steps that hand notifications back to Plasma. Used by `notify restore` (reported step by step) and by
/// the rollback paths (run silently).
fn restore_steps(ctx: &Ctx, purge: bool) -> Vec<Step> {
    let p = paths();
    let state = std::rc::Rc::new(read_state(&p).unwrap_or_default());
    let mut steps = Vec::new();
    {
        let c = ctx.clone();
        steps.push(Step::new("systemctl --user disable --now notifd.service", move || {
            systemctl(&c, &["disable", "--now", "notifd.service"]);
            Ok(())
        }));
    }
    {
        let (st, up, fp, dp) = (state.clone(), p.unit.clone(), p.fallback_unit.clone(), p.dbus.clone());
        steps.push(Step::new(format!("remove {}, {} and the D-Bus activation override", file_name(&p.unit), file_name(&p.fallback_unit)), move || {
            for (recorded, default) in [(&st.unit, &up), (&st.fallback_unit, &fp), (&st.dbus, &dp)] {
                let f = if recorded.is_empty() { default.clone() } else { PathBuf::from(recorded) };
                if f.exists() {
                    fs::remove_file(&f)?;
                }
            }
            Ok(())
        }));
    }
    {
        let (c, st) = (ctx.clone(), state.clone());
        steps.push(Step::new("remove the portal routing (if notify created it) and the full-screen KWin script", move || {
            portal_disable(&c)?;
            if st.autodnd && !c.no_systemctl {
                integrations::autodnd_remove();
            }
            // Plasma needs its notification tray item to take notifications back; show it if `notify tray hide` hid it
            if !c.no_systemctl {
                integrations::tray_ensure_shown();
            }
            Ok(())
        }));
    }
    {
        let (c, ts) = (ctx.clone(), p.takeover_state.clone());
        steps.push(Step::new("daemon-reload; if nobody owns the name, restart plasmashell so Plasma takes it back", move || {
            if !c.no_systemctl {
                systemctl(&c, &["daemon-reload"]);
                systemctl(&c, &["reset-failed", "notifd.service"]);
                for _ in 0..10 {
                    // give plasmashell a moment to take the name (its notification applet may just have been shown again)
                    if owner().is_some() {
                        break;
                    }
                    sleep_ms(500);
                }
                if owner().is_none() {
                    systemctl(&c, &["restart", "plasma-plasmashell.service"]);
                }
            }
            if ts.exists() {
                fs::remove_file(&ts)?;
            }
            Ok(())
        }));
    }
    if purge {
        let (payload, prev) = (p.payload.clone(), p.prev.clone());
        steps.push(Step::new(format!("delete daemon files {} (your config in {} is kept)", p.payload.display(), p.rules.display()), move || {
            let _ = fs::remove_dir_all(&payload);
            let _ = fs::remove_dir_all(&prev);
            Ok(())
        }));
    }
    steps
}

/// Hand notifications back without printing anything (rollbacks).
pub fn do_restore(ctx: &Ctx, _quiet: bool, _auto: bool) -> Result<()> {
    for s in restore_steps(ctx, false) {
        if let Some(a) = s.action {
            a()?;
        }
    }
    Ok(())
}

pub fn restore(ctx: &Ctx, dry: bool, purge: bool, auto: bool) -> Result<()> {
    ctx.out.text(&format!("{}restore plan", if dry { "DRY RUN - " } else { "" }));
    let steps = restore_steps(ctx, purge);
    run_plan(&ctx.out, "restore", steps, dry)?;
    if dry {
        ctx.out.result(true, "\nDry run only; nothing was changed.");
        return Ok(());
    }
    ctx.out.result(true, "restored: Plasma owns notifications again");
    if auto && !ctx.no_systemctl {
        for _ in 0..20 {
            if owner().is_some() {
                break;
            }
            sleep_ms(500);
        }
        let _ = Command::new("notify-send")
            .args(["-a", "noti-fi", "-u", "critical", "Notification daemon failed", "notifd kept failing, so notifications were handed back to Plasma. Check the setup, then run: notify takeover --now"])
            .status();
    }
    Ok(())
}

fn file_name(p: &Path) -> String {
    p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string()
}

