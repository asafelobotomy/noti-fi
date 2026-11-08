//! Reading launchers (.desktop files): the keys of their `[Desktop Entry]` group, and finding one by its id.

use std::collections::BTreeMap;
use std::fs;

use crate::Roots;

/// The `[Desktop Entry]` keys of a launcher (untranslated values, trimmed). Other groups (`[Desktop Action ...]`) are skipped.
pub fn entry_keys(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut inside = false;
    for l in text.lines() {
        let l = l.trim();
        if l.starts_with('[') {
            inside = l == "[Desktop Entry]";
            continue;
        }
        if !inside {
            continue;
        }
        if let Some((k, v)) = l.split_once('=') {
            out.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    out
}

/// A desktop entry id is a file name: refuse anything that could leave the applications folder.
fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 200 && !id.starts_with('.') && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// The text of the launcher `<id>.desktop`, from the first data folder that has it (user folder first, as XDG says).
pub fn find(roots: &Roots, id: &str) -> Option<String> {
    if !safe_id(id) {
        return None;
    }
    roots.data_dirs.iter().find_map(|d| fs::read_to_string(d.join("applications").join(format!("{id}.desktop"))).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_come_from_the_main_group_only() {
        let k = entry_keys("[Desktop Entry]\nName=Dolphin\nIcon = system-file-manager\n[Desktop Action new]\nName=New Window\n");
        assert_eq!(k.get("Name").map(String::as_str), Some("Dolphin"));
        assert_eq!(k.get("Icon").map(String::as_str), Some("system-file-manager"));
    }

    #[test]
    fn ids_cannot_leave_the_applications_folder() {
        let roots = Roots { data_dirs: vec!["/nonexistent".into()], ..Default::default() };
        for bad in ["", "../etc/passwd", ".hidden", "a/b"] {
            assert!(find(&roots, bad).is_none(), "{bad}");
        }
        assert!(safe_id("org.kde.dolphin"));
    }
}
