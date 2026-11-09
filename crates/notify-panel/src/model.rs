//! The bridge between Rust and QML: one QObject, `PanelModel`, that the QML pages bind to.
//!
//! All logic lives in the Qt-free library (`worker`, `view`). This file only forwards: worker messages become
//! property updates on the UI thread, and invokable calls become worker commands. QML sees simple values
//! (bool, int, string; lists as JSON strings), which keeps this boundary small.
//!
//! cxx-qt gotcha: `#[auto_cxx_name]` (snake_case -> camelCase for QML) is needed on EVERY `extern "RustQt"` block.

use std::pin::Pin;
use std::sync::Mutex;
use std::sync::mpsc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use notify_panel_core::draft::{self, Draft};
use notify_panel_core::shell;
use notify_panel_core::view::{self, Conn};
use notify_panel_core::worker::{self, Cmd, Msg};
use notify_protocol::DecidedIdx;

/// The socket this panel listens on for "show yourself"; set by `main` after it has claimed the instance.
pub static INSTANCE_LISTENER: Mutex<Option<std::os::unix::net::UnixListener>> = Mutex::new(None);

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        // connection
        #[qproperty(bool, connected)]
        #[qproperty(QString, state)]
        #[qproperty(QString, state_message)]
        #[qproperty(QString, daemon_version)]
        #[qproperty(i32, connections)]
        // daemon state
        #[qproperty(bool, dnd)]
        // focus mode: the one that is on, the modes the running daemon has, and the modes being edited in the draft
        #[qproperty(QString, focus_mode)]
        #[qproperty(QString, focus_modes_json)]
        #[qproperty(QString, focus_draft_json)]
        #[qproperty(QString, holding)]
        #[qproperty(QString, hold_text)]
        #[qproperty(i32, active)]
        #[qproperty(i32, queued)]
        #[qproperty(QString, zones_json)]
        // history (already filtered)
        #[qproperty(QString, history_json)]
        #[qproperty(i32, history_total)]
        #[qproperty(i32, history_shown)]
        // rules editor
        #[qproperty(QString, rules_json)]
        #[qproperty(i32, rules_count)]
        #[qproperty(QString, draft_zones_json)]
        #[qproperty(bool, dirty)]
        #[qproperty(bool, draft_valid)]
        #[qproperty(bool, checking)]
        #[qproperty(QString, draft_error)]
        #[qproperty(bool, external_changed)]
        #[qproperty(bool, conflict)]
        #[qproperty(QString, save_message)]
        #[qproperty(QString, sim_json)]
        #[qproperty(QString, settings_json)]
        // start at login (the autostart entry exists)
        #[qproperty(bool, autostart)]
        #[qproperty(QString, update_state)]   // idle | checking | current | available | installing | installed | error
        #[qproperty(QString, update_latest)]
        #[qproperty(QString, update_message)]
        #[qproperty(QString, update_page)]
        // the system tray icon exists (the desktop has a tray)
        #[qproperty(bool, tray_available)]
        // test guards: `NOTIFY_PANEL_TEST=1` (test mode) and `NOTIFD_RULES` set; the --selftest hooks refuse to act without them
        #[qproperty(bool, test_mode)]
        #[qproperty(bool, rules_overridden)]
        // apps
        #[qproperty(QString, apps_json)]
        #[qproperty(bool, apps_scanning)]
        #[qproperty(QString, system_json)]   // System page: {checks, summary, busy, output, ok, applied, action, error}
        type PanelModel = super::PanelModelRust;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        /// Connect to the daemon in the background and keep the properties live.
        #[qinvokable]
        fn start(self: Pin<&mut PanelModel>);
        #[qinvokable]
        fn set_do_not_disturb(self: Pin<&mut PanelModel>, on: bool);
        #[qinvokable]
        fn set_zone_muted(self: Pin<&mut PanelModel>, zone: &QString, on: bool);
        #[qinvokable]
        fn clear_history(self: Pin<&mut PanelModel>);
        #[qinvokable]
        fn set_history_filter(self: Pin<&mut PanelModel>, query: &QString, missed_only: bool);
        /// Print a JSON summary of the model to stdout (used by the headless tests).
        #[qinvokable]
        fn report(self: &PanelModel);

        // ---- rules editor: every edit changes the draft, then re-validates and re-simulates it
        #[qinvokable]
        fn rule_add(self: Pin<&mut PanelModel>) -> i32;
        #[qinvokable]
        fn rule_delete(self: Pin<&mut PanelModel>, index: i32);
        #[qinvokable]
        fn rule_move(self: Pin<&mut PanelModel>, index: i32, delta: i32) -> i32;
        #[qinvokable]
        fn rule_duplicate(self: Pin<&mut PanelModel>, index: i32) -> i32;
        #[qinvokable]
        fn rule_set_name(self: Pin<&mut PanelModel>, index: i32, name: &QString);
        /// Values seen in history and in the app list, per match field, most used first: what the rule editor offers as suggestions.
        #[qinvokable]
        fn rule_suggestions(self: Pin<&mut PanelModel>) -> QString;
        /// The most recent notifications (newest first): what the rule editor tests a rule against.
        #[qinvokable]
        fn recent_notifications(self: Pin<&mut PanelModel>) -> QString;
        /// An empty pattern removes the test for that field.
        #[qinvokable]
        fn rule_set_match(self: Pin<&mut PanelModel>, index: i32, field: &QString, pattern: &QString);
        /// An empty zone leaves the choice to later rules and the default.
        #[qinvokable]
        fn rule_set_zone(self: Pin<&mut PanelModel>, index: i32, zone: &QString);
        /// Milliseconds as text; empty = not set by this rule; 0 = stays until dismissed.
        #[qinvokable]
        fn rule_set_timeout(self: Pin<&mut PanelModel>, index: i32, text: &QString);
        #[qinvokable]
        fn rule_set_silent(self: Pin<&mut PanelModel>, index: i32, silent: bool);
        /// Look and behaviour of matching notifications: key is opacity, clickThrough, noFocus, width, fontFamily, fontSize or group;
        /// empty text = not set; clickThrough/noFocus take "true" or "false" (an explicit false overrides a later rule).
        #[qinvokable]
        fn rule_set_style(self: Pin<&mut PanelModel>, index: i32, key: &QString, text: &QString);
        /// Turn a focus mode on by name, or off with "off" (the daemon applies it at once; it is not part of the draft).
        #[qinvokable]
        fn set_focus(self: Pin<&mut PanelModel>, mode: &QString);
        // ---- focus modes in the draft: add, delete, and set label / minPriority / allow (text, one entry per line)
        #[qinvokable]
        fn focus_add(self: Pin<&mut PanelModel>) -> QString;
        #[qinvokable]
        fn focus_delete(self: Pin<&mut PanelModel>, name: &QString);
        #[qinvokable]
        fn focus_set(self: Pin<&mut PanelModel>, name: &QString, field: &QString, text: &QString);
        /// Describe the notification to simulate (appName, desktopEntry, origin, summary, body, urgency, timeout, ignoreState).
        #[qinvokable]
        fn sim_set(self: Pin<&mut PanelModel>, field: &QString, value: &QString);
        // ---- zones and appearance (the same draft; Save and Preview are shared with the rules page)
        /// Set a scalar setting by dotted path, e.g. "theme.card", "maxVisible", "sound.enabled" (booleans as "true"/"false").
        #[qinvokable]
        fn setting_set(self: Pin<&mut PanelModel>, path: &QString, text: &QString);
        /// Adds a zone and returns its id.
        #[qinvokable]
        fn zone_add(self: Pin<&mut PanelModel>) -> QString;
        #[qinvokable]
        fn zone_delete(self: Pin<&mut PanelModel>, id: &QString);
        /// field: "label", "anchor" or "accent".
        #[qinvokable]
        fn zone_set(self: Pin<&mut PanelModel>, id: &QString, field: &QString, text: &QString);
        /// The same keys on a zone (all but group).
        #[qinvokable]
        fn zone_style(self: Pin<&mut PanelModel>, id: &QString, key: &QString, text: &QString);
        /// Put a zone at an exact place (x, y are 0..1 of the screen from the left and top); the zone becomes "free".
        #[qinvokable]
        fn zone_place(self: Pin<&mut PanelModel>, id: &QString, x: f64, y: f64);
        // ---- apps: who sends notifications, and how notify treats each one
        /// Look again (reads launchers on disk and asks the daemon which apps have sent).
        #[qinvokable]
        fn apps_rescan(self: Pin<&mut PanelModel>);
        /// The installed event sounds (JSON array of names), and playing one so it can be heard before it is picked.
        #[qinvokable]
        fn sound_names(self: Pin<&mut PanelModel>) -> QString;
        #[qinvokable]
        fn play_sound(self: Pin<&mut PanelModel>, name: &QString);
        #[qinvokable]
        fn system_doctor(self: Pin<&mut PanelModel>);
        #[qinvokable]
        fn system_run(self: Pin<&mut PanelModel>, action: &QString, apply: bool);
        /// Change how notify handles one app, by editing the draft. `what`: "muted" ("true"/"false"), "zone" ("" = no choice) or "timeout" (ms, "" = no choice).
        #[qinvokable]
        fn app_set(self: Pin<&mut PanelModel>, match_field: &QString, match_value: &QString, label: &QString, what: &QString, text: &QString);
        /// Like `app_set`, for one kind of notification: `matches_json` is a list of [field, text] pairs, e.g. [["appName","Mailer"],["category","email.arrived"]].
        #[qinvokable]
        fn app_set_kind(self: Pin<&mut PanelModel>, matches_json: &QString, label: &QString, what: &QString, text: &QString);
        /// Change a Flatpak app's own notification permission right away: mode "allow", "deny" or "reset".
        #[qinvokable]
        fn app_flatpak(self: Pin<&mut PanelModel>, app_id: &QString, mode: &QString);
        #[qinvokable]
        fn rules_preview(self: Pin<&mut PanelModel>);
        #[qinvokable]
        fn rules_end_preview(self: Pin<&mut PanelModel>);
        /// Throw the edits away and start again from what is on disk.
        #[qinvokable]
        fn rules_reload(self: Pin<&mut PanelModel>);
        /// `force` overwrites a file that was edited by hand since the draft was started.
        #[qinvokable]
        fn rules_save(self: Pin<&mut PanelModel>, force: bool);

        /// Start the panel (hidden, in the system tray) when you log in, or stop doing so.
        #[qinvokable]
        fn set_autostart_enabled(self: Pin<&mut PanelModel>, on: bool);

        /// Look for a newer release (contacts github.com; only when asked), install it, and restart this program afterwards.
        #[qinvokable]
        fn check_for_updates(self: Pin<&mut PanelModel>);
        #[qinvokable]
        fn install_update(self: Pin<&mut PanelModel>);
        #[qinvokable]
        fn restart_app(self: Pin<&mut PanelModel>);
        /// The window is shown or hidden: a hidden panel (only the tray icon) fetches just a few history entries on each refresh.
        #[qinvokable]
        fn set_window_visible(self: Pin<&mut PanelModel>, visible: bool);
        /// Whether message titles may appear in the tray menu's recent list (follows the History page's "show message text").
        #[qinvokable]
        fn set_tray_show_text(self: Pin<&mut PanelModel>, on: bool);

        /// Another launch of the panel, or the tray's "Open", asked this one to come forward.
        #[qsignal]
        fn show_requested(self: Pin<&mut PanelModel>);
        /// A click on the tray icon: show the window, or hide it when it is already in front.
        #[qsignal]
        fn toggle_requested(self: Pin<&mut PanelModel>);
        /// The tray menu asked for a page ("history", "general").
        #[qsignal]
        fn page_requested(self: Pin<&mut PanelModel>, page: &QString);
        /// The tray menu's Quit.
        #[qsignal]
        fn quit_requested(self: Pin<&mut PanelModel>);

        /// The daemon refused a command; `message` says why.
        #[qsignal]
        fn command_failed(self: Pin<&mut PanelModel>, message: &QString);
    }

    impl cxx_qt::Threading for PanelModel {}
}

