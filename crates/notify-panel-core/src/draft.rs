//! The rules editor's working copy ("draft") and saving it.
//!
//! The draft is kept as raw JSON, not a typed struct, so keys this panel does not know about survive an edit
//! and save untouched. All editing is plain functions on that JSON; the daemon (`validate`, `simulate`) is the
//! authority on whether it is acceptable and what it does.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value, json};

/// The fields a rule can match on, in the order the editor shows them.
/// What a rule can test. `group` is the group (zone) a notification is in, decided by the rules that do not test it.
pub const MATCH_FIELDS: [&str; 9] = ["appName", "desktopEntry", "origin", "summary", "body", "urgency", "category", "channel", "group"];

/// How a popup looks and behaves: settable on a rule, on a zone, and (opacity, fontFamily, fontSize) on the theme. `group` is for rules only.
pub const STYLE_KEYS: [&str; 11] = ["opacity", "clickThrough", "noFocus", "width", "fontFamily", "fontSize", "group", "priority", "media", "stack", "timerStyle"];


/// True when a rule still changes something (otherwise a panel-managed rule is removed).
fn rule_has_effect(r: &Value) -> bool {
    ["silent", "zone", "timeout"].iter().chain(STYLE_KEYS.iter()).any(|k| r.get(*k).is_some())
}

#[derive(Debug, Clone)]
pub struct Draft {
    value: Value,
    base_raw: String,
    base_value: Value,
}



impl Draft {
    /// Start from the file's text. Fails if it is not a JSON object.
    pub fn from_raw(raw: &str) -> Result<Draft, String> {
        let value: Value = serde_json::from_str(raw).map_err(|e| format!("not valid JSON: {e}"))?;
        if !value.is_object() {
            return Err("the top level must be a JSON object".into());
        }
        Ok(Draft { base_value: value.clone(), value, base_raw: raw.to_string() })
    }

