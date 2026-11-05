//! The bridge between Rust and QML: one QObject, `PanelModel`, that the QML pages bind to.
//!
//! All logic lives in the Qt-free library (`worker`, `view`). This file only forwards: worker messages become
//! property updates on the UI thread, and invokable calls become worker commands. QML sees simple values
//! (bool, int, string; lists as JSON strings), which keeps this boundary small.
//!
//! cxx-qt gotcha: `#[auto_cxx_name]` (snake_case -> camelCase for QML) is needed on EVERY `extern "RustQt"` block.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use notify_apps::AppEntry;
use notify_panel_core::draft::{self, Draft, SimInput};
use notify_panel_core::shell;
use notify_panel_core::view::{self, Conn};
use notify_panel_core::worker::{self, Cmd, Msg, Snapshot};
use notify_protocol::{DecidedIdx, HistoryEntry};
use serde_json::{Value, json};

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
        #[qproperty(QString, rules_path_text)]
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

pub struct PanelModelRust {
    connected: bool,
    state: QString,
    state_message: QString,
    daemon_version: QString,
    connections: i32,
    dnd: bool,
    focus_mode: QString,
    focus_modes_json: QString,
    focus_draft_json: QString,
    holding: QString,
    hold_text: QString,
    active: i32,
    queued: i32,
    zones_json: QString,
    history_json: QString,
    history_total: i32,
    history_shown: i32,
    rules_json: QString,
    rules_count: i32,
    draft_zones_json: QString,
    dirty: bool,
    draft_valid: bool,
    checking: bool,
    draft_error: QString,
    external_changed: bool,
    conflict: bool,
    save_message: QString,
    rules_path_text: QString,
    sim_json: QString,
    settings_json: QString,
    autostart: bool,
    update_state: QString,
    update_latest: QString,
    update_message: QString,
    update_page: QString,
    tray_available: bool,
    test_mode: bool,
    rules_overridden: bool,
    apps_json: QString,
    apps_scanning: bool,
    // not exposed to QML
    tray: Option<notify_panel_core::tray::TrayHandle>,
    tray_show_text: bool,
    tx: Option<Sender<Cmd>>,
    stop: Arc<AtomicBool>,
    history_all: Vec<HistoryEntry>,
    filter_query: String,
    filter_missed: bool,
    // rules editor state
    draft: Draft,
    draft_loaded: bool,
    latest_raw: String,
    conflict_disk: String,
    error_idx: Option<usize>,
    validate_token: u64,
    sim_token: u64,
    sim: SimInput,
    sim_matched: Vec<usize>,
    sim_decided: DecidedIdx,
    apps: Vec<AppEntry>,
    apps_token: u64,
}

impl Default for PanelModelRust {
    fn default() -> Self {
        let c = Conn::Connecting;
        Self {
            connected: false,
            state: QString::from(c.code()),
            state_message: QString::from(&c.message()),
            daemon_version: QString::from(""),
            connections: 0,
            dnd: false,
            focus_mode: QString::from(""),
            focus_modes_json: QString::from("[]"),
            focus_draft_json: QString::from("[]"),
            holding: QString::from(""),
            hold_text: QString::from(view::hold_text(false, "")),
            active: 0,
            queued: 0,
            zones_json: QString::from("[]"),
            history_json: QString::from("[]"),
            history_total: 0,
            history_shown: 0,
            rules_json: QString::from("[]"),
            rules_count: 0,
            draft_zones_json: QString::from("[]"),
            dirty: false,
            draft_valid: false,
            checking: false,
            draft_error: QString::from(""),
            external_changed: false,
            conflict: false,
            save_message: QString::from(""),
            rules_path_text: QString::from(&draft::rules_path().display().to_string()),
            sim_json: QString::from("{}"),
            settings_json: QString::from("{}"),
            autostart: shell::autostart_enabled(&shell::config_home()),
            update_state: QString::from("idle"),
            update_latest: QString::from(""),
            update_message: QString::from(""),
            update_page: QString::from(""),
            tray_available: false,
            test_mode: std::env::var_os("NOTIFY_PANEL_TEST").is_some_and(|v| v == "1"),
            rules_overridden: std::env::var_os("NOTIFD_RULES").is_some_and(|v| !v.is_empty()),
            apps_json: QString::from("[]"),
            apps_scanning: false,
            tray: None,
            tray_show_text: false,
            tx: None,
            stop: Arc::new(AtomicBool::new(false)),
            history_all: Vec::new(),
            filter_query: String::new(),
            filter_missed: false,
            draft: Draft::empty(),
            draft_loaded: false,
            latest_raw: String::new(),
            conflict_disk: String::new(),
            error_idx: None,
            validate_token: 0,
            sim_token: 0,
            sim: SimInput::default(),
            sim_matched: Vec::new(),
            sim_decided: DecidedIdx::default(),
            apps: Vec::new(),
            apps_token: 0,
        }
    }
}

mod edit;
mod glue;
mod handlers;
mod update;

impl qobject::PanelModel {
    fn refresh_history_view(mut self: Pin<&mut Self>) {
        let (json, shown, total) = {
            let me = self.as_ref();
            let r = me.rust();
            let rows = view::filter_history(&r.history_all, &r.filter_query, r.filter_missed);
            (view::history_json(&rows), rows.len() as i32, r.history_all.len() as i32)
        };
        self.as_mut().set_history_json(QString::from(&json));
        self.as_mut().set_history_shown(shown);
        self.as_mut().set_history_total(total);
    }

