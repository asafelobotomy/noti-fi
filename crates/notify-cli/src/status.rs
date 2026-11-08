//! `status` and `doctor`.

use serde::Serialize;
use serde_json::json;

use crate::apps;
use crate::controls;
use crate::ctx::Ctx;
use crate::daemon;
use crate::install::PORTAL_MARK;
use crate::integrations;
use crate::out::{BAD, INFO, OK, WARN};
use crate::paths::paths;
use crate::sys::{notifd_owns, owner, owner_of, sh, which};
use notify_protocol::{ConfigStatus, Status};

#[derive(Serialize)]
pub struct OwnerInfo {
    pub pid: String,
    pub comm: String,
    pub unit: String,
}

#[derive(Serialize)]
pub struct StatusReport {
    pub owner: Option<OwnerInfo>,
    pub notifd_owns: bool,
    /// Who shows file operations (org.kde.JobViewServer): notifd's job bridge (`notify`) or Plasma.
    pub jobs_owner: Option<OwnerInfo>,
    pub daemon: Option<Status>,
    pub config: Option<ConfigStatus>,
}

pub fn status_data(ctx: &Ctx) -> StatusReport {
    let mut c = daemon::try_connect(ctx.dev);
    StatusReport {
        owner: owner().map(|o| OwnerInfo { pid: o.pid, comm: o.comm, unit: o.unit }),
        notifd_owns: notifd_owns(),
        jobs_owner: owner_of(notify_jobs::JOB_SERVER).map(|o| OwnerInfo { pid: o.pid, comm: o.comm, unit: o.unit }),
        daemon: c.as_mut().and_then(|c| c.status().ok()),
        config: c.as_mut().and_then(|c| c.config_status().ok()),
    }
}

fn print_status(ctx: &Ctx, r: &StatusReport) {
    ctx.out.heading("Notification ownership");
    match &r.owner {
        None => ctx.out.line(WARN, "nobody owns org.freedesktop.Notifications", Some("notifications from apps are being dropped")),
        Some(o) => {
            ctx.out.line(if r.notifd_owns { OK } else { INFO }, &format!("owner: {} (pid {}, unit {})", o.comm, o.pid, if o.unit.is_empty() { "none" } else { &o.unit }), None);
            ctx.out.line(INFO, if r.notifd_owns { "notifd is the owner" } else { "stock owner is active (notifd is NOT in control)" }, None);
        }
    }
    match &r.jobs_owner {
        Some(o) if o.comm == "notify" => ctx.out.line(OK, &format!("file operations (Dolphin's copy/move progress): shown by notifd's job bridge (pid {})", o.pid), None),
        Some(o) if r.notifd_owns => ctx.out.line(WARN, &format!("file operations (Dolphin's copy/move progress): held by {}, so they do not show", o.comm), Some("the job bridge takes over at the next login (or: systemctl --user restart plasma-plasmashell)")),
        Some(o) => ctx.out.line(INFO, &format!("file operations (Dolphin's copy/move progress): shown by {}", o.comm), None),
        None => ctx.out.line(if r.notifd_owns { WARN } else { INFO }, "nobody shows file operations (org.kde.JobViewServer has no owner)", r.notifd_owns.then_some("systemctl --user start notifd-jobs")),
    }
    let Some(d) = &r.daemon else {
        ctx.out.line(INFO, "no notifd instance answering", None);
        return;
    };
    ctx.out.line(
        INFO,
        &format!(
            "Do Not Disturb {}, muted: {}, {} on screen{}, {} in history",
            if d.dnd { "ON" } else { "off" },
            if d.muted.is_empty() { "none".to_string() } else { d.muted.join(", ") },
            d.active,
            if d.queued > 0 { format!(" (+{} queued)", d.queued) } else { String::new() },
            d.history
        ),
        None,
    );
    if !d.focus.is_empty() {
        ctx.out.line(INFO, &format!("focus mode '{}' is on: only important notifications are shown (see: notify focus list)", d.focus), None);
    }
    if !d.holding.is_empty() && d.holding != "dnd" && d.holding != "focus" {
        ctx.out.line(INFO, &format!("holding popups automatically: {} (critical ones still show)", d.holding), None);
    }
    if !d.cfg_error.is_empty() {
        ctx.out.line(BAD, &format!("rules.json rejected, running on the previous config: {}", d.cfg_error), Some("fix it, then check with: notify rules"));
    }
}

pub fn status(ctx: &Ctx) -> anyhow::Result<()> {
    let r = status_data(ctx);
    if ctx.out.json {
        ctx.out.document(&r);
    } else {
        print_status(ctx, &r);
    }
    Ok(())
}

#[derive(Serialize)]
struct Check {
    group: &'static str,
    level: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<String>,
}

