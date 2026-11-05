//! Typed messages for the notifd control socket, and the `rules.json` config model.
//!
//! Wire format: one JSON object per line.
//!
//! ```text
//! server -> client (first):  {"event":"hello","protocol":1,"daemon":"0.6.0","pid":123,"seq":0}
//! client -> server:          {"id":1,"method":"status","params":{}}
//! server -> client:          {"id":1,"result":{...}}   or   {"id":1,"error":{"code":"...","message":"..."}}
//! after "subscribe":         {"event":"state"|"notification"|"updated"|"closed"|"config"|"preview","seq":N,"t":ms,...}
//! ```
//!
//! This crate does no I/O. The daemon is the authority on behaviour; these types describe its messages.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

/// Protocol version this crate speaks. A daemon announcing a different number is incompatible.
pub const PROTOCOL_VERSION: u32 = 1;

// ---------------------------------------------------------------- envelope

/// A request sent to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// An error answer from the daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

/// An answer to a request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Response {
    pub id: Option<u64>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<ErrorBody>,
}

/// First message on every connection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hello {
    pub protocol: u32,
    pub daemon: String,
    pub pid: u32,
    pub seq: u64,
}

/// Anything the daemon can send: an answer or a pushed event.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Response(Response),
    Event(Box<EventEnvelope>),
}

impl Incoming {
    /// Parse one line from the daemon.
    pub fn parse(line: &str) -> Result<Incoming, serde_json::Error> {
        let v: Value = serde_json::from_str(line)?;
        if v.get("event").is_some() {
            Ok(Incoming::Event(Box::new(serde_json::from_value(v)?)))
        } else {
            Ok(Incoming::Response(serde_json::from_value(v)?))
        }
    }
}

// ---------------------------------------------------------------- events