    pub fn empty() -> Draft {
        Draft { value: json!({}), base_raw: String::new(), base_value: json!({}) }
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    /// The text that would be written: pretty JSON, two-space indent, trailing newline (as the shipped file).
    pub fn to_text(&self) -> String {
        serde_json::to_string_pretty(&self.value).expect("a Value always serializes") + "\n"
    }

    /// The file text this draft was started from (used to detect hand edits on disk).
    pub fn base_raw(&self) -> &str {
        &self.base_raw
    }

    pub fn is_dirty(&self) -> bool {
        self.value != self.base_value
    }

    /// Throw the edits away.
    pub fn revert(&mut self) {
        self.value = self.base_value.clone();
    }

    /// The draft now matches what is on disk (after a successful save).
    pub fn mark_saved(&mut self, text: &str) {
        self.base_raw = text.to_string();
        self.base_value = serde_json::from_str(text).unwrap_or_else(|_| self.value.clone());
        self.value = self.base_value.clone();
    }

    fn rules_mut(&mut self) -> &mut Vec<Value> {
        let obj = self.value.as_object_mut().expect("draft is an object");
        let e = obj.entry("rules").or_insert_with(|| json!([]));
        if !e.is_array() {
            *e = json!([]);
        }
        e.as_array_mut().unwrap()
    }

    fn rule_mut(&mut self, i: usize) -> Option<&mut Map<String, Value>> {
        self.rules_mut().get_mut(i).and_then(Value::as_object_mut)
    }

    pub fn rule_count(&self) -> usize {
        self.value.get("rules").and_then(Value::as_array).map(Vec::len).unwrap_or(0)
    }

    /// Zone names a rule can send popups to (the draft's own zones; the daemon's default zone if none are defined).
    pub fn zone_names(&self) -> Vec<String> {
        match self.value.get("zones").and_then(Value::as_object) {
            Some(z) if !z.is_empty() => z.keys().cloned().collect(),
            _ => vec!["personal".to_string()],
        }
    }

    pub fn rules(&self) -> Vec<RuleView> {
        let Some(list) = self.value.get("rules").and_then(Value::as_array) else { return Vec::new() };
        list.iter().enumerate().map(|(index, r)| rule_view(index, r)).collect()
    }

    // ------------------------------------------------------------ editing

    /// Append a rule that matches everything and changes nothing yet. Returns its position.
    pub fn add_rule(&mut self) -> usize {
        let r = self.rules_mut();
        r.push(json!({"name": "New rule", "match": {}}));
        r.len() - 1
    }

    pub fn delete_rule(&mut self, i: usize) {
        let r = self.rules_mut();
        if i < r.len() {
            r.remove(i);
        }
    }

    /// Move a rule up (-1) or down (+1). Returns its new position.
    pub fn move_rule(&mut self, i: usize, delta: i32) -> usize {
        let r = self.rules_mut();
        let j = i as i64 + delta as i64;
        if i >= r.len() || j < 0 || j as usize >= r.len() {
            return i;
        }
        r.swap(i, j as usize);
        j as usize
    }

    pub fn duplicate_rule(&mut self, i: usize) -> Option<usize> {
        let r = self.rules_mut();
        let mut copy = r.get(i)?.clone();
        if let Some(n) = copy.get("name").and_then(Value::as_str).map(|n| format!("{n} (copy)")) {
            copy["name"] = json!(n);
        }
        r.insert(i + 1, copy);
        Some(i + 1)
    }

    pub fn set_name(&mut self, i: usize, name: &str) {
        if let Some(r) = self.rule_mut(i) {
            if name.is_empty() {
                r.remove("name");
            } else {
                r.insert("name".into(), json!(name));
            }
        }
    }

    /// Set the regex a rule tests for `field`; an empty pattern removes the test.
    pub fn set_match(&mut self, i: usize, field: &str, pattern: &str) {
        if let Some(r) = self.rule_mut(i) {
            let m = r.entry("match").or_insert_with(|| json!({}));
            if !m.is_object() {
                *m = json!({});
            }
            let m = m.as_object_mut().unwrap();
            if pattern.is_empty() {
                m.remove(field);
            } else {
                m.insert(field.into(), json!(pattern));
            }
        }
    }

    /// Send matching popups to `zone`; `None` leaves the zone to later rules and the default.
    pub fn set_zone(&mut self, i: usize, zone: Option<&str>) {
        if let Some(r) = self.rule_mut(i) {
            match zone.filter(|z| !z.is_empty()) {
                Some(z) => r.insert("zone".into(), json!(z)),
                None => r.remove("zone"),
            };
        }
    }

    /// Milliseconds; `Some(0)` = stays until dismissed; `None` = not set by this rule.
    pub fn set_timeout(&mut self, i: usize, ms: Option<f64>) {
        if let Some(r) = self.rule_mut(i) {
            match ms {
                Some(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => r.insert("timeout".into(), json!(n as i64)),
                Some(n) => r.insert("timeout".into(), json!(n)),
                None => r.remove("timeout"),
            };
        }
    }

    pub fn set_silent(&mut self, i: usize, silent: bool) {
        if let Some(r) = self.rule_mut(i) {
            if silent {
                r.insert("silent".into(), json!(true));
            } else {
                r.remove("silent");
            }
        }
    }

    // ------------------------------------------------------------ settings and zones

    fn zones_mut(&mut self) -> &mut Map<String, Value> {
        let obj = self.value.as_object_mut().expect("draft is an object");
        let e = obj.entry("zones").or_insert_with(|| json!({}));
        if !e.is_object() {
            *e = json!({});
        }
        e.as_object_mut().unwrap()
    }

    // ------------------------------------------------------------ focus modes

    /// What the daemon uses when the file has no `focus` section.
    fn default_focus() -> Value {
        json!({"modes": {"urgent": {"label": "Urgent only", "minPriority": "urgent"}}})
    }

    fn focus_modes_mut(&mut self) -> &mut Map<String, Value> {
        let obj = self.value.as_object_mut().expect("draft is an object");
        // the first edit writes the built-in mode out, so it is not lost when other modes are added
        let f = obj.entry("focus").or_insert_with(Draft::default_focus);
        if !f.is_object() {
            *f = json!({});
        }
        let m = f.as_object_mut().unwrap().entry("modes").or_insert_with(|| json!({}));
        if !m.is_object() {
            *m = json!({});
        }
        m.as_object_mut().unwrap()
    }

    // ------------------------------------------------------------ per-app routing (one managed rule per app)

    /// The panel-managed rule whose match is exactly these (field, text) pairs.
    fn app_rule_index(&self, matches: &[(&str, &str)]) -> Option<usize> {
        let want: Vec<(String, String)> = matches.iter().map(|(f, v)| (f.to_string(), format!("^{}$", regex_escape(v)))).collect();
        self.value.get("rules")?.as_array()?.iter().position(|r| {
            r.get("name").and_then(Value::as_str).is_some_and(|n| n.starts_with(APP_RULE_PREFIX))
                && r.get("match").and_then(Value::as_object).is_some_and(|m| m.len() == want.len() && want.iter().all(|(f, v)| m.get(f).and_then(Value::as_str) == Some(v.as_str())))
        })
    }

    /// Set a look/behaviour key (opacity, clickThrough, noFocus, width, fontFamily, fontSize, group) on rule `i`. Empty text = not set.
    pub fn set_rule_style(&mut self, i: usize, key: &str, text: &str) -> Result<(), String> {
        if !STYLE_KEYS.contains(&key) {
            return Err(format!("unknown setting '{key}'"));
        }
        let r = self.rule_mut(i).ok_or_else(|| "no such rule".to_string())?;
        set_outcome(r, key, text)
    }

    /// Which rule an error from the daemon is about ("rule 'name': ..." or "rule #3: ..."), if any.
    pub fn error_index(&self, error: &str) -> Option<usize> {
        let rest = error.strip_prefix("rule ")?;
        if let Some(n) = rest.strip_prefix('#') {
            let num: String = n.chars().take_while(char::is_ascii_digit).collect();
            return num.parse::<usize>().ok().and_then(|n| n.checked_sub(1)).filter(|i| *i < self.rule_count());
        }
        let name = rest.strip_prefix('\'')?.split("':").next()?;
        self.rules().iter().position(|r| r.name == name)
    }
}


/// The places a zone can be: a 3x3 grid (row by row) and `free`.
pub const ANCHORS: [&str; 10] = ["top-left", "top-center", "top-right", "middle-left", "center", "middle-right", "bottom-left", "bottom-center", "bottom-right", "free"];

/// Rules the panel creates for "this app" start with this, so it can find (and tidy up) its own.
pub const APP_RULE_PREFIX: &str = "App: ";



#[derive(Clone, Copy)]
enum Kind {
    Text(&'static str),
    Color(&'static str),
    Bool(bool),
    Num(f64),
}


/// Every scalar setting the panel can edit, with the daemon's default (what it uses when the key is missing).
const SETTING_DEFAULTS: &[(&str, Kind)] = &[
    ("screen", Kind::Text("")),
    ("defaultZone", Kind::Text("personal")),
    ("defaultTimeout", Kind::Num(6000.0)),
    ("maxVisible", Kind::Num(5.0)),
    ("width", Kind::Num(380.0)),
    ("margin", Kind::Num(16.0)),
    ("spacing", Kind::Num(10.0)),
    ("dnd.allowCritical", Kind::Bool(true)),
    ("history.limit", Kind::Num(200.0)),
    ("history.persist", Kind::Bool(false)),
    ("sound.enabled", Kind::Bool(true)),
    ("sound.defaultName", Kind::Text("")),
    ("autoDnd.fullscreen", Kind::Bool(true)),
    ("autoDnd.screenShare", Kind::Bool(true)),
    ("theme.card", Kind::Color("#1d1f27")),
    ("theme.border", Kind::Color("#2c2f3a")),
    ("theme.text", Kind::Color("#f2f3f7")),
    ("theme.subtext", Kind::Color("#b9bccb")),
    ("theme.button", Kind::Color("#272a36")),
    ("theme.buttonHover", Kind::Color("#323646")),
    ("theme.opacity", Kind::Num(1.0)),
    ("theme.fontFamily", Kind::Text("")), ("theme.fontSize", Kind::Num(12.0)),
    ("theme.timerStyle", Kind::Text("none")),
    ("theme.radius", Kind::Num(12.0)),
    ("theme.tokens", Kind::Text("auto")),
    ("transfer.chart", Kind::Text("line")),
    ("center.anchor", Kind::Text("bottom-right")),
    ("center.keep", Kind::Num(50.0)),
];

fn lookup<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |cur, k| cur.get(k))
}


// ---------------------------------------------------------------- the simulator's input


mod rule_view;
mod style;
pub use rule_view::RuleView;
use rule_view::{number_text, rule_view};
use apps::regex_escape;
use style::{outcome_text, set_outcome, style_kind};
mod apps;
mod focus;
mod save;
mod settings;
mod zones;
pub use apps::AppRouting;
pub use save::*;

#[cfg(test)]
mod tests;