pub fn doctor(ctx: &Ctx) -> anyhow::Result<bool> {
    let p = paths();
    let mut checks: Vec<Check> = Vec::new();
    let mut add = |group: &'static str, level: &'static str, message: String, hint: Option<&str>| checks.push(Check { group, level, message, hint: hint.map(String::from) });

    for (t, why) in [("qs", "Quickshell runs the daemon"), ("dbus-daemon", "the daemon's private bus"), ("busctl", "inspect D-Bus ownership"), ("notify-send", "send test notifications"), ("systemctl", "manage the service")] {
        let found = which(t).is_some();
        add("Tools", if found { OK } else { WARN }, format!("{t}: {} ({why})", if found { "found" } else { "missing" }), None);
    }
    let st = status_data(ctx);
    if st.daemon.is_none() {
        add("Daemon", WARN, "no notifd instance answering on the control socket".into(), Some("systemctl --user start notifd"));
    }
    let installed = p.takeover_state.exists();
    let plain = crate::install::read_state(&p).is_some_and(|s| s.plain);
    let what = if plain { "installed as a plain user service (starts with your graphical session)" } else { "takeover installed (starts at login)" };
    add("Install", if installed { OK } else { INFO }, if installed { what.into() } else { "takeover not installed (another program owns notifications)".into() }, None);
    if installed {
        let (_, enabled) = sh("systemctl", &["--user", "is-enabled", "notifd.service"]);
        add("Install", if enabled == "enabled" { OK } else { WARN }, format!("notifd.service: {}", if enabled.is_empty() { "unknown" } else { &enabled }), None);
        let fb = p.fallback_unit.exists();
        if !plain {
            add("Install", if fb { OK } else { WARN }, format!("automatic fallback to Plasma if notifd keeps failing: {}", if fb { "armed" } else { "missing" }), if fb { None } else { Some("run: notify update") });
        }
        let jobs = p.jobs_unit.exists();
        add("Install", if jobs { OK } else { WARN }, format!("job bridge for file operations (notifd-jobs.service): {}", if jobs { "installed" } else { "missing" }), if jobs { None } else { Some("run: notify update") });
        let portal_ok = std::fs::read_to_string(&p.portal).map(|t| t.contains(PORTAL_MARK)).unwrap_or(false);
        add("Install", if portal_ok { OK } else { WARN }, format!("Flatpak/portal notifications routed through notifd: {}", if portal_ok { "yes" } else { "NO" }), if portal_ok { None } else { Some("run: notify portal") });
        if plain {
            add("Install", INFO, "not on Plasma: no fallback to Plasma and no KWin full-screen script (screen sharing and apps' Inhibit still hold popups)".into(), None);
        } else {
            let loaded = integrations::kwin_loaded();
            add("Install", if loaded { OK } else { WARN }, format!("automatic Do Not Disturb for full-screen windows: {}", if loaded { "active" } else { "KWin script not loaded" }), if loaded { None } else { Some("run: notify autodnd install") });
            if p.tray_marker.exists() {
                add("Install", INFO, "Plasma's notification tray icon is hidden by notify (restore and the automatic fallback show it again so Plasma can take notifications back)".into(), None);
            }
            if integrations::tray_state("status").as_deref() == Some("shown") {
                add("Install", WARN, "Plasma's own notification icon is still in the system tray (it does nothing now)".into(), Some("run: notify tray hide"));
            }
        }
    }
    let (rep, rules_text, rules_ok) = controls::rules_report(ctx, None);
    let apps = apps::collect();

    if ctx.out.json {
        let (okc, warnc, failc) = checks.iter().fold((0, 0, 0), |a, c| match c.level {
            OK => (a.0 + 1, a.1, a.2),
            WARN => (a.0, a.1 + 1, a.2),
            BAD => (a.0, a.1, a.2 + 1),
            _ => a,
        });
        ctx.out.document(&json!({
            "checks": checks, "status": st, "rules": rep, "apps": apps,
            "summary": {"ok": okc, "warn": warnc, "fail": failc + usize::from(!rules_ok)}
        }));
        return Ok(rules_ok);
    }
    let mut last = "";
    for c in &checks {
        if c.group != last {
            if c.group == "Install" || c.group == "Daemon" {
                // the status block sits between Tools and Install, as before
            }
            last = c.group;
            if c.group == "Install" {
                println!();
                print_status(ctx, &st);
                println!();
                println!("Install");
            } else if c.group == "Tools" {
                println!("Tools");
            }
        }
        ctx.out.line(c.level, &c.message, c.hint.as_deref());
    }
    if !checks.iter().any(|c| c.group == "Install") {
        println!();
        print_status(ctx, &st);
    }
    println!();
    controls::print_rules(ctx, &rep, rules_text.as_deref(), false);
    println!();
    apps::run(ctx)?;
    Ok(rules_ok)
}
