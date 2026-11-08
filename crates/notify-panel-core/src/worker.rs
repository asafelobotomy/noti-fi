//! The connection to notifd: connects, keeps the picture of the daemon's state up to date from pushed events,
//! applies commands from the UI, and reconnects when the daemon goes away. No Qt in here; results go to a `sink`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use notify_apps::{AppEntry, Roots};
use notify_client::{Client, Error};
use notify_protocol::{Config, Event, HistoryEntry, SimResult, Status};
use serde_json::{Value, json};

use crate::draft::{self, SaveOutcome};
use crate::view::{Conn, classify};

/// Something the user asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    Dnd(bool),
    Mute(String, bool),
    /// Turn a focus mode on by name, or off with "off".
    Focus(String),
    /// How many history entries each refresh fetches: the full list while the window is open, just the few the tray menu shows while it is not.
    HistoryLimit(u32),
    ClearHistory,
    /// Open, close or toggle the notification center.
    Center(String),
    MarkAllRead,
    /// Ask the daemon whether this draft config is acceptable. `token` comes back with the answer.
    Validate { token: u64, config: Value },
    /// Ask the daemon what it would do with a notification (the request carries the draft config).
    Simulate { token: u64, request: Value },
    /// Show real sample popups using this draft config.
    Preview { config: Value, seconds: u32 },
    EndPreview,
    /// Write rules.json (a file operation, but done here so everything the UI asks for goes through one place).
    Save { path: std::path::PathBuf, text: String, expected_base: String, force: bool },
    /// Work out which apps send (or could send) notifications. Reads the launchers on disk and asks the daemon who has sent.
    ScanApps { token: u64, roots: Roots },
    /// Change an app's Flatpak notification permission (allow, deny, or forget), then scan again.
    FlatpakPermission { token: u64, roots: Roots, app: String, allow: Option<bool> },
}

/// Everything the UI needs, fetched together so it is always consistent.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub status: Status,
    pub config: Config,
    pub history: Vec<HistoryEntry>,
    pub version: String,
    /// rules.json exactly as it is on disk.
    pub raw: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    /// The connection state changed (never repeated for the same state).
    State(Conn),
    /// A fresh picture of the daemon.
    Snapshot(Box<Snapshot>),
    /// The daemon refused a command; the text says why.
    Failed(String),
    /// Answer to `Cmd::Validate`.
    Validated { token: u64, ok: bool, error: String },
    /// Answer to `Cmd::Simulate`: the daemon's decision, or why it could not decide.
    Simulated { token: u64, result: Result<Box<SimResult>, String> },
    /// Answer to `Cmd::Save`; `text` is what was written (or tried).
    Saved { outcome: SaveOutcome, text: String },
    /// Answer to `Cmd::ScanApps` / `Cmd::FlatpakPermission`.
    Apps { token: u64, result: Result<Vec<AppEntry>, String> },
}

struct Reporter<F: FnMut(Msg)> {
    sink: F,
    last: Option<Conn>,
    history_limit: u32,
}

impl<F: FnMut(Msg)> Reporter<F> {
    fn state(&mut self, c: Conn) {
        if self.last.as_ref() != Some(&c) {
            self.last = Some(c.clone());
            (self.sink)(Msg::State(c));
        }
    }
    fn msg(&mut self, m: Msg) {
        (self.sink)(m);
    }
}

fn snapshot(c: &mut Client, config: &Config, raw: &str, version: &str, history_limit: u32) -> Result<Snapshot, Error> {
    Ok(Snapshot { status: c.status()?, config: config.clone(), history: c.history(history_limit)?, version: version.to_string(), raw: raw.to_string() })
}

