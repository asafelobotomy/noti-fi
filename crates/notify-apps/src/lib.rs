//! Which apps send notifications, and which could but never have.
//!
//! Three kinds of evidence are combined (nothing is read from inside an app's own data):
//! - **sent**: the daemon's registry of apps that sent it a notification (`seenApps`), plus Plasma's own list of apps it has seen;
//! - **capable**: installed apps that declare notifications (`X-GNOME-UsesNotifications`, messaging/email categories, KDE `.notifyrc` files,
//!   Flatpak apps with a notification permission);
//! - **Flatpak permission**: the portal's per-app yes/no, which can be changed with [`set_flatpak_permission`].
//!
//! Every path comes from [`Roots`], so tests run against fixture folders.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod desktop;
mod group;

use notify_protocol::{SeenApp, SeenType};
use serde::Serialize;

/// Where to look. [`Roots::system`] is the real machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Roots {
    /// Folders that contain `applications/*.desktop` and `knotifications6/*.notifyrc` (XDG data dirs, Flatpak exports).
    pub data_dirs: Vec<PathBuf>,
    /// The folder holding `plasmanotifyrc`.
    pub config_dir: PathBuf,
    /// Ask `flatpak` for permissions and installed apps (off in tests).
    pub flatpak: bool,
}

impl Roots {
    pub fn system() -> Roots {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        let env_dir = |k: &str, fallback: PathBuf| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or(fallback);
        let mut dirs = vec![env_dir("XDG_DATA_HOME", home.join(".local/share"))];
        match std::env::var("XDG_DATA_DIRS") {
            Ok(v) if !v.is_empty() => dirs.extend(v.split(':').filter(|p| !p.is_empty()).map(PathBuf::from)),
            _ => dirs.extend(["/usr/local/share".into(), "/usr/share".into()]),
        }
        // NOTIFY_APPS_ISOLATED=1 (tests): only the folders named by XDG_*, and never ask flatpak
        let isolated = std::env::var_os("NOTIFY_APPS_ISOLATED").is_some_and(|v| v == "1");
        if !isolated {
            dirs.push("/var/lib/flatpak/exports/share".into());
            dirs.push(home.join(".local/share/flatpak/exports/share"));
        }
        let mut seen = std::collections::BTreeSet::new();
        dirs.retain(|d| seen.insert(d.clone()));
        Roots { data_dirs: dirs, config_dir: env_dir("XDG_CONFIG_HOME", home.join(".config")), flatpak: !isolated }
    }
}

/// Why an app is thought able to send notifications.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// Its launcher says it uses notifications.
    Declared,
    /// A messaging, chat, email or feed app.
    Category,
    /// A KDE app with its own notification events.
    Kde,
    /// Plasma has seen notifications from it before.
    Plasma,
    /// A Flatpak app with a notification permission entry.
    Flatpak,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AppEntry {
    /// Stable id: the desktop entry (without `.desktop`), lower-cased; or the app name when nothing better is known.
    pub id: String,
    /// The system category it is listed under (from its launcher; "System" for KDE/Plasma notifiers; otherwise "Other").
    pub group: String,
    pub name: String,
    pub desktop_entry: String,
    pub icon: String,
    /// Has sent a notification to notify.
    pub sent: bool,
    pub count: u64,
    /// Milliseconds since the epoch; 0 = never.
    pub last: i64,
    /// Could send notifications (see [`Reason`]); an app that has sent one is always capable.
    pub capable: bool,
    pub reasons: Vec<Reason>,
    pub flatpak: bool,
    /// The Flatpak portal permission: `Some(true)` allowed, `Some(false)` denied, `None` = not set (or not a Flatpak).
    pub flatpak_permission: Option<bool>,
    /// How a notification from this app is recognised by a rule: `desktopEntry` or `appName`, and the exact text to match.
    pub match_field: String,
    pub match_value: String,
    /// The kinds of notification it has sent (category + channel + what they can do), most used first.
    pub types: Vec<SeenType>,
}

#[derive(Debug, Default, Clone)]
struct Launcher {
    id: String,
    name: String,
    icon: String,
    declared: bool,
    category: bool,
    categories: String,
    flatpak: bool,
}

/// Ids are compared lower-case. (An id such as `org.telegram.desktop` legitimately ends in "desktop": never strip it.)
fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

/// `[Desktop Entry]` keys of one launcher, or None when it is hidden or not an application.
fn parse_desktop(path: &Path, flatpak_export: bool) -> Option<Launcher> {
    let keys = desktop::entry_keys(&fs::read_to_string(path).ok()?);
    let get = |k: &str| keys.get(k).cloned().unwrap_or_default();
    let (name, icon, categories) = (get("Name"), get("Icon"), get("Categories"));
    let hidden = get("NoDisplay") == "true" || get("Hidden") == "true";
    let declared = get("X-GNOME-UsesNotifications") == "true";
    let category = categories.split(';').any(|c| ["InstantMessaging", "Chat", "Email", "IRCClient", "News", "Feed", "VideoConference"].contains(&c.trim()));
    if get("Type") != "Application" || hidden || name.is_empty() {
        return None;
    }
    let id = norm(path.file_name()?.to_str()?.strip_suffix(".desktop")?);
    Some(Launcher { id, name, icon, declared, category, categories, flatpak: flatpak_export })
}

