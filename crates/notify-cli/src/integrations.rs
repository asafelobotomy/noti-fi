//! Integrations with the rest of the desktop: the full-screen KWin script, the Plasma tray icon,
//! and Brave's profile (global shortcuts are in hotkey.rs).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use chrono::Local;
use serde_json::{Value, json};

use crate::apps::{CHROMIUM_FAMILY, config_dir, running_needles};
use crate::ctx::Ctx;
use crate::daemon;
use crate::out::{BAD, INFO, OK, WARN};
use crate::paths::paths;
use crate::sys::{atomic_write, exe_running, sh, sleep_ms, which};

// ---------------------------------------------------------------- automatic DND: the KWin script

const KWIN_ID: &str = "notifd-autodnd";

const KWIN_JS: &str = "// Installed by `notify autodnd`. Logs the active window's full-screen state; notifd follows this log line.
var last = null;
var watched = {};
function isFull(w) { return !!(w && w.fullScreen && !w.desktopWindow && !w.dock); }
function report() {
    var s = isFull(workspace.activeWindow);
    if (s !== last) { last = s; print(\"notifd-autodnd: fullscreen=\" + (s ? 1 : 0)); }
}
function watch(w) {
    if (!w || watched[w.internalId]) return;
    watched[w.internalId] = true;
    w.fullScreenChanged.connect(report);
}
workspace.windowActivated.connect(function (w) { watch(w); report(); });
workspace.windowRemoved.connect(report);
watch(workspace.activeWindow);
report();
";

fn kwin_meta() -> Value {
    json!({
        "KPlugin": {
            "Id": KWIN_ID, "Name": "notifd automatic Do Not Disturb", "Version": "1.0", "License": "MIT",
            "Description": "Logs whether the active window is full screen so notifd can pause popups",
            "Authors": [{"Name": "noti-fi"}], "EnabledByDefault": false
        },
        "X-Plasma-API": "javascript", "X-Plasma-MainScript": "code/main.js", "KPackageStructure": "KWin/Script"
    })
}

pub fn kwin_loaded() -> bool {
    let (rc, out) = sh("qdbus6", &["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.isScriptLoaded", KWIN_ID]);
    rc == 0 && out.trim() == "true"
}

pub fn autodnd_install() -> bool {
    let d = paths().kwin_script;
    if atomic_write(&d.join("metadata.json"), &(serde_json::to_string_pretty(&kwin_meta()).unwrap_or_default() + "\n")).is_err() || atomic_write(&d.join("contents/code/main.js"), KWIN_JS).is_err() {
        return false;
    }
    sh("kwriteconfig6", &["--file", "kwinrc", "--group", "Plugins", "--key", &format!("{KWIN_ID}Enabled"), "true"]);
    sh("qdbus6", &["org.kde.KWin", "/KWin", "reconfigure"]);
    sleep_ms(1500);
    if !kwin_loaded() {
        // reconfigure did not pick it up: load it directly
        sh("qdbus6", &["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.loadScript", &d.join("contents/code/main.js").to_string_lossy(), KWIN_ID]);
        sh("qdbus6", &["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.start"]);
        sleep_ms(1000);
    }
    kwin_loaded()
}

pub fn autodnd_remove() {
    sh("qdbus6", &["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.unloadScript", KWIN_ID]);
    sh("kwriteconfig6", &["--file", "kwinrc", "--group", "Plugins", "--key", &format!("{KWIN_ID}Enabled"), "false"]);
    sh("qdbus6", &["org.kde.KWin", "/KWin", "reconfigure"]);
    let _ = fs::remove_dir_all(paths().kwin_script);
    // never leave a stale "full screen" hold behind
    if let Some(mut c) = daemon::try_connect(false) {
        let _ = c.set_auto("fullscreen", false);
    }
}

pub fn autodnd(ctx: &Ctx, action: &str) -> Result<()> {
    match action {
        "install" => {
            ctx.out.text("installing the KWin script that reports full-screen windows to notifd...");
            let ok = autodnd_install();
            ctx.out.result(ok, if ok { "  installed and running" } else { "  installed, but KWin did not load it; log out and in, or check: qdbus6 org.kde.KWin /Scripting" });
            if ok { Ok(()) } else { bail!("KWin did not load the script") }
        }
        "remove" => {
            autodnd_remove();
            ctx.out.result(true, "removed");
            Ok(())
        }
        _ => {
            let d = paths().kwin_script;
            let loaded = kwin_loaded();
            let daemon = daemon::try_connect(ctx.dev).and_then(|mut c| c.status().ok());
            if ctx.out.json {
                ctx.out.document(&json!({
                    "installed": d.exists(), "loaded": loaded,
                    "daemon": daemon.as_ref().map(|s| json!({"fullscreen": s.auto.fullscreen, "share": s.auto.share, "holding": s.holding}))
                }));
                return Ok(());
            }
            ctx.out.line(if d.exists() { OK } else { INFO }, &format!("KWin script files: {}", if d.exists() { "present" } else { "not installed (run: notify autodnd install)" }), None);
            ctx.out.line(if loaded { OK } else { INFO }, &format!("KWin script loaded: {}", if loaded { "yes" } else { "no" }), None);
            if let Some(s) = daemon {
                ctx.out.line(INFO, &format!("daemon sees: fullscreen={} screen-share={}; holding notifications: {}", s.auto.fullscreen, s.auto.share, if s.holding.is_empty() { "no" } else { &s.holding }), None);
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------- Plasma tray icon

const TRAY_ITEM: &str = "org.kde.plasma.notifications";

fn tray_js(action: &str) -> String {
    format!(
        "
var out = \"no system tray found\";
var ps = panels();
for (var i = 0; i < ps.length; i++) {{
  var ws = ps[i].widgets();
  for (var j = 0; j < ws.length; j++) {{
    var w = ws[j];
    if (w.type != \"org.kde.plasma.systemtray\") continue;
    w.currentConfigGroup = [\"General\"];
    var items = String(w.readConfig(\"extraItems\")).split(\",\").filter(function (x) {{ return x.length > 0; }});
    var has = items.indexOf(\"{TRAY_ITEM}\") >= 0;
    var action = \"{action}\";
    if (action == \"hide\" && has) {{ items.splice(items.indexOf(\"{TRAY_ITEM}\"), 1); w.writeConfig(\"extraItems\", items); w.reloadConfig(); has = false; }}
    if (action == \"show\" && !has) {{ items.push(\"{TRAY_ITEM}\"); w.writeConfig(\"extraItems\", items); w.reloadConfig(); has = true; }}
    out = has ? \"shown\" : \"hidden\";
  }}
}}
print(out);
"
    )
}

pub fn tray_state(action: &str) -> Option<String> {
    let (rc, out) = sh("qdbus6", &["org.kde.plasmashell", "/PlasmaShell", "org.kde.PlasmaShell.evaluateScript", &tray_js(action)]);
    if rc == 0 { Some(out.trim().to_string()) } else { None }
}

/// Plasma 6 hosts its notification service inside the notifications tray applet. If that applet is hidden, Plasma
/// cannot take notifications back, so `restore` (and the automatic fallback) must show it again first.
/// Returns true if it had been hidden by `notify tray hide`.
pub fn tray_ensure_shown() -> bool {
    let marker = paths().tray_marker;
    if !marker.exists() {
        return false;
    }
    let shown = tray_state("show").as_deref() == Some("shown");
    if shown {
        let _ = fs::remove_file(&marker);
    }
    shown
}

pub fn tray(ctx: &Ctx, action: &str) -> Result<()> {
    let Some(st) = tray_state(action) else { bail!("could not reach plasmashell's scripting interface") };
    let marker = paths().tray_marker;
    match action {
        "hide" if st == "hidden" => {
            if let Some(d) = marker.parent() {
                fs::create_dir_all(d)?;
            }
            fs::write(&marker, "notify hid Plasma's notification tray item; `notify restore` shows it again\n")?;
        }
        "show" if st == "shown" => {
            let _ = fs::remove_file(&marker);
        }
        _ => {}
    }
    if ctx.out.json {
        ctx.out.document(&json!({"state": st, "hidden_by_notify": marker.exists()}));
        return Ok(());
    }
    let extra = if st == "shown" && crate::sys::notifd_owns() { "  (it no longer does anything since notifd owns notifications)" } else { "" };
    println!("Plasma's notification item in the system tray: {st}{extra}");
    if st == "hidden" {
        println!("Note: Plasma runs its notification service inside that item, so `notify restore` (and the automatic fallback) will show it again.");
    }
    Ok(())
}

// ---------------------------------------------------------------- Brave

fn get_path<'a>(v: &'a Value, dotted: &str) -> Option<&'a Value> {
    dotted.split('.').try_fold(v, |cur, k| cur.get(k))
}

fn set_path(v: &mut Value, dotted: &str, value: Value) {
    let keys: Vec<&str> = dotted.split('.').collect();
    let mut cur = v;
    for k in &keys[..keys.len() - 1] {
        if !cur.get(*k).map(Value::is_object).unwrap_or(false) {
            cur[*k] = json!({});
        }
        cur = &mut cur[*k];
    }
    cur[keys[keys.len() - 1]] = value;
}

pub struct AdaptArgs {
    pub apply: bool,
    pub background: bool,
    pub autostart: bool,
}

struct Change {
    file: PathBuf,
    key: &'static str,
    label: &'static str,
}

pub fn adapt(ctx: &Ctx, a: AdaptArgs) -> Result<()> {
    let cfg = config_dir();
    let targets: Vec<_> = CHROMIUM_FAMILY.iter().filter(|(n, rel, _)| n.starts_with("Brave") && cfg.join(rel).join("Default/Preferences").exists()).collect();
    if targets.is_empty() {
        bail!("no Brave profile found");
    }
    ctx.out.text(if a.apply { "APPLY - Brave adaptation" } else { "PREVIEW (nothing is changed without --apply) - Brave adaptation" });
    let mut failed = false;
    for (name, rel, entry) in targets {
        ctx.out.blank();
        ctx.out.heading(name);
        let base = cfg.join(rel);
        let (prefs_f, ls_f) = (base.join("Default/Preferences"), base.join("Local State"));
        let (mut prefs, mut ls): (Value, Value) = match (fs::read_to_string(&prefs_f).ok().and_then(|t| serde_json::from_str(&t).ok()), fs::read_to_string(&ls_f).ok().and_then(|t| serde_json::from_str(&t).ok())) {
            (Some(p), Some(l)) => (p, l),
            _ => {
                ctx.out.line(BAD, "cannot read profile", None);
                failed = true;
                continue;
            }
        };
        let mut changes: Vec<Change> = Vec::new();
        let push = get_path(&prefs, "brave.gcm.channel_status").and_then(Value::as_bool).unwrap_or(false);
        ctx.out.line(if push { OK } else { WARN }, &format!("web push (Google push messaging): {}", if push { "on" } else { "off" }), None);
        if !push {
            changes.push(Change { file: prefs_f.clone(), key: "brave.gcm.channel_status", label: "enable web push" });
        }
        let subs = get_path(&prefs, "gcm.push_messaging_application_id_map").and_then(Value::as_object).map(|m| m.len()).unwrap_or(0);
        ctx.out.line(INFO, &format!("{subs} sites currently hold a push subscription"), None);
        let bg = get_path(&ls, "background_mode.enabled").and_then(Value::as_bool).unwrap_or(false);
        ctx.out.line(INFO, &format!("keeps running after the last window closes: {}{}", if bg { "yes" } else { "no" }, if a.background || bg { "" } else { "   (--background to enable; push only arrives while Brave runs)" }), None);
        if a.background && !bg {
            changes.push(Change { file: ls_f.clone(), key: "background_mode.enabled", label: "keep running in the background" });
        }
        if changes.is_empty() {
            ctx.out.line(OK, "nothing to change", None);
        } else {
            for c in &changes {
                ctx.out.line(INFO, &format!("will set {} = true  ({})", c.key, c.label), None);
            }
            if a.apply {
                if exe_running(&running_needles(entry)) {
                    ctx.out.line(BAD, &format!("{name} is running; it would overwrite the change on exit"), Some("close all Brave windows (and its tray icon), then retry"));
                    failed = true;
                    continue;
                }
                let stamp = paths().backups.join(Local::now().format("%Y%m%d-%H%M%S").to_string()).join(name.replace(' ', "-"));
                // the profile files can hold account details: the backups are private (folder 0700, files 0600)
                fs::create_dir_all(&stamp)?;
                for d in [paths().backups, stamp.parent().map(Path::to_path_buf).unwrap_or_default(), stamp.clone()] {
                    let _ = fs::set_permissions(&d, fs::Permissions::from_mode(0o700));
                }
                for f in [&prefs_f, &ls_f] {
                    if changes.iter().any(|c| &c.file == f) {
                        let to = stamp.join(f.file_name().unwrap());
                        fs::copy(f, &to)?;
                        fs::set_permissions(&to, fs::Permissions::from_mode(0o600))?;
                    }
                }
                for c in &changes {
                    set_path(if c.file == prefs_f { &mut prefs } else { &mut ls }, c.key, json!(true));
                }
                for (f, v) in [(&prefs_f, &prefs), (&ls_f, &ls)] {
                    if changes.iter().any(|c| &c.file == f) {
                        atomic_write(f, &serde_json::to_string(v)?)?;
                    }
                }
                ctx.out.line(OK, &format!("applied; backup of the originals in {}", stamp.display()), None);
            }
        }
        if a.autostart {
            let exe = which(if *entry == "brave-origin" { "brave-origin" } else { "brave-browser" }).or_else(|| which("brave"));
            let ent = paths().autostart.join(format!("notify-{entry}.desktop"));
            ctx.out.line(INFO, &format!("autostart at login (hidden, no window): {}", ent.display()), None);
            if a.apply {
                if let Some(exe) = exe {
                    atomic_write(&ent, &format!("[Desktop Entry]\nType=Application\nName={name} (background, for notifications)\nExec={exe} --no-startup-window\nX-GNOME-Autostart-enabled=true\n"))?;
                    ctx.out.line(OK, "autostart entry written", None);
                }
            }
        }
    }
    if !a.apply {
        ctx.out.text("\nPreview only. Re-run with --apply (and close Brave first) to make the changes.");
    }
    if failed { bail!("adapt did not complete") } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_script_mentions_the_item_and_action() {
        let js = tray_js("hide");
        assert!(js.contains("org.kde.plasma.notifications") && js.contains("var action = \"hide\""));
    }
}