    fn on_snapshot(mut self: Pin<&mut Self>, s: Snapshot) {
        self.as_mut().set_daemon_version(QString::from(&s.version));
        self.as_mut().set_dnd(s.status.dnd);
        self.as_mut().set_focus_mode(QString::from(&s.status.focus));
        let modes: Vec<Value> = s.config.focus.modes.iter().map(|(n, m)| json!({"name": n, "label": m.label.clone().unwrap_or_else(|| n.clone()), "minPriority": m.min_priority.clone().unwrap_or_else(|| "urgent".into())})).collect();
        self.as_mut().set_focus_modes_json(QString::from(&json!(modes).to_string()));
        self.as_mut().set_holding(QString::from(&s.status.holding));
        self.as_mut().set_hold_text(QString::from(view::hold_text(true, &s.status.holding)));
        self.as_mut().set_active(s.status.active as i32);
        self.as_mut().set_queued(s.status.queued as i32);
        self.as_mut().set_zones_json(QString::from(&view::zones_json(&s.status, &s.config)));
        self.as_mut().rust_mut().history_all = s.history;
        self.as_mut().refresh_history_view();
        self.as_mut().push_tray();
        self.sync_draft(&s.raw);
    }

    /// Keep the editor's draft in step with the file: follow hand edits while there are no unsaved changes,
    /// and flag them when there are.
    fn sync_draft(mut self: Pin<&mut Self>, raw: &str) {
        self.as_mut().rust_mut().latest_raw = raw.to_string();
        let (loaded, dirty, base_matches) = {
            let me = self.as_ref();
            let r = me.rust();
            (r.draft_loaded, r.draft.is_dirty(), r.draft.base_raw() == raw)
        };
        if !loaded || (!dirty && !base_matches) {
            if let Ok(d) = Draft::from_raw(raw) {
                let mut r = self.as_mut().rust_mut();
                r.draft = d;
                r.draft_loaded = true;
                self.as_mut().set_external_changed(false);
                self.after_edit();
                return;
            }
        }
        self.as_mut().set_external_changed(dirty && !base_matches);
    }

    /// Something about the draft changed: refresh what the UI shows, and ask the daemon to validate and simulate it.
    fn after_edit(mut self: Pin<&mut Self>) {
        let (vt, st, config, request) = {
            let mut r = self.as_mut().rust_mut();
            r.validate_token += 1;
            r.sim_token += 1;
            (r.validate_token, r.sim_token, r.draft.value().clone(), r.sim.to_request(r.draft.value()))
        };
        self.as_mut().set_checking(true);
        self.as_ref().send_cmd(Cmd::Validate { token: vt, config });
        self.as_ref().send_cmd(Cmd::Simulate { token: st, request });
        self.refresh_rules_view();
    }

    fn refresh_rules_view(mut self: Pin<&mut Self>) {
        let (json, count, zones, dirty) = {
            let me = self.as_ref();
            let r = me.rust();
            let list: Vec<Value> = r
                .draft
                .rules()
                .into_iter()
                .map(|rv| {
                    let i = rv.index;
                    let mut v = serde_json::to_value(&rv).unwrap_or_default();
                    v["matched"] = json!(r.sim_matched.contains(&i));
                    v["decidesZone"] = json!(r.sim_decided.zone == Some(i));
                    v["decidesTimeout"] = json!(r.sim_decided.timeout == Some(i));
                    v["decidesSilent"] = json!(r.sim_decided.silent == Some(i));
                    v["hasError"] = json!(r.error_idx == Some(i));
                    v
                })
                .collect();
            (json!(list).to_string(), r.draft.rule_count() as i32, json!(r.draft.zone_names()).to_string(), r.draft.is_dirty())
        };
        self.as_mut().set_rules_json(QString::from(&json));
        self.as_mut().set_rules_count(count);
        self.as_mut().set_draft_zones_json(QString::from(&zones));
        let focus = self.as_ref().rust().draft.focus_view().to_string();
        self.as_mut().set_focus_draft_json(QString::from(&focus));
        let settings = self.as_ref().rust().draft.settings_view().to_string();
        self.as_mut().set_settings_json(QString::from(&settings));
        self.as_mut().set_dirty(dirty);
        self.refresh_apps_view();
    }

    /// The apps list as the UI shows it: what was found, plus how this draft treats each app.
    fn refresh_apps_view(mut self: Pin<&mut Self>) {
        let json = {
            let me = self.as_ref();
            let r = me.rust();
            let list: Vec<Value> = r
                .apps
                .iter()
                .map(|a| {
                    let mut v = serde_json::to_value(a).unwrap_or_default();
                    v["routing"] = serde_json::to_value(r.draft.app_routing(&a.match_field, &a.match_value)).unwrap_or_default();
                    // each kind of notification, with how the draft treats it and the match pairs that select it
                    let kinds: Vec<Value> = a
                        .types
                        .iter()
                        .map(|t| {
                            let mut pairs: Vec<(&str, &str)> = vec![(a.match_field.as_str(), a.match_value.as_str())];
                            if !t.category.is_empty() {
                                pairs.push(("category", t.category.as_str()));
                            }
                            pairs.push(("channel", t.channel.as_str()));
                            let mut kv = serde_json::to_value(t).unwrap_or_default();
                            kv["routing"] = serde_json::to_value(r.draft.routing_for(&pairs)).unwrap_or_default();
                            kv["matches"] = json!(pairs);
                            kv
                        })
                        .collect();
                    v["kinds"] = json!(kinds);
                    v
                })
                .collect();
            json!(list).to_string()
        };
        self.as_mut().set_apps_json(QString::from(&json));
    }

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
