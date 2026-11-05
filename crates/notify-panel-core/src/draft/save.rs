//! The rule simulator's input, and saving rules.json safely (refusing to overwrite a hand edit unless forced).

use super::*;

/// The notification the user describes in the simulator panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimInput {
    pub app_name: String,
    pub desktop_entry: String,
    pub origin: String,
    /// The app's category hint ("email.arrived"), or empty.
    pub category: String,
    pub summary: String,
    pub body: String,
    /// "low", "normal" or "critical".
    pub urgency: String,
    /// The app asked for no timeout (it wants the popup to stay).
    pub never_expires: bool,
    /// Ignore Do Not Disturb, mutes and full screen right now.
    pub ignore_state: bool,
}

impl Default for SimInput {
    fn default() -> Self {
        Self { app_name: String::new(), desktop_entry: String::new(), origin: String::new(), category: String::new(), summary: String::new(), body: String::new(), urgency: "normal".into(), never_expires: false, ignore_state: true }
    }
}

impl SimInput {
    /// Set one field by its UI name. Returns false for an unknown field or a bad value.
    pub fn set(&mut self, field: &str, value: &str) -> bool {
        match field {
            "appName" => self.app_name = value.into(),
            "desktopEntry" => self.desktop_entry = value.into(),
            "origin" => self.origin = value.into(),
            "category" => self.category = value.into(),
            "summary" => self.summary = value.into(),
            "body" => self.body = value.into(),
            "urgency" if ["low", "normal", "critical"].contains(&value) => self.urgency = value.into(),
            "timeout" if ["default", "never"].contains(&value) => self.never_expires = value == "never",
            "ignoreState" if ["true", "false"].contains(&value) => self.ignore_state = value == "true",
            _ => return false,
        }
        true
    }

    /// The `simulate` request for the daemon, evaluating against `config` (the draft).
    pub fn to_request(&self, config: &Value) -> Value {
        json!({
            "config": config,
            "ignoreState": self.ignore_state,
            "notification": {
                "appName": self.app_name, "desktopEntry": self.desktop_entry, "origin": self.origin, "category": self.category,
                "summary": self.summary, "body": self.body, "urgency": self.urgency,
                "expireTimeout": if self.never_expires { 0 } else { -1 }
            }
        })
    }
}

// ---------------------------------------------------------------- the file

/// Where rules.json lives: `$NOTIFD_RULES` if set (tests), otherwise `$XDG_CONFIG_HOME/notifd/rules.json`.
pub fn rules_path() -> PathBuf {
    if let Some(p) = std::env::var_os("NOTIFD_RULES").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let cfg = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into())).join(".config")
    });
    cfg.join("notifd/rules.json")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveOutcome {
    Saved,
    /// The file on disk differs from what the draft was started from (someone edited it); nothing was written.
    Conflict { disk: String },
    Failed(String),
}

/// Write `text` to `path`, atomically, unless the file changed since `expected_base` (then `Conflict`, unless `force`).
///
/// - Follows a symlink: the real file is replaced and the link is kept (the installed rules.json is reached through one).
/// - Keeps the previous contents next to it as `<file>.bak`.
pub fn save(path: &Path, text: &str, expected_base: &str, force: bool) -> SaveOutcome {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let disk = fs::read_to_string(&target).ok();
    if !force {
        if let Some(d) = &disk {
            if d != expected_base {
                return SaveOutcome::Conflict { disk: d.clone() };
            }
        }
    }
    let result = (|| -> std::io::Result<()> {
        let dir = target.parent().ok_or_else(|| std::io::Error::other("no parent directory"))?;
        fs::create_dir_all(dir)?;
        let mode = fs::metadata(&target).map(|m| m.permissions().mode() & 0o777).unwrap_or(0o644);
        if let Some(d) = &disk {
            let mut bak = target.clone().into_os_string();
            bak.push(".bak");
            fs::write(&bak, d)?;
        }
        let tmp = dir.join(format!(".{}.{}.tmp", target.file_name().and_then(|n| n.to_str()).unwrap_or("rules"), std::process::id()));
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
        }
        fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))?;
        if let Err(e) = fs::rename(&tmp, &target) {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
        Ok(())
    })();
    match result {
        Ok(()) => SaveOutcome::Saved,
        Err(e) => SaveOutcome::Failed(format!("could not write {}: {e}", target.display())),
    }
}
