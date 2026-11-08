//! Turning daemon data into what the UI shows. Pure functions, unit-tested.

use notify_client::Error;
use notify_protocol::{Config, HistoryEntry, PROTOCOL_VERSION, SimResult, Status};
use serde_json::json;

/// Where the connection to notifd stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conn {
    Connecting,
    Connected,
    /// Nothing is listening on the control socket.
    NoDaemon,
    /// The daemon speaks a different protocol version.
    Incompatible { daemon: u32 },
    /// The connection was working and dropped (or the daemon misbehaved).
    Lost(String),
}

impl Conn {
    /// Stable name for QML and tests.
    pub fn code(&self) -> &'static str {
        match self {
            Conn::Connecting => "connecting",
            Conn::Connected => "connected",
            Conn::NoDaemon => "no-daemon",
            Conn::Incompatible { .. } => "incompatible",
            Conn::Lost(_) => "lost",
        }
    }

    /// Plain-words explanation with advice, for the banner (assuming notifd is installed).
    pub fn message(&self) -> String {
        self.message_with(true)
    }

    /// The same, knowing whether notifd is installed on this machine: if it is not, starting it cannot help and the advice is to install it.
    pub fn message_with(&self, daemon_installed: bool) -> String {
        match self {
            Conn::NoDaemon if !daemon_installed => "notifd is not installed on this machine. See what installing it changes with: notify takeover --dry-run, then run: notify takeover --now".into(),
            Conn::Connecting => "Connecting to notifd...".into(),
            Conn::Connected => String::new(),
            Conn::NoDaemon => "notifd is not running. Start it with: systemctl --user start notifd".into(),
            Conn::Incompatible { daemon } => format!(
                "notifd speaks protocol {daemon} but this panel needs protocol {PROTOCOL_VERSION}. Update the daemon with: notify update"
            ),
            Conn::Lost(why) => format!("Lost the connection to notifd ({why}). Reconnecting..."),
        }
    }
}

/// Has `notify takeover` been run (it leaves a record in the state folder)?
pub fn daemon_installed() -> bool {
    let state = std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()).map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/state")));
    state.is_some_and(|s| s.join("notifd/takeover.json").exists())
}

/// Map a client error to a connection state.
pub fn classify(e: &Error) -> Conn {
    match e {
        Error::Connect { .. } => Conn::NoDaemon,
        Error::VersionMismatch { daemon, .. } => Conn::Incompatible { daemon: *daemon },
        Error::Refused(m) => Conn::Lost(m.clone()),
        other => Conn::Lost(other.to_string()),
    }
}

/// The sentence under the Do Not Disturb switch.
pub fn hold_text(connected: bool, holding: &str) -> &'static str {
    if !connected {
        return "Not connected";
    }
    match holding {
        "dnd" => "On: normal notifications are held (critical ones still show)",
        "fullscreen" => "Held automatically while a window is full screen",
        "screen sharing" => "Held automatically while the screen is being shared",
        "inhibited" => "Held because an app asked (a presentation, a call, a recording)",
        _ => "Off: notifications show normally",
    }
}

/// One card per zone: name, label, accent colour, whether it is muted, and how many popups it shows.
pub fn zones_json(status: &Status, cfg: &Config) -> String {
    let zones: Vec<_> = status
        .zones
        .iter()
        .map(|z| {
            let zc = cfg.zones.get(z);
            json!({
                "name": z,
                "label": zc.and_then(|x| x.label.clone()).unwrap_or_else(|| z.clone()),
                "accent": zc.and_then(|x| x.accent.clone()).unwrap_or_else(|| "#60a5fa".into()),
                "muted": status.muted.contains(z),
                "count": status.per_zone.get(z).copied().unwrap_or(0),
                "unread": status.unread_per_zone.get(z).copied().unwrap_or(0),
            })
        })
        .collect();
    json!(zones).to_string()
}

/// History rows matching `query` (app, title, site or zone, case-insensitive) and, optionally, only the held ones.
pub fn filter_history<'a>(all: &'a [HistoryEntry], query: &str, missed_only: bool) -> Vec<&'a HistoryEntry> {
    let q = query.trim().to_lowercase();
    all.iter()
        .filter(|h| !missed_only || !h.shown)
        .filter(|h| q.is_empty() || format!("{} {} {} {}", h.app, h.summary, h.origin, h.zone).to_lowercase().contains(&q))
        .collect()
}

