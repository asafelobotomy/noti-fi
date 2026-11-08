//! Naming the app behind a job from its .desktop file, as Plasma does: "org.kde.dolphin" -> "Dolphin", icon "system-file-manager".

use std::fs;
use std::path::PathBuf;

use crate::model::Job;

fn app_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    match std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        Some(d) => dirs.push(PathBuf::from(d)),
        None => {
            if let Some(h) = std::env::var_os("HOME") {
                dirs.push(PathBuf::from(h).join(".local/share"));
            }
        }
    }
    let sys = std::env::var("XDG_DATA_DIRS").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(sys.split(':').filter(|d| !d.is_empty()).map(PathBuf::from));
    dirs.into_iter().map(|d| d.join("applications")).collect()
}

/// A desktop entry id is a file name: refuse anything that could leave the applications folder.
fn safe(id: &str) -> bool {
    !id.is_empty() && id.len() <= 200 && !id.starts_with('.') && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

fn desktop_file(id: &str) -> Option<String> {
    if !safe(id) {
        return None;
    }
    app_dirs().into_iter().find_map(|d| fs::read_to_string(d.join(format!("{id}.desktop"))).ok())
}

/// `Name=` and `Icon=` from the [Desktop Entry] group (the untranslated values).
pub fn name_and_icon(text: &str) -> (String, String) {
    let (mut name, mut icon) = (String::new(), String::new());
    let mut in_main = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_main = line == "[Desktop Entry]";
            continue;
        }
        if !in_main {
            continue;
        }
        if let Some(v) = line.strip_prefix("Name=") {
            name = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Icon=") {
            icon = v.trim().to_string();
        }
    }
    (name, icon)
}

/// Fill in the app's name and icon from its desktop entry where the app did not send them.
pub fn fill(job: &mut Job) {
    if !job.app_name.is_empty() && !job.app_icon.is_empty() {
        return;
    }
    let Some(text) = desktop_file(&job.desktop_entry) else { return };
    let (name, icon) = name_and_icon(&text);
    if job.app_name.is_empty() {
        job.app_name = name;
    }
    if job.app_icon.is_empty() {
        job.app_icon = icon;
    }
}

/// The desktop entry for an old-style (V1) app name: "dolphin" -> "org.kde.dolphin".
pub fn entry_for_name(name: &str) -> Option<String> {
    [name.to_string(), format!("org.kde.{name}")].into_iter().find(|id| desktop_file(id).is_some())
}
