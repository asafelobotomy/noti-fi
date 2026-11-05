//! Which browsers and apps are ready to deliver notifications. Counts only: no site names are read out.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::ctx::Ctx;
use crate::out::{INFO, OK, WARN};
use crate::paths::home;
use crate::sys::exe_running;

/// (display name, profile folder relative to ~/.config, key into RUNNING)
pub const CHROMIUM_FAMILY: [(&str, &str, &str); 6] = [
    ("Brave Origin", "BraveSoftware/Brave-Origin", "brave-origin"),
    ("Brave", "BraveSoftware/Brave-Browser", "brave"),
    ("Chrome", "google-chrome", "chrome"),
    ("Chromium", "chromium", "chromium"),
    ("Vivaldi", "vivaldi", "vivaldi"),
    ("Edge", "microsoft-edge", "msedge"),
];

/// Executable paths that mean "this browser is running" (specific: Electron apps ship chrome_* helpers).
pub fn running_needles(entry: &str) -> Vec<&'static str> {
    match entry {
        "brave-origin" => vec!["brave-origin"],
        "brave" => vec!["brave-bin", "brave.com", "brave-browser"],
        "chrome" => vec!["google-chrome", "/opt/google/chrome"],
        "chromium" => vec!["/usr/lib/chromium", "/chromium/chromium"],
        "vivaldi" => vec!["vivaldi"],
        "msedge" => vec!["microsoft-edge", "msedge"],
        _ => vec![],
    }
}

const ELECTRON: [&str; 8] = ["Claude", "discord", "Slack", "Signal", "Cursor", "Code", "obsidian", "Proton Mail"];

#[derive(Serialize)]
pub struct Chromium {
    pub name: String,
    pub profile: bool,
    pub running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct Firefox {
    pub profile: String,
    pub allowed: Option<u32>,
    pub blocked: Option<u32>,
}

#[derive(Serialize)]
pub struct Apps {
    pub chromium: Vec<Chromium>,
    pub flatpak_browsers: Vec<String>,
    pub firefox: Vec<Firefox>,
    pub electron: Vec<String>,
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| home().join(".config"))
}

fn json_at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(v, |cur, k| cur.get(*k))
}

/// Count of allowed / blocked notification exceptions, and whether Brave's push messaging is on.
fn chromium_profile(base: &Path) -> Option<std::result::Result<(u32, u32, bool), String>> {
    let prefs = base.join("Default/Preferences");
    if !prefs.exists() {
        return None;
    }
    let text = match fs::read_to_string(&prefs) {
        Ok(t) => t,
        Err(_) => return Some(Err("unreadable Preferences".into())),
    };
    let v: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Some(Err("unreadable Preferences".into())),
    };
    let (mut allowed, mut blocked) = (0, 0);
    if let Some(m) = json_at(&v, &["profile", "content_settings", "exceptions", "notifications"]).and_then(Value::as_object) {
        for e in m.values() {
            match e.get("setting").and_then(Value::as_i64) {
                Some(1) => allowed += 1,
                Some(2) => blocked += 1,
                _ => {}
            }
        }
    }
    let push = json_at(&v, &["brave", "gcm", "channel_status"]).and_then(Value::as_bool).unwrap_or(false);
    Some(Ok((allowed, blocked, push)))
}

/// Profile (name, folder) pairs from every profiles.ini Firefox may use (XDG, classic, Flatpak).
fn firefox_profiles() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for root in [config_dir().join("mozilla/firefox"), home().join(".mozilla/firefox"), home().join(".var/app/org.mozilla.firefox/.mozilla/firefox")] {
        let Ok(text) = fs::read_to_string(root.join("profiles.ini")) else { continue };
        let mut section = String::new();
        let (mut name, mut path, mut rel): (String, String, bool) = (String::new(), String::new(), true);
        let flush = |section: &str, name: &mut String, path: &mut String, rel: &mut bool, out: &mut Vec<(String, PathBuf)>| {
            if section.starts_with("Profile") && !path.is_empty() {
                let dir = if *rel { root.join(&*path) } else { PathBuf::from(&*path) };
                if dir.exists() {
                    out.push((if name.is_empty() { path.clone() } else { name.clone() }, dir));
                }
            }
            name.clear();
            path.clear();
            *rel = true;
        };
        for l in text.lines() {
            let l = l.trim();
            if let Some(s) = l.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                flush(&section, &mut name, &mut path, &mut rel, &mut out);
                section = s.to_string();
            } else if let Some((k, v)) = l.split_once('=') {
                match k.trim() {
                    "Name" => name = v.trim().into(),
                    "Path" => path = v.trim().into(),
                    "IsRelative" => rel = v.trim() == "1",
                    _ => {}
                }
            }
        }
        flush(&section, &mut name, &mut path, &mut rel, &mut out);
    }
    out
}