/// A pushed event with its ordering information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventEnvelope {
    /// Ordering number (absent on the `error` event the daemon sends before refusing a connection).
    #[serde(default)]
    pub seq: u64,
    /// Milliseconds since the Unix epoch (absent on `hello`).
    #[serde(default)]
    pub t: Option<i64>,
    #[serde(flatten)]
    pub event: Event,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    Hello(HelloBody),
    /// Do Not Disturb, mutes, counts and so on changed (coalesced to at most one per 100 ms).
    State(Status),
    /// A notification arrived (shown or held).
    Notification(NotificationEvent),
    /// An app replaced a notification that is on screen.
    Updated(UpdatedEvent),
    /// A popup went away.
    Closed(ClosedEvent),
    /// rules.json was loaded or rejected.
    Config(ConfigEvent),
    /// A preview started or ended.
    Preview(PreviewEvent),
    /// The daemon refused this connection (for example too many connections).
    Error(ErrorBody),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HelloBody {
    pub protocol: u32,
    pub daemon: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationEvent {
    pub id: i64,
    pub app: String,
    pub summary: String,
    pub body: String,
    pub zone: String,
    pub origin: String,
    pub urgency: Urgency,
    pub shown: bool,
    pub why: String,
    pub transient: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdatedEvent {
    pub id: i64,
    pub summary: String,
    pub body: String,
    pub zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClosedEvent {
    pub id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigEvent {
    pub ok: bool,
    pub error: String,
    pub rules: u32,
    pub zones: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreviewEvent {
    pub state: PreviewState,
    #[serde(default)]
    pub shown: Option<u32>,
    #[serde(default)]
    pub seconds: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PreviewState {
    Started,
    Ended,
}

// ---------------------------------------------------------------- results

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Auto {
    pub fullscreen: bool,
    pub share: bool,
}

/// Answer to `status`, and the payload of `state` events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub dnd: bool,
    pub muted: Vec<String>,
    pub zones: Vec<String>,
    /// Popups currently on screen.
    pub active: u32,
    /// Popups waiting because their zone is at its cap.
    pub queued: u32,
    pub per_zone: BTreeMap<String, u32>,
    pub cap: u64,
    pub history: u32,
    pub cfg_error: String,
    pub auto: Auto,
    /// Why popups are being held right now ("" = not held).
    pub holding: String,
    pub previewing: bool,
    /// The focus mode that is on ("" = none).
    #[serde(default)]
    pub focus: String,
    /// Stacks of notifications (two or more together).
    #[serde(default)]
    pub stacks: Vec<StackInfo>,
}

/// A stack of notifications from one app/site/kind in one zone.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StackInfo {
    pub zone: String,
    pub key: String,
    pub count: u32,
    pub expanded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigStatus {
    pub ok: bool,
    pub error: String,
    pub rules: u32,
    pub zones: Vec<String>,
    pub screen_found: bool,
}

/// Answer to `configGet`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigGet {
    /// The file exactly as it is on disk.
    pub raw: String,
    /// The merged configuration the daemon is running (defaults + your file).
    pub effective: Config,
    pub error: String,
}

/// Answer to `validate`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidateResult {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub rules: Option<u32>,
    #[serde(default)]
    pub zones: Option<Vec<String>>,
}

/// A notification description for `simulate`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SimNotification {
    pub app_name: String,
    pub desktop_entry: String,
    pub origin: String,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
    /// The freedesktop category hint ("email.arrived"...), or empty.
    pub category: String,
    /// How it arrived: "app", "portal" or "web" (empty = decided from `origin`).
    pub channel: String,
    /// The app's requested timeout: -1 none, 0 never expire, otherwise milliseconds.
    pub expire_timeout: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SimRequest {
    /// Evaluate against this draft instead of the running config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Config>,
    pub notification: SimNotification,
    /// Ignore Do Not Disturb, mutes and full screen right now.
    pub ignore_state: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecidedBy {
    pub zone: Option<String>,
    pub timeout: Option<String>,
    pub silent: Option<String>,
}

/// The same information as [`DecidedBy`], as positions in the rules list (names can repeat or be missing).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecidedIdx {
    pub zone: Option<usize>,
    pub timeout: Option<usize>,
    pub silent: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SimResult {
    pub ok: bool,
    pub zone: String,
    /// Milliseconds; 0 = stays until dismissed.
    pub timeout: u64,
    pub silent: bool,
    pub would_show: bool,
    /// Why it would be held ("" = not held).
    pub hold: String,
    /// Names of every rule that matched, in order.
    pub matched: Vec<String>,
    pub decided_by: DecidedBy,
    /// Positions (0-based) of every rule that matched, in order.
    #[serde(default)]
    pub matched_idx: Vec<usize>,
    /// Positions of the rules that decided the zone, timeout and silent flag.
    #[serde(default)]
    pub decided_idx: DecidedIdx,
    /// The look and behaviour chosen: opacity, clickThrough, noFocus, width, fontFamily, fontSize, group.
    #[serde(default)]
    pub style: serde_json::Value,
}

/// One sample popup for `preview`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    pub reply: bool,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PreviewRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Config>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub samples: Option<Vec<Sample>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreviewResult {
    pub ok: bool,
    pub shown: u32,
    pub seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Milliseconds since the Unix epoch.
    pub t: i64,
    pub id: i64,
    pub app: String,
    pub summary: String,
    pub body: String,
    pub zone: String,
    pub origin: String,
    pub urgency: Urgency,
    pub shown: bool,
    /// Why it was not shown ("dnd", "fullscreen", "muted <zone>", "silent rule", ...); empty when shown.
    pub why: String,
    /// How many times the app replaced it.
    #[serde(default)]
    pub updates: Option<u32>,
}

/// One kind of notification an app sends (see `seenApps`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SeenType {
    /// `category|channel`, e.g. `email.arrived|app` or `general|web`.
    pub key: String,
    /// The freedesktop category hint; empty when the app sets none.
    pub category: String,
    /// `app` (D-Bus/libnotify/KDE), `portal` (sandboxed app) or `web` (browser page).
    pub channel: String,
    /// What notifications of this type can do: actions, reply, image, icon, progress, resident, transient, low, critical, sound.
    pub caps: Vec<String>,
    pub count: u64,
    pub last: i64,
}

/// An app that has sent notifications to the daemon (kept across restarts, independent of history).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SeenApp {
    /// Lower-cased desktop entry, or app name when there is no desktop entry.
    pub key: String,
    pub app: String,
    #[serde(rename = "desktopEntry")]
    pub desktop_entry: String,
    pub count: u64,
    /// Milliseconds since the Unix epoch.
    pub last: i64,
    /// The kinds of notification this app has sent, most used first.
    #[serde(default)]
    pub types: Vec<SeenType>,
}

// ---------------------------------------------------------------- config model (rules.json)

/// Serialize whole numbers as integers (`6000`, not `6000.0`) so files stay tidy.
fn ser_num<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        s.serialize_i64(*v as i64)
    } else {
        s.serialize_f64(*v)
    }
}