pub fn history_json(rows: &[&HistoryEntry]) -> String {
    serde_json::to_string(rows).unwrap_or_else(|_| "[]".into())
}

/// A simulation result as the simulator panel shows it.
/// The look and behaviour the daemon chose, in plain words, for the simulator.
fn look_lines(style: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(o) = style.get("opacity").and_then(|v| v.as_f64()) {
        out.push(format!("background {}% opaque", (o * 100.0).round()));
    }
    if let Some(w) = style.get("width").and_then(|v| v.as_f64()) {
        out.push(format!("{} px wide", w.round()));
    }
    let fam = style.get("fontFamily").and_then(|v| v.as_str()).unwrap_or("");
    let size = style.get("fontSize").and_then(|v| v.as_f64());
    match (fam.is_empty(), size) {
        (false, Some(sz)) => out.push(format!("font {fam}, {sz} px")),
        (true, Some(sz)) => out.push(format!("system font, {sz} px")),
        _ => {}
    }
    if style.get("clickThrough").and_then(|v| v.as_bool()) == Some(true) {
        out.push("clicks pass through".into());
    }
    if style.get("noFocus").and_then(|v| v.as_bool()) == Some(true) {
        out.push("never takes focus (no reply box)".into());
    }
    if let Some(g) = style.get("group").and_then(|v| v.as_str()).filter(|g| !g.is_empty()) {
        out.push(format!("grouped as '{g}'"));
    }
    out
}

