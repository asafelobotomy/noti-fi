//! Naming the app behind a job from its .desktop file, as Plasma does: "org.kde.dolphin" -> "Dolphin", icon "system-file-manager".

use notify_apps::{Roots, desktop};

use crate::model::Job;

fn launcher(id: &str) -> Option<String> {
    desktop::find(&Roots::system(), id)
}

/// `Name=` and `Icon=` from the [Desktop Entry] group (the untranslated values).
pub fn name_and_icon(text: &str) -> (String, String) {
    let k = desktop::entry_keys(text);
    (k.get("Name").cloned().unwrap_or_default(), k.get("Icon").cloned().unwrap_or_default())
}

/// Fill in the app's name and icon from its desktop entry where the app did not send them.
pub fn fill(job: &mut Job) {
    if !job.app_name.is_empty() && !job.app_icon.is_empty() {
        return;
    }
    let Some(text) = launcher(&job.desktop_entry) else { return };
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
    [name.to_string(), format!("org.kde.{name}")].into_iter().find(|id| launcher(id).is_some())
}