fn scan<F: FnMut(Msg)>(c: &mut Client, token: u64, roots: &Roots, rep: &mut Reporter<F>) -> Result<(), Error> {
    let seen = match c.seen_apps() {
        Ok(s) => s,
        Err(Error::Daemon { message, .. }) => {
            rep.msg(Msg::Apps { token, result: Err(message) });
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    let found = notify_apps::merge(&seen, &notify_apps::discover(roots));
    rep.msg(Msg::Apps { token, result: Ok(found) });
    Ok(())
}

/// Carry out one command. A refusal from the daemon is reported, not fatal; a broken connection is an `Err`.
fn handle<F: FnMut(Msg)>(c: &mut Client, cmd: Cmd, rep: &mut Reporter<F>) -> Result<bool, Error> {
    let refused = |rep: &mut Reporter<F>, e: Error| -> Result<bool, Error> {
        match e {
            Error::Daemon { message, .. } => {
                rep.msg(Msg::Failed(message));
                Ok(false)
            }
            other => Err(other),
        }
    };
    match cmd {
        Cmd::Dnd(on) => match c.dnd(if on { "on" } else { "off" }) {
            Ok(_) => Ok(true),
            Err(e) => refused(rep, e),
        },
        Cmd::HistoryLimit(n) => {
            rep.history_limit = n.clamp(1, 1000);
            Ok(true)
        }
        Cmd::Focus(m) => match c.focus(&m) {
            Ok(_) => Ok(true),
            Err(e) => refused(rep, e),
        },
        Cmd::Mute(z, on) => match c.mute(&z, on) {
            Ok(_) => Ok(true),
            Err(e) => refused(rep, e),
        },
        Cmd::ClearHistory => match c.clear_history() {
            Ok(_) => Ok(true),
            Err(e) => refused(rep, e),
        },
        Cmd::Center(action) => match c.center(&action) {
            Ok(_) => Ok(true),
            Err(e) => refused(rep, e),
        },
        Cmd::MarkAllRead => match c.mark_read(None) {
            Ok(_) => Ok(true),
            Err(e) => refused(rep, e),
        },
        Cmd::Validate { token, config } => {
            match c.call_raw("validate", json!({ "config": config })) {
                Ok(v) => rep.msg(Msg::Validated {
                    token,
                    ok: v["ok"].as_bool().unwrap_or(false),
                    error: v["error"].as_str().unwrap_or("").to_string(),
                }),
                Err(Error::Daemon { message, .. }) => rep.msg(Msg::Validated { token, ok: false, error: message }),
                Err(e) => return Err(e),
            }
            Ok(false)
        }
        Cmd::Simulate { token, request } => {
            match c.call_raw("simulate", request) {
                Ok(v) => match serde_json::from_value::<SimResult>(v) {
                    Ok(r) => rep.msg(Msg::Simulated { token, result: Ok(Box::new(r)) }),
                    Err(e) => rep.msg(Msg::Simulated { token, result: Err(format!("unreadable answer: {e}")) }),
                },
                Err(Error::Daemon { message, .. }) => rep.msg(Msg::Simulated { token, result: Err(message) }),
                Err(e) => return Err(e),
            }
            Ok(false)
        }
        Cmd::Preview { config, seconds } => match c.call_raw("preview", json!({ "config": config, "seconds": seconds })) {
            Ok(_) => Ok(true),
            Err(e) => refused(rep, e),
        },
        Cmd::EndPreview => match c.end_preview() {
            Ok(_) => Ok(true),
            Err(e) => refused(rep, e),
        },
        Cmd::ScanApps { token, roots } => {
            scan(c, token, &roots, rep)?;
            Ok(false)
        }
        Cmd::FlatpakPermission { token, roots, app, allow } => {
            match notify_apps::set_flatpak_permission(&app, allow) {
                Ok(()) => scan(c, token, &roots, rep)?,
                Err(why) => rep.msg(Msg::Failed(why)),
            }
            Ok(false)
        }
        Cmd::Save { path, text, expected_base, force } => {
            let outcome = draft::save(&path, &text, &expected_base, force);
            rep.msg(Msg::Saved { outcome, text });
            Ok(false)
        }
    }
}

/// One connected session. Returns when the connection fails or `stop` is set.
fn session<F: FnMut(Msg)>(c: &mut Client, rx: &Receiver<Cmd>, stop: &AtomicBool, rep: &mut Reporter<F>) -> Result<(), Error> {
    let version = c.hello().daemon.clone();
    c.subscribe()?;
    // anything the UI asked for while we were disconnected is stale now: do not run it after the fact
    while rx.try_recv().is_ok() {}
    let got = c.config_get()?;
    let (mut config, mut raw) = (got.effective, got.raw);
    let first = snapshot(c, &config, &raw, &version, rep.history_limit)?;
    rep.msg(Msg::Snapshot(Box::new(first)));
    rep.state(Conn::Connected);

    let mut dirty = false;
    while !stop.load(Ordering::Relaxed) {
        // a burst of edits asks for many validations and simulations; only the newest of each is worth running
        let cmds: Vec<Cmd> = rx.try_iter().collect();
        let newest_validate = cmds.iter().rposition(|c| matches!(c, Cmd::Validate { .. }));
        let newest_simulate = cmds.iter().rposition(|c| matches!(c, Cmd::Simulate { .. }));
        let newest_scan = cmds.iter().rposition(|c| matches!(c, Cmd::ScanApps { .. }));
        for (i, cmd) in cmds.into_iter().enumerate() {
            match &cmd {
                Cmd::Validate { .. } if Some(i) != newest_validate => continue,
                Cmd::Simulate { .. } if Some(i) != newest_simulate => continue,
                Cmd::ScanApps { .. } if Some(i) != newest_scan => continue,
                _ => {}
            }
            if handle(c, cmd, rep)? {
                dirty = true;
            }
        }
        if let Some(e) = c.next_event(Duration::from_millis(100))? {
            dirty = true;
            let mut config_changed = matches!(e.event, Event::Config(_));
            // a burst of events becomes a single refresh
            while let Some(e2) = c.next_event(Duration::from_millis(25))? {
                config_changed |= matches!(e2.event, Event::Config(_));
            }
            if config_changed {
                let got = c.config_get()?;
                config = got.effective;
                raw = got.raw;
            }
        }
        if dirty {
            let s = snapshot(c, &config, &raw, &version, rep.history_limit)?;
            rep.msg(Msg::Snapshot(Box::new(s)));
            dirty = false;
        }
    }
    Ok(())
}

fn nap(stop: &AtomicBool, ms: u64) {
    let mut left = ms;
    while left > 0 && !stop.load(Ordering::Relaxed) {
        let step = left.min(100);
        std::thread::sleep(Duration::from_millis(step));
        left -= step;
    }
}

/// Run until `stop` is set: connect (retrying every second), serve commands from `rx`, report to `sink`.
pub fn run(socket: PathBuf, rx: Receiver<Cmd>, stop: std::sync::Arc<AtomicBool>, sink: impl FnMut(Msg)) {
    let mut rep = Reporter { sink, last: None, history_limit: 200 };
    rep.state(Conn::Connecting);
    while !stop.load(Ordering::Relaxed) {
        match Client::connect(&socket) {
            Err(e) => rep.state(classify(&e)),
            Ok(mut c) => {
                if let Err(e) = session(&mut c, &rx, &stop, &mut rep) {
                    if !stop.load(Ordering::Relaxed) {
                        rep.state(classify(&e));
                    }
                }
            }
        }
        nap(&stop, 1000);
    }
}