/// The `[Global]` name, icon and desktop entry of a KDE `.notifyrc`.
fn parse_notifyrc(path: &Path) -> Option<(String, String, String)> {
    let text = fs::read_to_string(path).ok()?;
    let (mut inside, mut name, mut icon, mut entry) = (false, String::new(), String::new(), String::new());
    for l in text.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            inside = l == "[Global]";
            continue;
        }
        if !inside {
            continue;
        }
        if let Some((k, v)) = l.split_once('=') {
            match k.trim() {
                "Name" => name = v.trim().into(),
                "IconName" => icon = v.trim().into(),
                "DesktopEntry" => entry = v.trim().into(),
                _ => {}
            }
        }
    }
    let stem = path.file_stem()?.to_str()?.to_string();
    Some((if entry.is_empty() { stem } else { entry }, name, icon))
}

/// Apps listed in Plasma's `plasmanotifyrc` (`[Applications][id]`): Plasma has seen notifications from them.
fn plasma_apps(config_dir: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(config_dir.join("plasmanotifyrc")) else { return Vec::new() };
    text.lines().filter_map(|l| l.trim().strip_prefix("[Applications][")?.strip_suffix(']').map(String::from)).collect()
}

/// Flatpak's notification permissions: app id -> allowed.
fn flatpak_permissions() -> BTreeMap<String, bool> {
    let mut out = BTreeMap::new();
    let Ok(o) = Command::new("flatpak").args(["permissions", "notifications"]).output() else { return out };
    for l in String::from_utf8_lossy(&o.stdout).lines() {
        // table, object, app, permissions, data
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() >= 4 && f[0] == "notifications" {
            out.insert(norm(f[2]), f[3] == "yes");
        }
    }
    out
}

fn flatpak_installed() -> Vec<(String, String)> {
    let Ok(o) = Command::new("flatpak").args(["list", "--app", "--columns=application,name"]).output() else { return Vec::new() };
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|l| {
            let (id, name) = l.split_once('\t')?;
            Some((id.trim().to_string(), name.trim().to_string()))
        })
        .collect()
}

/// Everything found on disk, before it is merged with what the daemon has seen.
#[derive(Debug, Default, Clone)]
pub struct Discovery {
    launchers: Vec<Launcher>,
    notifyrc: Vec<(String, String, String)>,
    plasma: Vec<String>,
    permissions: BTreeMap<String, bool>,
    flatpaks: Vec<(String, String)>,
}

pub fn discover(roots: &Roots) -> Discovery {
    let mut d = Discovery::default();
    let mut seen_ids = std::collections::BTreeSet::new();
    for dir in &roots.data_dirs {
        let flatpak_export = dir.to_string_lossy().contains("flatpak/exports");
        if let Ok(rd) = fs::read_dir(dir.join("applications")) {
            let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "desktop")).collect();
            files.sort();
            for f in files {
                if let Some(l) = parse_desktop(&f, flatpak_export) {
                    if seen_ids.insert(l.id.clone()) {
                        d.launchers.push(l);
                    }
                }
            }
        }
        if let Ok(rd) = fs::read_dir(dir.join("knotifications6")) {
            let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "notifyrc")).collect();
            files.sort();
            d.notifyrc.extend(files.iter().filter_map(|f| parse_notifyrc(f)));
        }
    }
    d.plasma = plasma_apps(&roots.config_dir);
    if roots.flatpak {
        d.permissions = flatpak_permissions();
        d.flatpaks = flatpak_installed();
    }
    d
}