pub fn sim_view(r: &SimResult) -> serde_json::Value {
    let by = |d: &Option<String>| d.as_ref().map(|n| format!("rule '{n}'")).unwrap_or_else(|| "default".into());
    json!({
        "ok": true,
        "zone": r.zone,
        "zoneBy": by(&r.decided_by.zone),
        "timeout": if r.timeout == 0 { "stays until dismissed".to_string() } else { format!("{} ms", r.timeout) },
        "timeoutBy": by(&r.decided_by.timeout),
        "outcome": if r.would_show { "shown".to_string() } else { format!("not shown ({})", r.hold) },
        "wouldShow": r.would_show,
        "silent": r.silent,
        "matched": r.matched,
        "look": look_lines(&r.style),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify_protocol::{Auto, Urgency, Zone, Anchor};
    use std::collections::BTreeMap;

    fn status() -> Status {
        Status {
            dnd: false,
            muted: vec!["business".into()],
            zones: vec!["business".into(), "ghost".into()],
            active: 2,
            queued: 0,
            per_zone: BTreeMap::from([("ghost".to_string(), 2)]),
            cap: 5,
            history: 0,
            cfg_error: String::new(),
            auto: Auto::default(),
            holding: String::new(),
            previewing: false,
            focus: String::new(),
            stacks: Vec::new(),
            unread: 0,
            unread_per_zone: Default::default(),
            center: false,
            kept: 0,
            inhibitors: vec![],
            sharing: false,
            latest: None,
            screens: Default::default(),
        }
    }

    fn entry(app: &str, summary: &str, zone: &str, shown: bool) -> HistoryEntry {
        HistoryEntry { t: 0, id: 1, app: app.into(), summary: summary.into(), body: String::new(), zone: zone.into(), origin: String::new(), urgency: Urgency::Normal, shown, why: if shown { String::new() } else { "dnd".into() }, updates: None, desktop_entry: String::new(), category: String::new(), channel: String::new(), read: false, icon: String::new(), live: false, actions: vec![] }
    }

    #[test]
    fn zones_carry_label_accent_mute_and_count() {
        let mut cfg = Config::default();
        cfg.zones.insert("business".into(), Zone { accent: Some("#c084fc".into()), label: Some("Business".into()), ..Zone::at(Anchor::TopLeft) });
        let v: serde_json::Value = serde_json::from_str(&zones_json(&status(), &cfg)).unwrap();
        assert_eq!(v[0]["label"], "Business");
        assert_eq!(v[0]["accent"], "#c084fc");
        assert_eq!(v[0]["muted"], true);
        assert_eq!(v[0]["count"], 0);
        assert_eq!(v[1]["count"], 2);
        assert_eq!(v[1]["label"], "ghost", "a zone missing from the config falls back to its name");
    }

    #[test]
    fn history_filter_matches_text_and_held_only() {
        let all = vec![entry("Steam", "Friend online", "personal", true), entry("Brave", "Tip", "business", false), entry("kernel", "Disk full", "system", true)];
        assert_eq!(filter_history(&all, "", false).len(), 3);
        assert_eq!(filter_history(&all, "STEAM", false).len(), 1);
        assert_eq!(filter_history(&all, "business", false).len(), 1, "zone names are searched too");
        assert_eq!(filter_history(&all, "  disk ", false).len(), 1, "whitespace is ignored");
        assert_eq!(filter_history(&all, "", true).len(), 1, "missed only");
        assert_eq!(filter_history(&all, "steam", true).len(), 0);
    }

    #[test]
    fn connection_states_have_codes_and_advice() {
        assert_eq!(Conn::NoDaemon.code(), "no-daemon");
        assert!(Conn::NoDaemon.message().contains("systemctl --user start notifd"));
        assert!(Conn::NoDaemon.message_with(false).contains("notify takeover --now"), "not installed: the advice is to install it, not to start it");
        assert!(!Conn::NoDaemon.message_with(false).contains("systemctl"));
        assert_eq!(Conn::Lost("x".into()).message_with(false), Conn::Lost("x".into()).message(), "only the no-daemon message depends on installation");
        let i = Conn::Incompatible { daemon: 9 };
        assert_eq!(i.code(), "incompatible");
        assert!(i.message().contains("protocol 9") && i.message().contains("notify update"));
        assert!(Conn::Lost("closed".into()).message().contains("Reconnecting"));
        assert!(Conn::Connected.message().is_empty());
    }

    #[test]
    fn classifies_client_errors() {
        let e = Error::Connect { path: "/x".into(), source: std::io::Error::from(std::io::ErrorKind::NotFound) };
        assert_eq!(classify(&e), Conn::NoDaemon);
        assert_eq!(classify(&Error::VersionMismatch { daemon: 2, client: 1 }), Conn::Incompatible { daemon: 2 });
        assert_eq!(classify(&Error::Closed).code(), "lost");
    }

    #[test]
    fn sim_results_are_presented_in_plain_words() {
        use notify_protocol::{DecidedBy, DecidedIdx};
        let r = SimResult {
            ok: true, zone: "business".into(), timeout: 0, silent: false, would_show: true, hold: String::new(),
            matched: vec!["Business sites".into()],
            decided_by: DecidedBy { zone: Some("Business sites".into()), timeout: None, silent: None },
            matched_idx: vec![0], decided_idx: DecidedIdx { zone: Some(0), timeout: None, silent: None },
            style: json!({"opacity": 0.7, "width": 500, "fontFamily": "Serif", "fontSize": 16, "clickThrough": true, "noFocus": false, "group": "Mail"}),
        };
        let v = sim_view(&r);
        assert_eq!(v["zoneBy"], "rule 'Business sites'");
        assert_eq!(v["timeoutBy"], "default");
        assert_eq!(v["timeout"], "stays until dismissed");
        assert_eq!(v["outcome"], "shown");
        let look: Vec<&str> = v["look"].as_array().unwrap().iter().filter_map(|l| l.as_str()).collect();
        assert_eq!(look, vec!["background 70% opaque", "500 px wide", "font Serif, 16 px", "clicks pass through", "grouped as 'Mail'"]);
        let held = SimResult { would_show: false, hold: "dnd".into(), timeout: 6000, ..r };
        assert_eq!(sim_view(&held)["outcome"], "not shown (dnd)");
        assert_eq!(sim_view(&held)["timeout"], "6000 ms");
    }

    #[test]
    fn hold_text_explains_each_reason() {
        assert_eq!(hold_text(false, ""), "Not connected");
        assert!(hold_text(true, "dnd").starts_with("On:"));
        assert!(hold_text(true, "fullscreen").contains("full screen"));
        assert!(hold_text(true, "screen sharing").contains("shared"));
        assert!(hold_text(true, "").starts_with("Off"));
    }
}
