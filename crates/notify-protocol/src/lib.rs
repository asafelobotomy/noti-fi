//! Typed messages for the notifd control socket, and the `rules.json` config model.
//!
//! Wire format: one JSON object per line.
//!
//! ```text
//! server -> client (first):  {"event":"hello","protocol":1,"daemon":"0.8.0","pid":123,"seq":0}
//! client -> server:          {"id":1,"method":"status","params":{}}
//! server -> client:          {"id":1,"result":{...}}   or   {"id":1,"error":{"code":"...","message":"..."}}
//! after "subscribe":         {"event":"state"|"notification"|"updated"|"closed"|"config"|"preview","seq":N,"t":ms,...}
//! ```
//!
//! This crate does no I/O. The daemon is the authority on behaviour; these types describe its messages.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
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
    /// "notifd": lets a client tell it reached notifd (absent before 0.18).
    #[serde(default)]
    pub name: String,
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
    #[serde(default)]
    pub name: String,
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
    /// History entries not yet read, in all and per zone.
    #[serde(default)]
    pub unread: u32,
    #[serde(default)]
    pub unread_per_zone: BTreeMap<String, u32>,
    /// The notification center is open.
    #[serde(default)]
    pub center: bool,
    /// Notifications kept open for their apps after their popup left (see `center.keep`).
    #[serde(default)]
    pub kept: u32,
    /// Apps holding notifications through Inhibit (`holding` is then "inhibited", unless Do Not Disturb is on).
    #[serde(default)]
    pub inhibitors: Vec<Inhibitor>,
    /// The screen is being shared (automatic Do Not Disturb's screen-share watch): notification text is hidden.
    #[serde(default)]
    pub sharing: bool,
    /// The newest notification (its title is empty while the screen is shared); None with an empty history.
    #[serde(default)]
    pub latest: Option<Latest>,
}

/// The newest notification, as a bar shows it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Latest {
    pub id: i64,
    pub t: i64,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub read: bool,
}

/// An app that asked for notifications to be held.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Inhibitor {
    pub app: String,
    pub reason: String,
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

mod sim;
pub use sim::*;

mod config;
pub use config::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hello_and_events() {
        let m = Incoming::parse(r#"{"event":"hello","protocol":1,"daemon":"0.6.0","pid":42,"seq":0}"#).unwrap();
        match m {
            Incoming::Event(e) => assert_eq!(e.event, Event::Hello(HelloBody { name: String::new(), protocol: 1, daemon: "0.6.0".into(), pid: 42 })),
            _ => panic!("expected an event"),
        }
        let m = Incoming::parse(r#"{"event":"hello","name":"notifd","protocol":1,"daemon":"0.18.0","pid":7,"seq":0}"#).unwrap();
        assert!(matches!(m, Incoming::Event(e) if matches!(&e.event, Event::Hello(h) if h.name == "notifd")), "newer daemons name themselves");
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
