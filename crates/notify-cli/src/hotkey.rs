//! Global shortcuts through KDE's shortcut service: one toggles Do Not Disturb, one opens the notification center.
//! Each is a hidden launcher .desktop file registered with kglobalaccel, the way System Settings' "Add Command" does it.

use std::fs;

use anyhow::{Result, anyhow, bail};

use crate::ctx::Ctx;
use crate::install::exe_path;
use crate::paths::paths;
use crate::sys::{atomic_write, sh};

/// What a shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Dnd,
    Center,
}

impl Target {
    /// The launcher's file name, which is also its id in KDE's shortcut service.
    pub fn desktop_file(self) -> &'static str {
        match self {
            Target::Dnd => "notify-dnd-toggle.desktop",
            Target::Center => "notify-center-toggle.desktop",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Target::Dnd => "Toggle Do Not Disturb (noti-fi)",
            Target::Center => "Show notifications (noti-fi)",
        }
    }
    /// The arguments to `notify` it runs.
    pub fn args(self) -> &'static str {
        match self {
            Target::Dnd => "dnd toggle",
            Target::Center => "center toggle",
        }
    }
    pub fn default_keys(self) -> &'static str {
        match self {
            Target::Dnd => "Meta+Shift+N",
            Target::Center => "Meta+N",
        }
    }
    fn what(self) -> &'static str {
        match self {
            Target::Dnd => "toggle Do Not Disturb",
            Target::Center => "open or close the notification list",
        }
    }
}

/// "Meta+Shift+N" -> the integer Qt (and KDE's shortcut service) uses.
pub fn qt_keycode(combo: &str) -> Result<i64> {
    let mut code: i64 = 0;
    for part in combo.split('+') {
        let k = part.trim().to_lowercase();
        code |= match k.as_str() {
            "meta" | "super" => 0x1000_0000,
            "shift" => 0x0200_0000,
            "ctrl" | "control" => 0x0400_0000,
            "alt" => 0x0800_0000,
            s if s.len() == 1 && s.chars().all(|c| c.is_ascii_alphanumeric()) => s.to_uppercase().chars().next().unwrap() as i64,
            s if s.starts_with('f') && s[1..].parse::<i64>().is_ok_and(|n| (1..=12).contains(&n)) => 0x0100_0030 + s[1..].parse::<i64>().unwrap_or(1) - 1,
            _ => bail!("unsupported key '{}'", part),
        };
    }
    if code & 0x00FF_FFFF == 0 {
        bail!("needs a main key, like Meta+Shift+N");
    }
    Ok(code)
}

/// The launcher file for a shortcut.
pub fn desktop_entry(t: Target, exe: &str) -> String {
    format!("[Desktop Entry]\nType=Application\nName={}\nExec={exe} {}\nNoDisplay=true\nX-KDE-GlobalAccel-CommandShortcut=true\n", t.name(), t.args())
}

fn kga(args: &[&str]) -> (i32, String) {
    let mut a = vec!["--user", "call", "org.kde.kglobalaccel", "/kglobalaccel", "org.kde.KGlobalAccel"];
    a.extend_from_slice(args);
    sh("busctl", &a)
}

pub fn hotkey(ctx: &Ctx, t: Target, keys: Option<&str>, remove: bool, dry: bool) -> Result<()> {
    let id = t.desktop_file();
    let desktop = paths().applications.join(id);
    if remove {
        kga(&["unregister", "ss", id, "_launch"]);
        if desktop.exists() {
            fs::remove_file(&desktop)?;
        }
        ctx.out.result(true, "hotkey removed");
        return Ok(());
    }
    let keys = keys.unwrap_or(t.default_keys());
    let code = qt_keycode(keys).map_err(|e| anyhow!("{e}"))?;
    println!("{keys} -> {} {}", exe_path(), t.args());
    if dry {
        ctx.out.result(true, "\nDry run only; nothing was changed.");
        return Ok(());
    }
    atomic_write(&desktop, &desktop_entry(t, &exe_path()))?;
    sh("kbuildsycoca6", &[]);
    let (rc, _) = kga(&["doRegister", "as", "4", id, "_launch", "", t.name()]);
    let (rc2, _) = kga(&["setShortcut", "asaiu", "4", id, "_launch", "", t.name(), "1", &code.to_string(), "3"]);
    if rc != 0 || rc2 != 0 {
        bail!("could not register with KDE's shortcut service; set it in System Settings -> Shortcuts -> Add Command instead");
    }
    ctx.out.result(true, &format!("registered. Press it to {}. Change it in System Settings -> Shortcuts if you like.", t.what()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_codes_match_qt() {
        assert_eq!(qt_keycode("Meta+Shift+N").unwrap(), 301_989_966); // the value KDE stored for the real shortcut
        assert_eq!(qt_keycode("ctrl+alt+F5").unwrap(), 0x0400_0000 | 0x0800_0000 | (0x0100_0030 + 4));
        assert!(qt_keycode("Bogus+Q").is_err());
        assert!(qt_keycode("Meta+Shift").is_err());
    }

    #[test]
    fn each_shortcut_has_its_own_launcher() {
        assert_ne!(Target::Dnd.desktop_file(), Target::Center.desktop_file());
        let e = desktop_entry(Target::Center, "/usr/bin/notify");
        assert!(e.contains("Exec=/usr/bin/notify center toggle\n") && e.contains("X-KDE-GlobalAccel-CommandShortcut=true"));
        assert!(desktop_entry(Target::Dnd, "notify").contains("Exec=notify dnd toggle\n"));
        assert_eq!(qt_keycode(Target::Center.default_keys()).unwrap(), 0x1000_0000 | 'N' as i64);
    }
}