pub use state::PanelModelRust;

mod edit;
mod glue;
mod handlers;
mod sound;
mod state;
mod system;
mod update;
mod views;

impl qobject::PanelModel {
    fn on_state(mut self: Pin<&mut Self>, c: Conn) {
        let connected = c == Conn::Connected;
        if connected {
            let count = *self.as_ref().connections() + 1;
            self.as_mut().set_connections(count);
        } else {
            self.as_mut().set_hold_text(QString::from(view::hold_text(false, "")));
        }
        self.as_mut().set_connected(connected);
        self.as_mut().set_state(QString::from(c.code()));
        self.as_mut().set_state_message(QString::from(&c.message_with(view::daemon_installed())));
        self.as_mut().push_tray();
        // a check sent before the connection dropped was thrown away with it: ask again, or Save stays disabled
        if connected && *self.as_ref().checking() {
            self.after_edit();
        }
    }

    pub fn start(mut self: Pin<&mut Self>) {
        if self.as_ref().rust().tx.is_some() {
            return;
        }
        self.as_mut().start_tray();
        // a later launch of the panel asks this one to show its window
        if let Some(l) = INSTANCE_LISTENER.lock().ok().and_then(|mut g| g.take()) {
            let qt = self.qt_thread();
            shell::serve(l, move || {
                let _ = qt.queue(|m| m.show_requested());
            });
        }
        let (tx, rx) = mpsc::channel::<Cmd>();
        self.as_mut().rust_mut().tx = Some(tx);
        let stop = self.as_ref().rust().stop.clone();
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            worker::run(notify_client::default_socket_path(), rx, stop, move |msg| {
                let _ = qt.queue(move |m| match msg {
                    Msg::State(c) => m.on_state(c),
                    Msg::Snapshot(s) => m.on_snapshot(*s),
                    Msg::Failed(text) => m.command_failed(&QString::from(&text)),
                    Msg::Validated { token, ok, error } => m.on_validated(token, ok, error),
                    Msg::Simulated { token, result } => m.on_simulated(token, result),
                    Msg::Saved { outcome, text } => m.on_saved(outcome, text),
                    Msg::Apps { token, result } => m.on_apps(token, result),
                });
            });
        });
    }

    fn send_cmd(&self, cmd: Cmd) {
        if let Some(tx) = &self.rust().tx {
            let _ = tx.send(cmd);
        }
    }

    fn send(&self, cmd: Cmd) {
        self.send_cmd(cmd);
    }

    pub fn set_do_not_disturb(self: Pin<&mut Self>, on: bool) {
        self.send(Cmd::Dnd(on));
    }

    pub fn set_zone_muted(self: Pin<&mut Self>, zone: &QString, on: bool) {
        self.send(Cmd::Mute(zone.to_string(), on));
    }

    pub fn clear_history(self: Pin<&mut Self>) {
        self.send(Cmd::ClearHistory);
    }

    pub fn set_history_filter(mut self: Pin<&mut Self>, query: &QString, missed_only: bool) {
        self.as_mut().rust_mut().filter_query = query.to_string();
        self.as_mut().rust_mut().filter_missed = missed_only;
        self.refresh_history_view();
    }

    // ------------------------------------------------------------ rules editor
}
