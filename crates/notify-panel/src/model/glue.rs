//! How the panel meets the desktop session from the model's side: the system tray icon, and start at login.
//! A child module of `model` so it can reach the model's private state.

use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use notify_panel_core::shell;
use notify_panel_core::tray::{self, TrayAction, TrayState};
use notify_panel_core::worker::Cmd;
use serde_json::Value;

use super::qobject;

impl qobject::PanelModel {
    pub fn set_autostart_enabled(mut self: Pin<&mut Self>, on: bool) {
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => return self.refuse(&format!("cannot tell where this program is: {e}")),
        };
        if let Err(e) = shell::set_autostart(&shell::config_home(), &shell::data_home(), &exe, on) {
            self.as_mut().refuse(&format!("could not change the start-at-login setting: {e}"));
        }
        let now = shell::autostart_enabled(&shell::config_home());
        self.as_mut().set_autostart(now);
    }

    pub fn set_window_visible(self: Pin<&mut Self>, visible: bool) {
        self.send(Cmd::HistoryLimit(if visible { 200 } else { 8 }));
    }

    pub fn set_tray_show_text(mut self: Pin<&mut Self>, on: bool) {
        self.as_mut().rust_mut().tray_show_text = on;
        self.push_tray();
    }

    /// Put the icon in the system tray (once), without holding up the window: talking to the tray host can take a moment.
    /// Where there is no tray the panel is just a window. `NOTIFY_PANEL_NO_TRAY=1` (used by the tests, which must not touch your real tray) skips it.
    pub(super) fn start_tray(self: Pin<&mut Self>) {
        if self.as_ref().rust().tray.is_some() || std::env::var_os("NOTIFY_PANEL_NO_TRAY").is_some_and(|v| v == "1") {
            return;
        }
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let qt2 = qt.clone();
            let handle = tray::spawn(TrayState::default(), move |action| {
                let _ = qt2.queue(move |m| m.on_tray_action(action));
            });
            let _ = qt.queue(move |mut m| {
                let available = handle.is_some();
                m.as_mut().rust_mut().tray = handle;
                m.as_mut().set_tray_available(available);
                m.push_tray();
            });
        });
    }

    /// Tell the tray menu what the panel now knows.
    pub(super) fn push_tray(self: Pin<&mut Self>) {
        let state = {
            let me = self.as_ref();
            let r = me.rust();
            if r.tray.is_none() {
                return;
            }
            let json = |q: &QString| serde_json::from_str::<Vec<Value>>(&q.to_string()).unwrap_or_default();
            let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
            TrayState {
                online: *me.connected(),
                dnd: *me.dnd(),
                active: (*me.active()).max(0) as u32,
                focus: me.focus_mode().to_string(),
                focus_modes: json(me.focus_modes_json()).iter().map(|m| (text(m, "name"), text(m, "label"))).collect(),
                zones: json(me.zones_json()).iter().map(|z| (text(z, "name"), text(z, "label"), z.get("muted").and_then(Value::as_bool).unwrap_or(false))).collect(),
                recent: r
                    .history_all
                    .iter()
                    .take(6)
                    .map(|h| tray::recent_line(&tray::age_text(now, h.t), &h.app, &h.summary, h.shown, r.tray_show_text))
                    .collect(),
            }
        };
        if let Some(t) = self.as_ref().rust().tray.as_ref() {
            t.update(state);
        }
    }

    fn on_tray_action(mut self: Pin<&mut Self>, action: TrayAction) {
        match action {
            TrayAction::Toggle => self.as_mut().toggle_requested(),
            TrayAction::Show => self.as_mut().show_requested(),
            TrayAction::OpenPage(p) => self.as_mut().page_requested(&QString::from(&p)),
            TrayAction::SetDnd(on) => self.set_do_not_disturb(on),
            TrayAction::SetFocus(m) => self.send(Cmd::Focus(m)),
            TrayAction::MuteZone(z, on) => self.send(Cmd::Mute(z, on)),
            TrayAction::Quit => self.as_mut().quit_requested(),
        }
    }

    /// Print a JSON summary of the model to stdout (used by the headless tests).
    pub fn report(&self) {
        let r = self.rust();
        let zones: serde_json::Value = serde_json::from_str(&self.zones_json().to_string()).unwrap_or_default();
        println!(
            "{}",
            serde_json::json!({
                "report": {
                    "state": self.state().to_string(),
                    "state_message": self.state_message().to_string(),
                    "connected": *self.connected(),
                    "connections": *self.connections(),
                    "daemon_version": self.daemon_version().to_string(),
                    "dnd": *self.dnd(),
                    "holding": self.holding().to_string(),
                    "zones": zones.as_array().map(|a| a.len()).unwrap_or(0),
                    "history_total": r.history_all.len(),
                    "history_shown": *self.history_shown(),
                    "active": *self.active(),
                    "rules": *self.rules_count(),
                    "dirty": *self.dirty(),
                    "draft_valid": *self.draft_valid(),
                    "conflict": *self.conflict(),
                    "external_changed": *self.external_changed(),
                    "draft_error": self.draft_error().to_string(),
                    "save_message": self.save_message().to_string(),
                }
            })
        );
    }
}
