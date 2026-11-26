//! The daemon's files, embedded in this binary so `notify update` installs exactly the version
//! this binary was built with (no project folder needed). A directory can override them for development.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::sys::atomic_write;

include!(concat!(env!("OUT_DIR"), "/payload_files.rs"));

pub const SCHEMA: &str = include_str!("../../../notifd/rules.schema.json");
pub const DEFAULT_RULES: &str = include_str!("../../../notifd/rules.json");

/// The names of the files that make up the daemon (not the user's rules.json).
pub fn file_names() -> impl Iterator<Item = &'static str> {
    FILES.iter().map(|(n, _)| *n)
}

pub struct Payload {
    pub files: Vec<(String, String)>,
    pub rules: String,
    pub origin: String,
}

impl Payload {
    pub fn embedded() -> Payload {
        Payload {
            files: FILES.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect(),
            rules: DEFAULT_RULES.into(),
            origin: "embedded in this binary".into(),
        }
    }

    pub fn from_dir(dir: &Path) -> Result<Payload> {
        let mut files = Vec::new();
        for n in file_names() {
            let t = fs::read_to_string(dir.join(n)).with_context(|| format!("reading {}", dir.join(n).display()))?;
            files.push((n.to_string(), t));
        }
        let rules = fs::read_to_string(dir.join("rules.json")).unwrap_or_else(|_| DEFAULT_RULES.to_string());
        Ok(Payload { files, rules, origin: dir.display().to_string() })
    }

    pub fn load(override_dir: Option<&Path>) -> Result<Payload> {
        match override_dir {
            Some(d) => Payload::from_dir(d),
            None => Ok(Payload::embedded()),
        }
    }

    pub fn write_files(&self, dir: &Path) -> Result<()> {
        for (n, t) in &self.files {
            atomic_write(&dir.join(n), t)?;
        }
        // which noti-fi put these files here, so a later version knows they are not its own (see `refresh_installed`)
        atomic_write(&dir.join(STAMP), &format!("{CURRENT}\n"))
    }
}

/// In the daemon folder: the version of noti-fi that last wrote its files.
pub const STAMP: &str = ".noti-fi-version";
pub use notify_update::CURRENT;

/// The version recorded in `dir` (None: written by a noti-fi older than 0.21.10, or never).
pub fn stamped_version(dir: &Path) -> Option<String> {
    fs::read_to_string(dir.join(STAMP)).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// The daemon files in `dir` belong to another noti-fi version and differ from this binary's.
/// A package upgrade (pacman, AUR) replaces `notify` but not these files, so without this check notifd kept
/// running the files of the version that first installed it. Files this version wrote itself, such as a
/// checkout deployed with `notify update --payload`, are left alone.
pub fn is_stale(dir: &Path) -> bool {
    if stamped_version(dir).as_deref() == Some(CURRENT) {
        return false;
    }
    FILES.iter().any(|(n, t)| fs::read_to_string(dir.join(n)).ok().as_deref() != Some(*t))
}

/// If the installed daemon files are stale, keep them in `prev` (as `notify update` does) and put this binary's in
/// their place. Returns the version they came from ("an older version" when unknown).
pub fn refresh_installed(dir: &Path, prev: &Path) -> Result<Option<String>> {
    if !dir.join("shell.qml").exists() || !is_stale(dir) {
        return Ok(None);
    }
    let from = stamped_version(dir).unwrap_or_else(|| "an older version".into());
    fs::create_dir_all(prev)?;
    for n in file_names() {
        if dir.join(n).exists() {
            fs::copy(dir.join(n), prev.join(n))?;
        }
    }
    Payload::embedded().write_files(dir)?;
    Ok(Some(from))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_disk() -> Vec<String> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../notifd");
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".qml") || n == "rules.schema.json")
            .collect();
        v.sort();
        v
    }

    #[test]
    fn every_daemon_file_in_the_folder_is_embedded() {
        let mut embedded: Vec<String> = file_names().map(String::from).collect();
        embedded.sort();
        assert_eq!(embedded, on_disk(), "a file in notifd/ is missing from the binary (it would be missing from `notify update` and the release archive)");
        assert!(Payload::embedded().files.iter().all(|(_, t)| !t.is_empty()));
    }

    #[test]
    fn the_embedded_daemon_starts_on_a_private_bus() {
        // what `notify update` checks before it touches the live daemon; skipped where Quickshell is not installed (for example CI)
        if which_qs().is_none() {
            return;
        }
        let pl = Payload::embedded();
        assert!(crate::install::preflight(&pl, &pl.rules), "the daemon built from the embedded files did not start: a file is missing or broken");
    }

    #[test]
    fn stale_files_are_replaced_and_current_ones_kept() {
        let tmp = std::env::temp_dir().join(format!("notify-payload-test-{}", std::process::id()));
        let (dir, prev) = (tmp.join("notifd"), tmp.join("notifd.prev"));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&dir).unwrap();
        // an install from before the stamp existed, with an old shell.qml
        Payload::embedded().write_files(&dir).unwrap();
        fs::remove_file(dir.join(STAMP)).unwrap();
        fs::write(dir.join("shell.qml"), "old").unwrap();
        assert!(is_stale(&dir));
        assert_eq!(refresh_installed(&dir, &prev).unwrap().as_deref(), Some("an older version"));
        assert_eq!(fs::read_to_string(prev.join("shell.qml")).unwrap(), "old");
        assert!(!is_stale(&dir));
        assert_eq!(stamped_version(&dir).as_deref(), Some(CURRENT));
        // files this version wrote on purpose (a checkout deployed with `notify update --payload`) are kept
        fs::write(dir.join("shell.qml"), "from a checkout").unwrap();
        assert!(!is_stale(&dir));
        assert_eq!(refresh_installed(&dir, &prev).unwrap(), None);
        // ...until another version of noti-fi starts on them
        fs::write(dir.join(STAMP), "0.0.1\n").unwrap();
        assert_eq!(refresh_installed(&dir, &prev).unwrap().as_deref(), Some("0.0.1"));
        assert_ne!(fs::read_to_string(dir.join("shell.qml")).unwrap(), "from a checkout");
        let _ = fs::remove_dir_all(&tmp);
    }

    fn which_qs() -> Option<String> {
        crate::sys::which("qs")
    }
}
