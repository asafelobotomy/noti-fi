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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_disk() -> Vec<String> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../notifd");
        let mut v: Vec<String> = fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".qml") || n == "rules.schema.json").collect();
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

    fn which_qs() -> Option<String> {
        crate::sys::which("qs")
    }
}