fn ser_opt_num<S: Serializer>(v: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
    match v {
        Some(n) => ser_num(n, s),
        None => s.serialize_none(),
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Anchor {
    #[serde(rename = "top-left")]
    TopLeft,
    #[serde(rename = "top-center")]
    TopCenter,
    #[serde(rename = "top-right")]
    TopRight,
    #[serde(rename = "middle-left")]
    MiddleLeft,
    #[serde(rename = "center")]
    Center,
    #[serde(rename = "middle-right")]
    MiddleRight,
    #[serde(rename = "bottom-left")]
    BottomLeft,
    #[serde(rename = "bottom-center")]
    BottomCenter,
    #[serde(rename = "bottom-right")]
    BottomRight,
    /// A position the user chose: see `Zone::x` and `Zone::y`.
    #[serde(rename = "free")]
    Free,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Zone {
    pub anchor: Anchor,
    /// Only for `free` zones: where on the screen, 0..1 from the left edge.
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "ser_opt_num")]
    pub x: Option<f64>,
    /// Only for `free` zones: 0..1 from the top edge (a zone in the lower half grows upward).
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "ser_opt_num")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Background opacity of the popup, 0.1 to 1 (text stays solid).
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "ser_opt_num")]
    pub opacity: Option<f64>,
    /// Clicks pass through the popup to the window underneath.
    #[serde(default, rename = "clickThrough", skip_serializing_if = "Option::is_none")]
    pub click_through: Option<bool>,
    /// The popup never takes keyboard focus (so it has no reply box).
    #[serde(default, rename = "noFocus", skip_serializing_if = "Option::is_none")]
    pub no_focus: Option<bool>,
    /// Popup width in pixels (120 to 4000).
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "ser_opt_num")]
    pub width: Option<f64>,
    #[serde(default, rename = "fontFamily", skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    /// Body text size in pixels (6 to 48).
    #[serde(default, rename = "fontSize", skip_serializing_if = "Option::is_none", serialize_with = "ser_opt_num")]
    pub font_size: Option<f64>,
    /// low, normal, high or urgent. Urgent passes Do Not Disturb and focus modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    /// Show large pictures and audio/video players in the popup (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<bool>,
    /// Stack notifications of the same app/site/kind into one place with a count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<bool>,
}