/// Combine what the daemon has seen with what was discovered. Sorted: apps that sent notifications (newest first),
/// then apps that could but never did, by name.
pub fn merge(seen: &[SeenApp], disc: &Discovery) -> Vec<AppEntry> {
    let mut apps: BTreeMap<String, AppEntry> = BTreeMap::new();
    let blank = |id: &str, name: &str| AppEntry {
        id: id.to_string(),
        group: String::new(),
        name: if name.is_empty() { id.to_string() } else { name.to_string() },
        desktop_entry: String::new(),
        icon: String::new(),
        sent: false,
        count: 0,
        last: 0,
        capable: false,
        reasons: Vec::new(),
        flatpak: false,
        flatpak_permission: None,
        match_field: "desktopEntry".to_string(),
        match_value: id.to_string(),
        types: Vec::new(),
    };
    let add_reason = |e: &mut AppEntry, r: Reason| {
        e.capable = true;
        if !e.reasons.contains(&r) {
            e.reasons.push(r);
        }
    };

    for l in &disc.launchers {
        let flatpak = l.flatpak || disc.permissions.contains_key(&l.id) || disc.flatpaks.iter().any(|(i, _)| norm(i) == l.id);
        let mut e = blank(&l.id, &l.name);
        e.desktop_entry = l.id.clone();
        e.icon = l.icon.clone();
        e.flatpak = flatpak;
        e.group = group::group_of(&l.categories).to_string();
        if l.declared {
            add_reason(&mut e, Reason::Declared);
        }
        if l.category {
            add_reason(&mut e, Reason::Category);
        }
        apps.insert(l.id.clone(), e);
    }
    // Flatpak apps (by id), whether or not they exported a launcher we saw
    for (id, name) in &disc.flatpaks {
        let key = norm(id);
        let e = apps.entry(key.clone()).or_insert_with(|| blank(&key, name));
        e.flatpak = true;
        if e.desktop_entry.is_empty() {
            e.desktop_entry = key.clone();
        }
    }
    for (key, allowed) in &disc.permissions {
        let e = apps.entry(key.clone()).or_insert_with(|| blank(key, key));
        e.flatpak = true;
        e.flatpak_permission = Some(*allowed);
        add_reason(e, Reason::Flatpak);
    }
    for (entry, name, icon) in &disc.notifyrc {
        let key = norm(entry);
        let e = apps.entry(key.clone()).or_insert_with(|| blank(&key, name));
        if e.icon.is_empty() {
            e.icon = icon.clone();
        }
        if e.desktop_entry.is_empty() {
            e.desktop_entry = key.clone();
        }
        add_reason(e, Reason::Kde);
    }
    for id in &disc.plasma {
        let key = norm(id);
        let e = apps.entry(key.clone()).or_insert_with(|| blank(&key, id));
        if e.desktop_entry.is_empty() {
            e.desktop_entry = key.clone();
        }
        add_reason(e, Reason::Plasma);
    }

    // what the daemon has seen: match by desktop entry, then by name
    for s in seen {
        let by_entry = (!s.desktop_entry.is_empty()).then(|| norm(&s.desktop_entry)).filter(|k| apps.contains_key(k));
        let by_name = || apps.iter().find(|(_, e)| e.name.eq_ignore_ascii_case(&s.app) || e.id == norm(&s.app)).map(|(k, _)| k.clone());
        let key = by_entry.or_else(by_name).unwrap_or_else(|| {
            let k = norm(if s.desktop_entry.is_empty() { &s.app } else { &s.desktop_entry });
            apps.entry(k.clone()).or_insert_with(|| blank(&k, &s.app));
            k
        });
        let e = apps.get_mut(&key).expect("present");
        e.sent = true;
        e.capable = true;
        e.count += s.count;
        e.last = e.last.max(s.last);
        if e.desktop_entry.is_empty() {
            e.desktop_entry = s.desktop_entry.clone();
        }
        if !s.app.is_empty() && e.name == e.id {
            e.name = s.app.clone();
        }
        for t in &s.types {
            match e.types.iter_mut().find(|x| x.key == t.key) {
                Some(x) => {
                    x.count += t.count;
                    x.last = x.last.max(t.last);
                    for c in &t.caps {
                        if !x.caps.contains(c) {
                            x.caps.push(c.clone());
                        }
                    }
                }
                None => e.types.push(t.clone()),
            }
        }
        e.types.sort_by(|a, b| b.count.cmp(&a.count).then(a.key.cmp(&b.key)));
        // a rule must match the way this app actually identifies itself
        if !s.desktop_entry.is_empty() {
            e.match_field = "desktopEntry".into();
            e.match_value = s.desktop_entry.clone();
        } else if !s.app.is_empty() {
            e.match_field = "appName".into();
            e.match_value = s.app.clone();
        }
    }

    let mut out: Vec<AppEntry> = apps.into_values().filter(|e| e.sent || e.capable).collect();
    for e in &mut out {
        if e.group.is_empty() {
            e.group = if e.reasons.iter().any(|r| matches!(r, Reason::Kde | Reason::Plasma)) { "System" } else { "Other" }.to_string();
        }
    }
    out.sort_by(|a, b| b.sent.cmp(&a.sent).then(b.last.cmp(&a.last)).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

/// Allow (`Some(true)`), deny (`Some(false)`) or forget (`None`) an app's notification permission in the Flatpak portal.
pub fn set_flatpak_permission(app_id: &str, allow: Option<bool>) -> Result<(), String> {
    if !app_id.starts_with(|c: char| c.is_ascii_alphanumeric()) || !app_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-') {
        return Err(format!("'{app_id}' is not a valid app id"));
    }
    let run = |args: &[&str]| -> Result<(), String> {
        let o = Command::new("flatpak").args(args).output().map_err(|e| format!("cannot run flatpak: {e}"))?;
        if o.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&o.stderr).trim().to_string()) }
    };
    match allow {
        Some(yes) => run(&["permission-set", "notifications", "notification", app_id, if yes { "yes" } else { "no" }]),
        None => run(&["permission-reset", app_id]),
    }
}

#[cfg(test)]
mod tests;
