//! The sounds a user can pick: the event sounds of the installed sound themes (freedesktop names such as `message-new-instant`),
//! and playing one so it can be heard before it is chosen.

use std::path::PathBuf;
use std::pin::Pin;

use cxx_qt_lib::QString;
use serde_json::json;

use super::qobject;

/// Where sound themes live: the user's own first, then the system's.
fn theme_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share"))) {
        dirs.push(home.join("sounds"));
    }
    let system = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dirs.extend(system.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("sounds")));
    dirs
}

/// Every event sound file: (name, path), the name being the file name without its extension.
fn sound_files() -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for root in theme_dirs() {
        let Ok(themes) = std::fs::read_dir(&root) else { continue };
        for theme in themes.flatten() {
            let Ok(files) = std::fs::read_dir(theme.path().join("stereo")) else { continue };
            for f in files.flatten() {
                let p = f.path();
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                if !["oga", "ogg", "wav"].contains(&ext) {
                    continue;
                }
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    if !out.iter().any(|(n, _)| n == stem) {
                        out.push((stem.to_string(), p.clone()));
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn on_path(cmd: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file()))
}

impl qobject::PanelModel {
    /// The names of the installed event sounds, sorted, as a JSON array.
    pub fn sound_names(self: Pin<&mut Self>) -> QString {
        let names: Vec<String> = sound_files().into_iter().map(|(n, _)| n).collect();
        QString::from(&json!(names).to_string())
    }

    /// Play one sound by name ("" = the default notification sound), in the background, the way notifd would.
    pub fn play_sound(self: Pin<&mut Self>, name: &QString) {
        let name = name.to_string();
        let name = if name.is_empty() { "message-new-instant".to_string() } else { name };
        // only names a theme could hold: no paths, no shell
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') {
            return;
        }
        std::thread::spawn(move || {
            if on_path("canberra-gtk-play") {
                let _ = std::process::Command::new("canberra-gtk-play").args(["-i", &name]).status();
                return;
            }
            if let Some((_, path)) = sound_files().into_iter().find(|(n, _)| *n == name) {
                for player in ["pw-play", "paplay"] {
                    if on_path(player) {
                        let _ = std::process::Command::new(player).arg(&path).status();
                        return;
                    }
                }
            }
        });
    }
}