impl Zone {
    /// A zone at `anchor` with nothing else set.
    pub fn at(anchor: Anchor) -> Zone {
        Zone { anchor, x: None, y: None, accent: None, label: None, opacity: None, click_through: None, no_focus: None, width: None, font_family: None, font_size: None, priority: None, media: None, stack: None }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Field name -> regex (JavaScript syntax, case-insensitive). Fields: appName, desktopEntry, origin, summary, body, urgency.
    #[serde(default, rename = "match", skip_serializing_if = "BTreeMap::is_empty")]
    pub matches: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "ser_opt_num")]
    pub timeout: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silent: Option<bool>,
    /// Background opacity of the popup, 0.1 to 1 (text stays solid).
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "ser_opt_num")]
    pub opacity: Option<f64>,
    /// Clicks pass through the popup to the window underneath.
    #[serde(default, rename = "clickThrough", skip_serializing_if = "Option::is_none")]
    pub click_through: Option<bool>,
    /// The popup never takes keyboard focus (so it has no reply box).
    #[serde(default, rename = "noFocus", skip_serializing_if = "Option::is_none")]
    pub no_focus: Option<bool>,
    /// Popup width in pixels (120 to 4000).
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "ser_opt_num")]
    pub width: Option<f64>,
    #[serde(default, rename = "fontFamily", skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    /// Body text size in pixels (6 to 48).
    #[serde(default, rename = "fontSize", skip_serializing_if = "Option::is_none", serialize_with = "ser_opt_num")]
    pub font_size: Option<f64>,
    /// Notifications with the same group name (in one zone) are kept together under a heading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// low, normal, high or urgent. Urgent passes Do Not Disturb and focus modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    /// Show large pictures and audio/video players in the popup (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<bool>,
    /// Stack notifications of the same app/site/kind into one place with a count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct DndSettings {
    #[serde(rename = "allowCritical")]
    pub allow_critical: bool,
}
impl Default for DndSettings {
    fn default() -> Self {
        Self { allow_critical: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct HistorySettings {
    pub limit: u32,
    pub persist: bool,
}
impl Default for HistorySettings {
    fn default() -> Self {
        Self { limit: 200, persist: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct SoundSettings {
    pub enabled: bool,
    #[serde(rename = "defaultName")]
    pub default_name: String,
}
impl Default for SoundSettings {
    fn default() -> Self {
        Self { enabled: true, default_name: String::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct AutoDndSettings {
    pub fullscreen: bool,
    #[serde(rename = "screenShare")]
    pub screen_share: bool,
    /// Internal: false stops the watchers (tests).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch: Option<bool>,
}
impl Default for AutoDndSettings {
    fn default() -> Self {
        Self { fullscreen: true, screen_share: true, watch: None }
    }
}

/// One focus mode: only notifications at or above `min_priority`, or matching an `allow` entry, are shown while it is on.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FocusMode {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// low, normal, high or urgent (default urgent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_priority: Option<String>,
    /// Each entry is a set of patterns like a rule's `match`; a notification matching all of one entry always gets through.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct FocusSettings {
    pub modes: BTreeMap<String, FocusMode>,
}
impl Default for FocusSettings {
    fn default() -> Self {
        Self { modes: BTreeMap::from([("urgent".to_string(), FocusMode { label: Some("Urgent only".into()), min_priority: Some("urgent".into()), allow: Vec::new() })]) }
    }
}

/// Graphs of downloads and transfers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct TransferSettings {
    /// bars, line or heatmap.
    pub chart: String,
}
impl Default for TransferSettings {
    fn default() -> Self {
        Self { chart: "line".into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default, rename_all = "camelCase")]
pub struct Theme {
    pub card: String,
    pub border: String,
    pub text: String,
    pub subtext: String,
    pub button: String,
    pub button_hover: String,
    /// Popup background opacity, 0.1 to 1.
    #[serde(serialize_with = "ser_num")]
    pub opacity: f64,
    /// "" = the system font.
    pub font_family: String,
    /// Body text size in pixels.
    #[serde(serialize_with = "ser_num")]
    pub font_size: f64,
}
impl Default for Theme {
    fn default() -> Self {
        Self {
            card: "#1d1f27".into(),
            border: "#2c2f3a".into(),
            text: "#f2f3f7".into(),
            subtext: "#b9bccb".into(),
            button: "#272a36".into(),
            button_hover: "#323646".into(),
            opacity: 1.0,
            font_family: String::new(),
            font_size: 12.0,
        }
    }
}

/// The whole `rules.json`. Unknown keys are rejected (the daemon would silently ignore them, which hides typos).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default, rename_all = "camelCase")]
pub struct Config {
    pub screen: String,
    pub debug: bool,
    pub default_zone: String,
    #[serde(serialize_with = "ser_num")]
    pub default_timeout: f64,
    pub max_visible: u32,
    #[serde(serialize_with = "ser_num")]
    pub width: f64,
    #[serde(serialize_with = "ser_num")]
    pub margin: f64,
    #[serde(serialize_with = "ser_num")]
    pub spacing: f64,
    pub dnd: DndSettings,
    pub history: HistorySettings,
    pub sound: SoundSettings,
    pub auto_dnd: AutoDndSettings,
    pub theme: Theme,
    pub focus: FocusSettings,
    pub transfer: TransferSettings,
    pub zones: BTreeMap<String, Zone>,
    pub rules: Vec<Rule>,
}

impl Default for Config {
    /// The same defaults the daemon starts with.
    fn default() -> Self {
        let mut zones = BTreeMap::new();
        zones.insert(
            "personal".to_string(),
            Zone { accent: Some("#60a5fa".into()), label: Some("Personal".into()), ..Zone::at(Anchor::TopRight) },
        );
        Self {
            screen: String::new(),
            debug: false,
            default_zone: "personal".into(),
            default_timeout: 6000.0,
            max_visible: 5,
            width: 380.0,
            margin: 16.0,
            spacing: 10.0,
            dnd: DndSettings::default(),
            history: HistorySettings::default(),
            sound: SoundSettings::default(),
            auto_dnd: AutoDndSettings::default(),
            theme: Theme::default(),
            focus: FocusSettings::default(),
            transfer: TransferSettings::default(),
            zones,
            rules: Vec::new(),
        }
    }
}

impl Config {
    /// Parse a rules.json text (structure only; the daemon judges regexes and zone references).
    pub fn from_json(text: &str) -> Result<Config, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Pretty-print the way the shipped file is written.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("Config always serializes") + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hello_and_events() {
        let m = Incoming::parse(r#"{"event":"hello","protocol":1,"daemon":"0.6.0","pid":42,"seq":0}"#).unwrap();
        match m {
            Incoming::Event(e) => assert_eq!(e.event, Event::Hello(HelloBody { protocol: 1, daemon: "0.6.0".into(), pid: 42 })),
            _ => panic!("expected an event"),
        }
        let m = Incoming::parse(r#"{"event":"closed","seq":9,"t":1,"id":5}"#).unwrap();
        assert!(matches!(m, Incoming::Event(e) if e.seq == 9 && e.event == Event::Closed(ClosedEvent { id: 5 })));
    }

    #[test]
    fn parses_responses() {
        let ok = Incoming::parse(r#"{"id":3,"result":{"pong":true}}"#).unwrap();
        assert!(matches!(ok, Incoming::Response(r) if r.id == Some(3) && r.error.is_none()));
        let err = Incoming::parse(r#"{"id":null,"error":{"code":"bad_request","message":"x"}}"#).unwrap();
        assert!(matches!(err, Incoming::Response(r) if r.id.is_none() && r.error.as_ref().unwrap().code == "bad_request"));
    }

    #[test]
    fn config_round_trips_and_keeps_integers_tidy() {
        let c = Config::default();
        let text = c.to_json_pretty();
        assert!(text.contains("\"defaultTimeout\": 6000,"), "whole numbers must serialize as integers:\n{text}");
        assert_eq!(Config::from_json(&text).unwrap(), c);
    }

    #[test]
    fn config_rejects_typos_and_bad_anchors() {
        assert!(Config::from_json(r#"{"defualtZone":"x"}"#).is_err());
        assert!(Config::from_json(r#"{"zones":{"a":{"anchor":"middle"}}}"#).is_err());
        assert!(Config::from_json(r#"{"rules":[{"nme":"x"}]}"#).is_err());
    }

    #[test]
    fn partial_config_fills_defaults() {
        let c = Config::from_json(r#"{"width": 500, "rules":[{"name":"r","match":{"appName":"a"},"zone":"personal"}]}"#).unwrap();
        assert_eq!(c.width, 500.0);
        assert_eq!(c.default_timeout, 6000.0);
        assert_eq!(c.rules[0].matches["appName"], "a");
    }
}
