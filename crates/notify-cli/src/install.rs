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
use crate::units::{dbus_text, fallback_unit_text, jobs_unit_text, unit_text};
use crate::integrations;
use crate::lint;
use crate::out::{Out, Step, run_plan, slot};
use crate::paths::{Paths, paths};
use crate::payload::{self, Payload};
use crate::sys::{atomic_write, is_notifd, notifd_owns, owner, owner_of, sh, sh_timeout, sleep_ms, which};

pub const PORTAL_MARK: &str = "# managed by notify: routes portal notifications through notifd";

// ---------------------------------------------------------------- generated files

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
    /// The job bridge unit (empty in installs made before it existed).
    #[serde(default)]
    pub jobs_unit: String,
    /// A plain user service (not KDE Plasma): no plasmashell ordering, no fallback to Plasma, no KWin script.
    #[serde(default)]
    pub plain: bool,
}

/// The session is KDE Plasma (XDG_CURRENT_DESKTOP lists KDE); anything else gets the plain install.
pub fn plasma_session() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.split(':').any(|x| x.eq_ignore_ascii_case("KDE")))
}

pub(crate) fn read_state(p: &Paths) -> Option<TakeoverState> {
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
    let Ok(tmp) = crate::sys::private_dir("notifd-preflight") else { return false };
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

pub fn takeover(ctx: &Ctx, now: bool, dry: bool, no_portal: bool, no_autodnd: bool, force_plain: bool) -> Result<()> {
    let plain = force_plain || !plasma_session();
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
    ctx.out.text(&format!("{}takeover plan{}", if dry { "DRY RUN - " } else { "" }, if plain { " (plain user service: no Plasma here to take over from)" } else { "" }));
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
        let when = if plain { "starts with your graphical session" } else { "starts before plasmashell" };
        steps.push(Step::new(format!("write user unit {} ({when})", p.unit.display()), move || atomic_write(&unit, &unit_text(&payload_dir, plain))));
    }
    if !plain {
        let f = p.fallback_unit.clone();
        steps.push(Step::new(format!("write fallback unit {} (hands back to Plasma if notifd keeps failing)", p.fallback_unit.display()), move || atomic_write(&f, &fallback_unit_text())));
    }
    {
        let f = p.jobs_unit.clone();
        steps.push(Step::new(format!("write job bridge unit {} (KDE apps' copy/move/delete progress through notifd)", p.jobs_unit.display()), move || atomic_write(&f, &jobs_unit_text(plain))));
    }
    {
        let f = p.dbus.clone();
        steps.push(Step::new(format!("write D-Bus activation override {} (restarts notifd if it ever stops)", p.dbus.display()), move || atomic_write(&f, dbus_text())));
    }
    let autodnd_done = slot(false);
    if !no_autodnd && !plain {
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
    steps.push(Step::new("systemctl --user enable notifd.service notifd-jobs.service (active from next login)", move || {
        if !nosys {
            sh_timeout("systemctl", &["--user", "enable", "notifd.service", "notifd-jobs.service"], Duration::from_secs(30));
        }
        Ok(())
    }));
    {
        let (st, pd, ad) = (p.takeover_state.clone(), portal_done.clone(), autodnd_done.clone());
        let (unit, fb, dbus, portal, jobs) = (p.unit.clone(), p.fallback_unit.clone(), p.dbus.clone(), p.portal.clone(), p.jobs_unit.clone());
        steps.push(Step::new(format!("record what was installed in {} (restore removes exactly these)", p.takeover_state.display()), move || {
            let state = TakeoverState {
                unit: unit.display().to_string(),
                fallback_unit: if plain { String::new() } else { fb.display().to_string() },
                dbus: dbus.display().to_string(),
                portal: if *pd.borrow() { Some(portal.display().to_string()) } else { None },
                autodnd: *ad.borrow(),
                jobs_unit: jobs.display().to_string(),
                plain,
            };
            atomic_write(&st, &serde_json::to_string_pretty(&state)?)
        }));
    }
    if now && plain {
        let c2 = ctx.clone();
        steps.push(Step::new("NOW: start notifd and the job bridge; auto-rollback if notifd is not the owner (stop mako, dunst or swaync first)", move || {
            let other = owner().filter(|o| !is_notifd(o));
            if let Some(o) = &other {
                c2.out.text(&format!("  ! {}", handover_note(o)));
            }
            let rc = systemctl(&c2, &["start", "notifd.service"]);
            systemctl(&c2, &["start", "notifd-jobs.service"]);
            if !c2.no_systemctl && (rc != 0 || !healthy(None, plain_start_timeout(other.as_ref()))) {
                c2.out.text("  ! takeover did not take effect; rolling back");
                do_restore(&c2)?;
                bail!("notifd did not become the notification owner and the install was rolled back (is another notification daemon running?)");
            }
            Ok(())
        }));
    } else if now {
        let c2 = ctx.clone();
        steps.push(Step::new("NOW: stop plasmashell, start notifd and the job bridge, start plasmashell (panel disappears ~5s); auto-rollback if notifd is not the owner", move || {
            let sys = |a: &[&str]| systemctl(&c2, a);
            sys(&["stop", "plasma-plasmashell.service"]);
            let rc = sys(&["start", "notifd.service"]);
            sys(&["start", "notifd-jobs.service"]);
            sys(&["start", "plasma-plasmashell.service"]);
            if !c2.no_systemctl && (rc != 0 || !notifd_owns()) {
                c2.out.text("  ! takeover did not take effect; rolling back");
                do_restore(&c2)?;
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

mod update;
pub use update::update;
mod restore;
mod handover;
use handover::{handover_note, plain_start_timeout};
pub use restore::{do_restore, restore};
