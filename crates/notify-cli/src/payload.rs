//! The daemon's files, embedded in this binary so `notify update` installs exactly the version
//! this binary was built with (no project folder needed). A directory can override them for development.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::sys::atomic_write;

pub const SHELL_QML: &str = include_str!("../../../notifd/shell.qml");
pub const POPUP_QML: &str = include_str!("../../../notifd/Popup.qml");
pub const MEDIA_QML: &str = include_str!("../../../notifd/Media.qml");
pub const CHART_QML: &str = include_str!("../../../notifd/TransferChart.qml");
pub const VALIDATE_QML: &str = include_str!("../../../notifd/Validate.qml");
pub const SCHEMA: &str = include_str!("../../../notifd/rules.schema.json");
pub const DEFAULT_RULES: &str = include_str!("../../../notifd/rules.json");

/// The files that make up the daemon (not the user's rules.json).
pub const FILE_NAMES: [&str; 6] = ["shell.qml", "Popup.qml", "Media.qml", "TransferChart.qml", "Validate.qml", "rules.schema.json"];

pub struct Payload {
    pub files: Vec<(String, String)>,
    pub rules: String,
    pub origin: String,
}

impl Payload {
    pub fn embedded() -> Payload {
        Payload {
            files: vec![
                ("shell.qml".into(), SHELL_QML.into()),
                ("Popup.qml".into(), POPUP_QML.into()),
                ("Media.qml".into(), MEDIA_QML.into()),
                ("TransferChart.qml".into(), CHART_QML.into()),
                ("Validate.qml".into(), VALIDATE_QML.into()),
                ("rules.schema.json".into(), SCHEMA.into()),
            ],
            rules: DEFAULT_RULES.into(),
            origin: "embedded in this binary".into(),
        }
    }

    pub fn from_dir(dir: &Path) -> Result<Payload> {
        let mut files = Vec::new();
        for n in FILE_NAMES {
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
