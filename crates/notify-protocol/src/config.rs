//! The `rules.json` configuration model (zones, rules, theme, focus modes...). Unknown keys are rejected: the daemon would silently ignore them, which hides typos.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, Serializer};


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
    #[serde(default, rename = "timerStyle", skip_serializing_if = "Option::is_none")]
    pub timer_style: Option<String>,
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
        Zone { anchor, x: None, y: None, accent: None, label: None, opacity: None, click_through: None, no_focus: None, width: None, font_family: None, font_size: None, timer_style: None, priority: None, media: None, stack: None }
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
    #[serde(default, rename = "timerStyle", skip_serializing_if = "Option::is_none")]
    pub timer_style: Option<String>,
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
    /// Programs whose screen captures do not count as sharing (binary or application name).
    #[serde(rename = "ignoreCaptureBy", default, skip_serializing_if = "Vec::is_empty")]
    pub ignore_capture_by: Vec<String>,
}
impl Default for AutoDndSettings {
    fn default() -> Self {
        Self { fullscreen: true, screen_share: true, watch: None, ignore_capture_by: vec![] }
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

/// The notification center (the list the tray icon opens).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct CenterSettings {
    /// Where it opens, next to the tray: any popup anchor (top-left ... bottom-right, top-center, center ...).
    pub anchor: String,
    /// How many notifications whose popup has gone stay open for their apps, so their buttons work from the list. 0 = none.
    pub keep: u32,
}
impl Default for CenterSettings {
    fn default() -> Self {
        Self { anchor: "bottom-right".into(), keep: 50 }
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
    /// How a popup shows its time left: none, or any of line, circle, frame separated by commas.
    pub timer_style: String,
    /// Corner radius of popups and the notification list, in pixels.
    #[serde(serialize_with = "ser_num")]
    pub radius: f64,
    /// A desktop's design tokens as the base of the theme: "auto" (uQ's tokens.json when present), "" or a file path.
    pub tokens: String,
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
            timer_style: "none".into(),
            radius: 12.0,
            tokens: "auto".into(),
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
    pub center: CenterSettings,
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
            center: CenterSettings::default(),
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

