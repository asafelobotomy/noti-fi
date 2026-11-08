//! Simulation, preview, history and "seen apps" messages.


use serde::{Deserialize, Serialize};

use crate::{Config, Urgency};

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
    /// The sender's desktop entry, the freedesktop category and how it arrived (`app`, `portal`, `web`).
    /// Empty in entries recorded before 0.15.
    #[serde(default, rename = "desktopEntry")]
    pub desktop_entry: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub channel: String,
    /// Read in the notification center, or dismissed by the user (entries from before 0.18 are unread).
    #[serde(default)]
    pub read: bool,
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
