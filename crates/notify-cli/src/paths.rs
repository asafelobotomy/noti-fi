//! Where everything lives. All locations honour XDG variables so tests can run in a fake home.

use std::path::PathBuf;

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

pub fn xdg(var: &str, default: &str) -> PathBuf {
    match std::env::var_os(var).filter(|v| !v.is_empty()) {
        Some(v) => PathBuf::from(v),
        None => home().join(default),
    }
}

pub struct Paths {
    pub unit: PathBuf,
    pub fallback_unit: PathBuf,
    pub jobs_unit: PathBuf,
    pub dbus: PathBuf,
    pub payload: PathBuf,
    pub prev: PathBuf,
    pub rules: PathBuf,
    pub portal: PathBuf,
    pub portal_system: PathBuf,
    pub takeover_state: PathBuf,
    pub dev_sock: PathBuf,
    pub dev_bus: PathBuf,
    pub dev_state: PathBuf,
    pub dev_payload: PathBuf,
    pub backups: PathBuf,
    /// Exists while `notify tray hide` has hidden Plasma's notification icon.
    pub tray_marker: PathBuf,
    pub kwin_script: PathBuf,
    pub autostart: PathBuf,
    pub hotkey_desktop: PathBuf,
}

pub fn paths() -> Paths {
    let config = xdg("XDG_CONFIG_HOME", ".config");
    let data = xdg("XDG_DATA_HOME", ".local/share");
    let state_home = xdg("XDG_STATE_HOME", ".local/state");
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("XDG_RUNTIME_DIR-is-not-set"));
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").ok().and_then(|d| d.split(':').next().map(str::to_lowercase)).filter(|d| !d.is_empty()).unwrap_or_else(|| "kde".into());
    Paths {
        unit: config.join("systemd/user/notifd.service"),
        fallback_unit: config.join("systemd/user/notifd-fallback.service"),
        jobs_unit: config.join("systemd/user/notifd-jobs.service"),
        dbus: data.join("dbus-1/services/org.freedesktop.Notifications.service"),
        payload: data.join("notifd"),
        prev: data.join("notifd.prev"),
        rules: config.join("notifd/rules.json"),
        portal: config.join(format!("xdg-desktop-portal/{desktop}-portals.conf")),
        portal_system: PathBuf::from(format!("/usr/share/xdg-desktop-portal/{desktop}-portals.conf")),
        takeover_state: state_home.join("notifd/takeover.json"),
        dev_sock: runtime.join("notifd-dev.sock"),
        dev_bus: state_home.join("notify/dev.bus"),
        dev_state: state_home.join("notify/dev-state"),
        dev_payload: state_home.join("notify/dev-payload"),
        backups: state_home.join("notify/backups"),
        tray_marker: state_home.join("notify/tray-hidden"),
        kwin_script: data.join("kwin/scripts/notifd-autodnd"),
        autostart: config.join("autostart"),
        hotkey_desktop: data.join("applications/notify-dnd-toggle.desktop"),
    }
}