fn firefox_perms(profile: &Path) -> Option<(u32, u32)> {
    let db = profile.join("permissions.sqlite");
    if !db.exists() {
        return None;
    }
    let uri = format!("file:{}?mode=ro&immutable=1", db.display());
    let con = rusqlite::Connection::open_with_flags(uri, rusqlite::OpenFlags::SQLITE_OPEN_URI | rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let mut st = con.prepare("select permission, count(*) from moz_perms where type='desktop-notification' group by permission").ok()?;
    let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))).ok()?;
    let (mut a, mut b) = (0, 0);
    for r in rows.flatten() {
        match r.0 {
            1 => a = r.1 as u32,
            2 => b = r.1 as u32,
            _ => {}
        }
    }
    Some((a, b))
}

pub fn collect() -> Apps {
    let cfg = config_dir();
    let mut chromium = Vec::new();
    for (name, rel, entry) in CHROMIUM_FAMILY {
        let base = cfg.join(rel);
        if !base.exists() {
            continue;
        }
        let running = exe_running(&running_needles(entry));
        chromium.push(match chromium_profile(&base) {
            None => Chromium { name: name.into(), profile: false, running, allowed: None, blocked: None, push: None, error: None },
            Some(Err(e)) => Chromium { name: name.into(), profile: false, running, allowed: None, blocked: None, push: None, error: Some(e) },
            Some(Ok((a, b, p))) => Chromium { name: name.into(), profile: true, running, allowed: Some(a), blocked: Some(b), push: Some(p), error: None },
        });
    }
    let mut flatpak = Vec::new();
    if let Ok(rd) = fs::read_dir(home().join(".var/app")) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.contains("Chrom") || n.contains("Brave") {
                flatpak.push(n);
            }
        }
    }
    flatpak.sort();
    let firefox = firefox_profiles().into_iter().map(|(name, d)| {
        let p = firefox_perms(&d);
        Firefox { profile: name, allowed: p.map(|x| x.0), blocked: p.map(|x| x.1) }
    }).collect();
    let electron = ELECTRON.iter().filter(|a| cfg.join(a).exists()).map(|s| s.to_string()).collect();
    Apps { chromium, flatpak_browsers: flatpak, firefox, electron }
}

pub fn run(ctx: &Ctx) -> Result<()> {
    let apps = collect();
    if ctx.out.json {
        ctx.out.document(&apps);
        return Ok(());
    }
    let o = &ctx.out;
    o.heading("Chromium-family browsers");
    for c in &apps.chromium {
        if let Some(e) = &c.error {
            o.line(WARN, &format!("{}: {e}", c.name), None);
        } else if !c.profile {
            o.line(INFO, &format!("{}: profile folder exists, no Default profile", c.name), None);
        } else {
            let (a, b, p) = (c.allowed.unwrap_or(0), c.blocked.unwrap_or(0), c.push.unwrap_or(false));
            o.line(
                if a > 0 { OK } else { INFO },
                &format!("{}{}: {a} sites allowed, {b} blocked, push (GCM) {}", c.name, if c.running { " (running)" } else { "" }, if p { "on" } else { "off/unset" }),
                None,
            );
            if c.name.starts_with("Brave") && !p {
                o.line(INFO, "web push needs Google push messaging enabled", Some("see: notify adapt"));
            }
        }
    }
    for n in &apps.flatpak_browsers {
        o.line(INFO, &format!("Flatpak browser: {n} (its notifications use the portal; see: notify portal)"), None);
    }
    o.heading("Firefox-family");
    for f in &apps.firefox {
        let extra = match (f.allowed, f.blocked) {
            (Some(a), Some(b)) => format!(", {a} sites allowed, {b} blocked"),
            _ => String::new(),
        };
        o.line(INFO, &format!("profile '{}'{extra} (web notifications carry no site name, so rules can only match their text)", f.profile), None);
    }
    if apps.firefox.is_empty() {
        o.line(INFO, "no Firefox profile found", None);
    }
    o.heading("Electron / native apps (use system notifications, nothing to adapt)");
    o.line(INFO, &if apps.electron.is_empty() { "none found".to_string() } else { apps.electron.join(", ") }, None);
    Ok(())
}
