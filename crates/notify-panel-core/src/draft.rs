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
pub const MATCH_FIELDS: [&str; 8] = ["appName", "desktopEntry", "origin", "summary", "body", "urgency", "category", "channel"];

/// How a popup looks and behaves: settable on a rule, on a zone, and (opacity, fontFamily, fontSize) on the theme. `group` is for rules only.
pub const STYLE_KEYS: [&str; 10] = ["opacity", "clickThrough", "noFocus", "width", "fontFamily", "fontSize", "group", "priority", "media", "stack"];

enum StyleKind {
    Num,
    Flag,
    Text,
}

fn style_kind(key: &str) -> Option<StyleKind> {
    Some(match key {
        "opacity" | "width" | "fontSize" | "timeout" => StyleKind::Num,
        "clickThrough" | "noFocus" | "silent" | "media" | "stack" => StyleKind::Flag,
        "fontFamily" | "group" | "zone" | "priority" => StyleKind::Text,
        _ => return None,
    })
}

/// Set (or, for empty text, remove) one outcome key on a rule or zone object. Flags: "true"/"false" are both stored (an explicit false overrides
/// a later rule); `silent` false removes it.
fn set_outcome(o: &mut Map<String, Value>, key: &str, text: &str) -> Result<(), String> {
    let kind = style_kind(key).ok_or_else(|| format!("unknown setting '{key}'"))?;
    let t = text.trim();
    let value = match kind {
        StyleKind::Text => (!t.is_empty()).then(|| json!(t)),
        StyleKind::Flag => match t {
            "" => None,
            "false" if key == "silent" => None,
            "true" => Some(json!(true)),
            "false" => Some(json!(false)),
            _ => return Err(format!("'{t}' is not true or false")),
        },
        StyleKind::Num => {
            if t.is_empty() {
                None
            } else {
                let n: f64 = t.parse().map_err(|_| format!("'{t}' is not a number"))?;
                if !n.is_finite() || n < 0.0 {
                    return Err("must be zero or more".into());
                }
                Some(if n.fract() == 0.0 { json!(n as i64) } else { json!((n * 1000.0).round() / 1000.0) })
            }
        }
    };
    match value {
        Some(v) => o.insert(key.into(), v),
        None => o.remove(key),
    };
    Ok(())
}

/// The value of an outcome key as text ("" = not set).
fn outcome_text(o: &Value, key: &str) -> String {
    match o.get(key) {
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::String(t)) => t.clone(),
        Some(v) => number_text(v),
        None => String::new(),
    }
}

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

/// One rule as the UI shows it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RuleView {
    pub index: usize,
    /// The name, or "(unnamed rule)".
    pub title: String,
    pub name: String,
    /// "origin ~ example-shop|example-chat  →  business"
    pub summary: String,
    /// Pattern per match field (empty string when the rule does not test that field).
    #[serde(rename = "match")]
    pub matches: Map<String, Value>,
    pub zone: String,
    /// Milliseconds as text ("" = not set); "0" means the popup stays until dismissed.
    pub timeout: String,
    pub silent: bool,
    /// Look and behaviour keys as text ("" = not set): opacity, clickThrough ("", "true", "false"), noFocus, width, fontFamily, fontSize, group.
    pub style: Map<String, Value>,
}

fn number_text(v: &Value) -> String {
    match v.as_f64() {
        Some(n) if n.fract() == 0.0 => format!("{}", n as i64),
        Some(n) => n.to_string(),
        None => String::new(),
    }
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

    /// Everything the Zones and Appearance pages show, with the daemon's defaults filled in for missing keys.
    pub fn settings_view(&self) -> Value {
        let g = |path: &str| lookup(&self.value, path).cloned();
        let mut out = Map::new();
        for (path, default) in SETTING_DEFAULTS {
            out.insert((*path).into(), g(path).unwrap_or_else(|| default_value(default)));
        }
        let mut zones = Vec::new();
        if let Some(z) = self.value.get("zones").and_then(Value::as_object) {
            for (id, v) in z {
                let used = self.rules().iter().filter(|r| r.zone == *id).count();
                zones.push(json!({
                    "id": id,
                    "label": v.get("label").and_then(Value::as_str).unwrap_or(""),
                    "anchor": v.get("anchor").and_then(Value::as_str).unwrap_or("top-right"),
                    "accent": v.get("accent").and_then(Value::as_str).unwrap_or(""),
                    "style": STYLE_KEYS.iter().filter(|k| **k != "group").map(|k| (k.to_string(), json!(outcome_text(v, k)))).collect::<Map<String, Value>>(),
                    "x": v.get("x").and_then(Value::as_f64).unwrap_or(0.5),
                    "y": v.get("y").and_then(Value::as_f64).unwrap_or(0.5),
                    "rules": used,
                }));
            }
        }
        json!({"values": out, "zones": zones})
    }

    /// Set a scalar setting by dotted path (`theme.card`, `sound.enabled`, `maxVisible`...). Only known settings are accepted.
    /// Text for a text setting; "" removes the key (the daemon then uses its default). Numbers and booleans are parsed.
    pub fn set_setting(&mut self, path: &str, text: &str) -> Result<(), String> {
        let Some((_, kind)) = SETTING_DEFAULTS.iter().find(|(p, _)| *p == path) else { return Err(format!("unknown setting '{path}'")) };
        let value = match *kind {
            Kind::Text(_) | Kind::Color(_) => {
                if text.is_empty() { None } else { Some(json!(text)) }
            }
            Kind::Bool(_) => match text {
                "true" => Some(json!(true)),
                "false" => Some(json!(false)),
                _ => return Err(format!("'{text}' is not true or false")),
            },
            Kind::Num(_) => {
                let t = text.trim();
                if t.is_empty() {
                    None
                } else {
                    let n: f64 = t.parse().map_err(|_| format!("'{t}' is not a number"))?;
                    if n < 0.0 || !n.is_finite() {
                        return Err("must be zero or more".into());
                    }
                    Some(if n.fract() == 0.0 { json!(n as i64) } else { json!(n) })
                }
            }
        };
        let parts: Vec<&str> = path.split('.').collect();
        let mut cur = self.value.as_object_mut().expect("draft is an object");
        for k in &parts[..parts.len() - 1] {
            let e = cur.entry(*k).or_insert_with(|| json!({}));
            if !e.is_object() {
                *e = json!({});
            }
            cur = e.as_object_mut().unwrap();
        }
        let last = parts[parts.len() - 1];
        match value {
            Some(v) => cur.insert(last.into(), v),
            None => cur.remove(last),
        };
        Ok(())
    }

    fn zones_mut(&mut self) -> &mut Map<String, Value> {
        let obj = self.value.as_object_mut().expect("draft is an object");
        let e = obj.entry("zones").or_insert_with(|| json!({}));
        if !e.is_object() {
            *e = json!({});
        }
        e.as_object_mut().unwrap()
    }

    /// Add a zone in the first free corner (or the top-right). Returns its id.
    pub fn add_zone(&mut self) -> String {
        let zones = self.zones_mut();
        let mut n = zones.len() + 1;
        while zones.contains_key(&format!("zone{n}")) {
            n += 1;
        }
        let id = format!("zone{n}");
        let used: Vec<&str> = zones.values().filter_map(|z| z.get("anchor").and_then(Value::as_str)).collect();
        let anchor = ["top-right", "top-left", "bottom-right", "bottom-left", "top-center", "bottom-center", "middle-right", "middle-left", "center"].into_iter().find(|a| !used.contains(a)).unwrap_or("top-right");
        zones.insert(id.clone(), json!({"anchor": anchor, "accent": "#34d399", "label": format!("Zone {n}")}));
        id
    }

    /// Remove a zone. Refused for the last zone and for the default zone; rules that sent popups there stop choosing a zone.
    pub fn delete_zone(&mut self, id: &str) -> Result<usize, String> {
        if self.zones_mut().len() <= 1 {
            return Err("there must be at least one zone".into());
        }
        if self.value.get("defaultZone").and_then(Value::as_str) == Some(id) {
            return Err("this is the default zone; choose another default first".into());
        }
        if self.zones_mut().remove(id).is_none() {
            return Err(format!("no zone '{id}'"));
        }
        let mut cleared = 0;
        for r in self.rules_mut().iter_mut().filter_map(Value::as_object_mut) {
            if r.get("zone").and_then(Value::as_str) == Some(id) {
                r.remove("zone");
                cleared += 1;
            }
        }
        Ok(cleared)
    }

    /// `field` is `label`, `anchor` or `accent`. An empty label or accent removes it.
    pub fn set_zone_field(&mut self, id: &str, field: &str, text: &str) -> Result<(), String> {
        if !["label", "anchor", "accent"].contains(&field) {
            return Err(format!("unknown zone field '{field}'"));
        }
        if field == "anchor" && !ANCHORS.contains(&text) {
            return Err(format!("'{text}' is not a place popups can go"));
        }
        let Some(z) = self.zones_mut().get_mut(id).and_then(Value::as_object_mut) else { return Err(format!("no zone '{id}'")) };
        if text.is_empty() && field != "anchor" {
            z.remove(field);
        } else {
            z.insert(field.into(), json!(text));
        }
        if field == "anchor" {
            if text == "free" {
                // a free zone needs a position; start in the middle of the screen
                z.entry("x").or_insert(json!(0.5));
                z.entry("y").or_insert(json!(0.5));
            } else {
                z.remove("x");
                z.remove("y");
            }
        }
        Ok(())
    }

    /// Put a zone at an exact place on the screen: `x` and `y` are 0..1 from the left and top edges. Makes the zone `free`.
    pub fn set_zone_position(&mut self, id: &str, x: f64, y: f64) -> Result<(), String> {
        if !(x.is_finite() && y.is_finite()) {
            return Err("the position must be a number".into());
        }
        let r = |v: f64| (v.clamp(0.0, 1.0) * 1000.0).round() / 1000.0;
        let Some(z) = self.zones_mut().get_mut(id).and_then(Value::as_object_mut) else { return Err(format!("no zone '{id}'")) };
        z.insert("anchor".into(), json!("free"));
        z.insert("x".into(), json!(r(x)));
        z.insert("y".into(), json!(r(y)));
        Ok(())
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

    /// The focus modes as the UI shows them: name, label, minimum priority and the allow list as text (one entry per line, `field:pattern`
    /// pairs separated by ` and `; a bare pattern means the app name).
    pub fn focus_view(&self) -> Value {
        let fallback = Draft::default_focus();
        let focus = self.value.get("focus").unwrap_or(&fallback);
        let Some(modes) = focus.get("modes").and_then(Value::as_object) else { return json!([]) };
        Value::Array(
            modes
                .iter()
                .map(|(name, m)| {
                    let allow: Vec<String> = m
                        .get("allow")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_object)
                                .map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|v| format!("{k}:{v}"))).collect::<Vec<_>>().join(" and "))
                                .collect()
                        })
                        .unwrap_or_default();
                    json!({
                        "name": name,
                        "label": m.get("label").and_then(Value::as_str).unwrap_or(""),
                        "minPriority": m.get("minPriority").and_then(Value::as_str).unwrap_or("urgent"),
                        "allow": allow.join("\n"),
                    })
                })
                .collect(),
        )
    }

    /// Add a mode that lets only urgent notifications through. Returns its name.
    pub fn add_focus_mode(&mut self) -> String {
        let modes = self.focus_modes_mut();
        let mut n = modes.len() + 1;
        while modes.contains_key(&format!("mode{n}")) {
            n += 1;
        }
        let name = format!("mode{n}");
        modes.insert(name.clone(), json!({"label": format!("Focus {n}"), "minPriority": "urgent"}));
        name
    }

    pub fn delete_focus_mode(&mut self, name: &str) -> Result<(), String> {
        if self.focus_modes_mut().remove(name).is_none() {
            return Err(format!("no focus mode '{name}'"));
        }
        Ok(())
    }

    /// `field` is `label`, `minPriority` (low, normal, high, urgent) or `allow` (see [`Draft::focus_view`]).
    pub fn set_focus_field(&mut self, name: &str, field: &str, text: &str) -> Result<(), String> {
        let allow = if field == "allow" { Some(parse_allow(text)?) } else { None };
        let m = self.focus_modes_mut().get_mut(name).and_then(Value::as_object_mut).ok_or_else(|| format!("no focus mode '{name}'"))?;
        match field {
            "label" => {
                if text.trim().is_empty() { m.remove("label") } else { m.insert("label".into(), json!(text.trim())) };
            }
            "minPriority" => {
                if !["low", "normal", "high", "urgent"].contains(&text) {
                    return Err(format!("'{text}' is not a priority"));
                }
                m.insert("minPriority".into(), json!(text));
            }
            "allow" => {
                let list = allow.unwrap_or_default();
                if list.is_empty() { m.remove("allow") } else { m.insert("allow".into(), Value::Array(list)) };
            }
            _ => return Err(format!("unknown focus setting '{field}'")),
        }
        Ok(())
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

    /// What the panel's rule for this app (or for one kind of its notifications) currently does.
    pub fn app_routing(&self, field: &str, value: &str) -> AppRouting {
        self.routing_for(&[(field, value)])
    }

    /// Same, for several match pairs, e.g. the app plus `("category", "email.arrived")`.
    pub fn routing_for(&self, matches: &[(&str, &str)]) -> AppRouting {
        let Some(i) = self.app_rule_index(matches) else { return AppRouting::default() };
        let r = &self.value["rules"][i];
        let mut style = Map::new();
        for k in STYLE_KEYS {
            style.insert(k.into(), json!(outcome_text(r, k)));
        }
        AppRouting {
            exists: true,
            muted: r.get("silent").and_then(Value::as_bool).unwrap_or(false),
            zone: r.get("zone").and_then(Value::as_str).unwrap_or("").to_string(),
            timeout: r.get("timeout").map(number_text).unwrap_or_default(),
            style,
        }
    }

    /// Change one thing about how this app's notifications are handled. See [`Draft::set_routing`].
    pub fn set_app_routing(&mut self, label: &str, field: &str, value: &str, what: &str, text: &str) -> Result<(), String> {
        self.set_routing(label, &[(field, value)], what, text)
    }

    /// Change one thing about the notifications matching these (field, text) pairs. `what` is `muted` ("true"/"false"), `zone`, `timeout`,
    /// or a style key (opacity, clickThrough, noFocus, width, fontFamily, fontSize, group); empty text = not set. The rule is created on
    /// demand, named `App: <label>`; rules with more match pairs (a specific kind of notification) go before those with fewer, all before the
    /// user's own rules; a rule that no longer changes anything is removed.
    pub fn set_routing(&mut self, label: &str, matches: &[(&str, &str)], what: &str, text: &str) -> Result<(), String> {
        if matches.is_empty() || matches.iter().any(|(f, v)| v.is_empty() || !MATCH_FIELDS.contains(f)) {
            return Err("this notification cannot be matched".into());
        }
        let key = if what == "muted" { "silent" } else { what };
        let text = if what == "muted" && !["true", "false"].contains(&text) {
            return Err(format!("'{text}' is not true or false"));
        } else {
            text
        };
        if style_kind(key).is_none() {
            return Err(format!("unknown app setting '{what}'"));
        }
        let idx = match self.app_rule_index(matches) {
            Some(i) => i,
            None => {
                // validate and see whether there is anything to do before creating a rule
                let mut probe = Map::new();
                set_outcome(&mut probe, key, text)?;
                if probe.is_empty() {
                    return Ok(()); // nothing to undo
                }
                let n = matches.len();
                let rules = self.rules_mut();
                let managed = |r: &Value| r.get("name").and_then(Value::as_str).is_some_and(|n| n.starts_with(APP_RULE_PREFIX));
                let at = rules
                    .iter()
                    .position(|r| !managed(r) || r.get("match").and_then(Value::as_object).map_or(0, Map::len) < n)
                    .unwrap_or(rules.len());
                let mut m = Map::new();
                for (f, v) in matches {
                    m.insert((*f).into(), json!(format!("^{}$", regex_escape(v))));
                }
                rules.insert(at, json!({"name": format!("{APP_RULE_PREFIX}{label}"), "match": Value::Object(m)}));
                at
            }
        };
        let r = self.rule_mut(idx).ok_or_else(|| "no such rule".to_string())?;
        set_outcome(r, key, text)?;
        if !rule_has_effect(&self.value["rules"][idx]) {
            self.delete_rule(idx);
        }
        Ok(())
    }

    /// Set a look/behaviour key (opacity, clickThrough, noFocus, width, fontFamily, fontSize, group) on rule `i`. Empty text = not set.
    pub fn set_rule_style(&mut self, i: usize, key: &str, text: &str) -> Result<(), String> {
        if !STYLE_KEYS.contains(&key) {
            return Err(format!("unknown setting '{key}'"));
        }
        let r = self.rule_mut(i).ok_or_else(|| "no such rule".to_string())?;
        set_outcome(r, key, text)
    }

    /// The same keys on a zone (everything but `group`).
    pub fn set_zone_style(&mut self, id: &str, key: &str, text: &str) -> Result<(), String> {
        if !STYLE_KEYS.contains(&key) || key == "group" {
            return Err(format!("a zone has no setting '{key}'"));
        }
        let z = self.zones_mut().get_mut(id).and_then(Value::as_object_mut).ok_or_else(|| format!("no zone '{id}'"))?;
        set_outcome(z, key, text)
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

/// Parse the allow list text: one entry per line, `field:pattern` pairs joined with ` and `; a line without a field is an app-name pattern.
fn parse_allow(text: &str) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let mut entry = Map::new();
        for part in line.split(" and ").map(str::trim).filter(|p| !p.is_empty()) {
            let (field, pat) = match part.split_once(':') {
                Some((f, p)) if MATCH_FIELDS.contains(&f.trim()) => (f.trim(), p.trim()),
                _ => ("appName", part),
            };
            if pat.is_empty() {
                return Err(format!("'{line}': a pattern is empty"));
            }
            entry.insert(field.into(), json!(pat));
        }
        out.push(Value::Object(entry));
    }
    Ok(out)
}

/// The places a zone can be: a 3x3 grid (row by row) and `free`.
pub const ANCHORS: [&str; 10] = ["top-left", "top-center", "top-right", "middle-left", "center", "middle-right", "bottom-left", "bottom-center", "bottom-right", "free"];

/// Rules the panel creates for "this app" start with this, so it can find (and tidy up) its own.
pub const APP_RULE_PREFIX: &str = "App: ";

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct AppRouting {
    pub exists: bool,
    pub muted: bool,
    pub zone: String,
    pub timeout: String,
    /// Look and behaviour keys as text ("" = not set).
    pub style: Map<String, Value>,
}

/// Escape text for use inside an ECMAScript regular expression.
fn regex_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        if "\\^$.*+?()[]{}|/-".contains(c) {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

#[derive(Clone, Copy)]
enum Kind {
    Text(&'static str),
    Color(&'static str),
    Bool(bool),
    Num(f64),
}

fn default_value(k: &Kind) -> Value {
    match k {
        Kind::Text(d) | Kind::Color(d) => json!(d),
        Kind::Bool(b) => json!(b),
        Kind::Num(d) => json!(*d as i64),
    }
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
    ("theme.fontFamily", Kind::Text("")),
    ("theme.fontSize", Kind::Num(12.0)),
    ("transfer.chart", Kind::Text("line")),
];

fn lookup<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |cur, k| cur.get(k))
}

fn rule_view(index: usize, r: &Value) -> RuleView {
    let name = r.get("name").and_then(Value::as_str).unwrap_or("").to_string();
    let mut matches = Map::new();
    let mut tests: Vec<String> = Vec::new();
    for f in MATCH_FIELDS {
        let p = r.get("match").and_then(|m| m.get(f)).and_then(Value::as_str).unwrap_or("");
        matches.insert(f.into(), json!(p));
        if !p.is_empty() {
            tests.push(format!("{f} ~ {p}"));
        }
    }
    let zone = r.get("zone").and_then(Value::as_str).unwrap_or("").to_string();
    let timeout = r.get("timeout").map(number_text).unwrap_or_default();
    let silent = r.get("silent").and_then(Value::as_bool).unwrap_or(false);
    let mut does: Vec<String> = Vec::new();
    if !zone.is_empty() {
        does.push(zone.clone());
    }
    if !timeout.is_empty() {
        does.push(if timeout == "0" { "stays until dismissed".into() } else { format!("{timeout} ms") });
    }
    if silent {
        does.push("silent".into());
    }
    let when = if tests.is_empty() { "everything".to_string() } else { tests.join(" and ") };
    let summary = if does.is_empty() { format!("{when}  (no effect yet)") } else { format!("{when}  →  {}", does.join(", ")) };
    let mut style = Map::new();
    for k in STYLE_KEYS {
        style.insert(k.into(), json!(outcome_text(r, k)));
    }
    RuleView {
        index,
        title: if name.is_empty() { "(unnamed rule)".into() } else { name.clone() },
        name,
        summary,
        matches,
        zone,
        timeout,
        silent,
        style,
    }
}

// ---------------------------------------------------------------- the simulator's input

/// The notification the user describes in the simulator panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimInput {
    pub app_name: String,
    pub desktop_entry: String,
    pub origin: String,
    /// The app's category hint ("email.arrived"), or empty.
    pub category: String,
    pub summary: String,
    pub body: String,
    /// "low", "normal" or "critical".
    pub urgency: String,
    /// The app asked for no timeout (it wants the popup to stay).
    pub never_expires: bool,
    /// Ignore Do Not Disturb, mutes and full screen right now.
    pub ignore_state: bool,
}

impl Default for SimInput {
    fn default() -> Self {
        Self { app_name: String::new(), desktop_entry: String::new(), origin: String::new(), category: String::new(), summary: String::new(), body: String::new(), urgency: "normal".into(), never_expires: false, ignore_state: true }
    }
}

impl SimInput {
    /// Set one field by its UI name. Returns false for an unknown field or a bad value.
    pub fn set(&mut self, field: &str, value: &str) -> bool {
        match field {
            "appName" => self.app_name = value.into(),
            "desktopEntry" => self.desktop_entry = value.into(),
            "origin" => self.origin = value.into(),
            "category" => self.category = value.into(),
            "summary" => self.summary = value.into(),
            "body" => self.body = value.into(),
            "urgency" if ["low", "normal", "critical"].contains(&value) => self.urgency = value.into(),
            "timeout" if ["default", "never"].contains(&value) => self.never_expires = value == "never",
            "ignoreState" if ["true", "false"].contains(&value) => self.ignore_state = value == "true",
            _ => return false,
        }
        true
    }

    /// The `simulate` request for the daemon, evaluating against `config` (the draft).
    pub fn to_request(&self, config: &Value) -> Value {
        json!({
            "config": config,
            "ignoreState": self.ignore_state,
            "notification": {
                "appName": self.app_name, "desktopEntry": self.desktop_entry, "origin": self.origin, "category": self.category,
                "summary": self.summary, "body": self.body, "urgency": self.urgency,
                "expireTimeout": if self.never_expires { 0 } else { -1 }
            }
        })
    }
}

// ---------------------------------------------------------------- the file

/// Where rules.json lives: `$NOTIFD_RULES` if set (tests), otherwise `$XDG_CONFIG_HOME/notifd/rules.json`.
pub fn rules_path() -> PathBuf {
    if let Some(p) = std::env::var_os("NOTIFD_RULES").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let cfg = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into())).join(".config")
    });
    cfg.join("notifd/rules.json")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveOutcome {
    Saved,
    /// The file on disk differs from what the draft was started from (someone edited it); nothing was written.
    Conflict { disk: String },
    Failed(String),
}

/// Write `text` to `path`, atomically, unless the file changed since `expected_base` (then `Conflict`, unless `force`).
///
/// - Follows a symlink: the real file is replaced and the link is kept (the installed rules.json is reached through one).
/// - Keeps the previous contents next to it as `<file>.bak`.
pub fn save(path: &Path, text: &str, expected_base: &str, force: bool) -> SaveOutcome {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let disk = fs::read_to_string(&target).ok();
    if !force {
        if let Some(d) = &disk {
            if d != expected_base {
                return SaveOutcome::Conflict { disk: d.clone() };
            }
        }
    }
    let result = (|| -> std::io::Result<()> {
        let dir = target.parent().ok_or_else(|| std::io::Error::other("no parent directory"))?;
        fs::create_dir_all(dir)?;
        let mode = fs::metadata(&target).map(|m| m.permissions().mode() & 0o777).unwrap_or(0o644);
        if let Some(d) = &disk {
            let mut bak = target.clone().into_os_string();
            bak.push(".bak");
            fs::write(&bak, d)?;
        }
        let tmp = dir.join(format!(".{}.{}.tmp", target.file_name().and_then(|n| n.to_str()).unwrap_or("rules"), std::process::id()));
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
        }
        fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))?;
        if let Err(e) = fs::rename(&tmp, &target) {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
        Ok(())
    })();
    match result {
        Ok(()) => SaveOutcome::Saved,
        Err(e) => SaveOutcome::Failed(format!("could not write {}: {e}", target.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    const SAMPLE: &str = r#"{
  "defaultZone": "personal",
  "futureSetting": { "keep": "me" },
  "zones": { "business": {"anchor": "top-left"}, "personal": {"anchor": "top-right"} },
  "rules": [
    { "name": "Business sites", "match": { "origin": "example-shop|example-chat" }, "zone": "business", "extra": 1 },
    { "name": "Critical", "match": { "urgency": "critical" }, "timeout": 0 },
    { "match": { "appName": "^kernel$" }, "silent": true }
  ]
}
"#;

    fn tmpdir(label: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("notify-draft-{label}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn settings_show_defaults_and_set_by_path() {
        let mut d = Draft::from_raw(SAMPLE).unwrap();
        let v = d.settings_view();
        assert_eq!(v["values"]["theme.card"], "#1d1f27", "a missing key shows the daemon's default");
        assert_eq!(v["values"]["maxVisible"], 5);
        assert_eq!(v["zones"].as_array().unwrap().len(), 2);
        assert_eq!(v["zones"][0]["rules"], 1, "business is used by one rule");
        d.set_setting("theme.card", "#000000").unwrap();
        d.set_setting("maxVisible", "3").unwrap();
        d.set_setting("sound.enabled", "false").unwrap();
        assert_eq!(d.value()["theme"]["card"], "#000000");
        assert_eq!(d.value()["maxVisible"], 3);
        assert_eq!(d.value()["sound"]["enabled"], false);
        assert_eq!(d.value()["futureSetting"]["keep"], "me", "unknown keys survive");
        d.set_setting("theme.card", "").unwrap();
        assert!(d.value()["theme"].get("card").is_none(), "empty text removes the key");
        assert!(d.set_setting("maxVisible", "lots").is_err());
        assert!(d.set_setting("maxVisible", "-1").is_err());
        assert!(d.set_setting("sound.enabled", "maybe").is_err());
        assert!(d.set_setting("nonsense", "1").is_err());
    }

    #[test]
    fn zones_can_be_added_edited_and_deleted_safely() {
        let mut d = Draft::from_raw(SAMPLE).unwrap();
        let id = d.add_zone();
        assert_eq!(id, "zone3");
        d.set_zone_field(&id, "anchor", "center").unwrap();
        assert!(d.set_zone_field(&id, "anchor", "somewhere").is_err());
        d.set_zone_position(&id, 0.25, 1.7).unwrap();
        assert_eq!((d.value()["zones"][&id]["anchor"].as_str(), d.value()["zones"][&id]["x"].as_f64(), d.value()["zones"][&id]["y"].as_f64()), (Some("free"), Some(0.25), Some(1.0)), "clamped to the screen");
        d.set_zone_field(&id, "anchor", "top-center").unwrap();
        assert!(d.value()["zones"][&id].get("x").is_none(), "leaving free drops the position");
        d.set_zone_field(&id, "anchor", "free").unwrap();
        assert_eq!(d.value()["zones"][&id]["x"], 0.5, "switching to free starts in the middle");
        d.set_zone_field(&id, "anchor", "bottom-left").unwrap();
        let anchor = d.value()["zones"][&id]["anchor"].as_str().unwrap().to_string();
        assert!(["bottom-right", "bottom-left"].contains(&anchor.as_str()), "a free corner is chosen: {anchor}");
        d.set_zone_field(&id, "label", "Games").unwrap();
        d.set_zone_field(&id, "anchor", "top-left").unwrap();
        assert_eq!(d.value()["zones"][&id]["label"], "Games");
        assert!(d.set_zone_field(&id, "colour", "x").is_err());
        assert!(d.set_zone_field("nope", "label", "x").is_err());
        d.set_zone_field(&id, "label", "").unwrap();
        assert!(d.value()["zones"][&id].get("label").is_none());
        assert!(d.delete_zone("personal").is_err(), "the default zone cannot be deleted");
        assert_eq!(d.delete_zone("business").unwrap(), 1, "one rule loses its zone");
        assert!(d.rules()[0].zone.is_empty());
        assert!(d.delete_zone(&id).is_ok());
        assert!(d.delete_zone("personal").is_err(), "the last zone cannot be deleted");
    }

    #[test]
    fn per_app_rules_are_created_updated_and_tidied_away() {
        let mut d = Draft::from_raw(SAMPLE).unwrap();
        assert!(!d.app_routing("appName", "Steam").exists);
        d.set_app_routing("Steam", "appName", "Steam", "muted", "true").unwrap();
        assert_eq!(d.rule_count(), 4);
        assert_eq!(d.value()["rules"][0]["name"], "App: Steam", "the app's rule goes first so it wins");
        assert_eq!(d.value()["rules"][0]["match"]["appName"], "^Steam$");
        d.set_app_routing("Steam", "appName", "Steam", "zone", "business").unwrap();
        d.set_app_routing("Telegram", "desktopEntry", "org.telegram.desktop", "timeout", "3000").unwrap();
        assert_eq!(d.rule_count(), 5);
        assert_eq!(d.value()["rules"][1]["name"], "App: Telegram", "managed rules stay together, before the user's own");
        assert_eq!(d.value()["rules"][1]["match"]["desktopEntry"], "^org\\.telegram\\.desktop$", "regex characters are escaped");
        let r = d.app_routing("appName", "Steam");
        assert!(r.exists && r.muted && r.zone == "business", "{r:?}");
        d.set_app_routing("Steam", "appName", "Steam", "muted", "false").unwrap();
        d.set_app_routing("Steam", "appName", "Steam", "zone", "").unwrap();
        assert!(!d.app_routing("appName", "Steam").exists, "a rule that does nothing is removed");
        assert_eq!(d.rule_count(), 4);
        d.set_app_routing("Nobody", "appName", "Nobody", "muted", "false").unwrap();
        assert_eq!(d.rule_count(), 4, "turning off something that was never on adds nothing");
        assert!(d.set_app_routing("X", "nonsense", "x", "muted", "true").is_err());
        assert!(d.set_app_routing("X", "appName", "x", "muted", "maybe").is_err());
        assert!(d.set_app_routing("X", "appName", "x", "timeout", "soon").is_err());
        assert!(d.set_app_routing("X", "appName", "x", "colour", "red").is_err());
        assert_eq!(d.value()["rules"][1]["name"], "Business sites", "the user's own rules are untouched");
    }

    #[test]
    fn style_keys_are_set_on_rules_zones_apps_and_kinds_of_notification() {
        let mut d = Draft::from_raw(SAMPLE).unwrap();
        d.set_rule_style(0, "opacity", "0.6").unwrap();
        d.set_rule_style(0, "clickThrough", "true").unwrap();
        d.set_rule_style(0, "noFocus", "false").unwrap();
        d.set_rule_style(0, "width", "480").unwrap();
        d.set_rule_style(0, "fontFamily", "Serif").unwrap();
        d.set_rule_style(0, "group", "Sites").unwrap();
        let r = &d.value()["rules"][0];
        assert_eq!((r["opacity"].as_f64(), r["clickThrough"].as_bool(), r["noFocus"].as_bool(), r["width"].as_i64()), (Some(0.6), Some(true), Some(false), Some(480)));
        assert_eq!(d.rules()[0].style["clickThrough"], "true");
        assert_eq!(d.rules()[0].style["noFocus"], "false", "an explicit false is kept: it overrides a later rule");
        d.set_rule_style(0, "clickThrough", "").unwrap();
        assert!(d.value()["rules"][0].get("clickThrough").is_none(), "empty text removes the key");
        assert!(d.set_rule_style(0, "opacity", "clear").is_err());
        assert!(d.set_rule_style(0, "flavour", "x").is_err());
        assert!(d.set_rule_style(0, "noFocus", "perhaps").is_err());

        d.set_zone_style("personal", "opacity", "0.5").unwrap();
        assert_eq!(d.value()["zones"]["personal"]["opacity"], 0.5);
        assert!(d.set_zone_style("personal", "group", "x").is_err(), "a zone has no group");
        assert!(d.set_zone_style("ghost", "opacity", "0.5").is_err());
        assert_eq!(d.settings_view()["zones"][1]["style"]["opacity"], "0.5");
        d.set_setting("theme.opacity", "0.9").unwrap();
        d.set_setting("theme.fontFamily", "Noto Sans").unwrap();
        d.set_setting("theme.fontSize", "14").unwrap();
        assert_eq!((d.value()["theme"]["opacity"].as_f64(), d.value()["theme"]["fontSize"].as_i64()), (Some(0.9), Some(14)));

        // per app, and per kind of notification of an app: the kind's rule goes first
        d.set_routing("Mailer", &[("appName", "Mailer")], "clickThrough", "true").unwrap();
        d.set_routing("Mailer mail", &[("appName", "Mailer"), ("category", "email.arrived")], "group", "Mail").unwrap();
        d.set_routing("Mailer mail", &[("appName", "Mailer"), ("category", "email.arrived")], "zone", "business").unwrap();
        let names: Vec<&str> = d.value()["rules"].as_array().unwrap().iter().filter_map(|r| r["name"].as_str()).collect();
        assert_eq!(&names[..2], &["App: Mailer mail", "App: Mailer"], "more specific first, before the user's rules: {names:?}");
        assert_eq!(d.routing_for(&[("appName", "Mailer"), ("category", "email.arrived")]).style["group"], "Mail");
        assert_eq!(d.routing_for(&[("appName", "Mailer")]).style["clickThrough"], "true");
        d.set_routing("Mailer mail", &[("appName", "Mailer"), ("category", "email.arrived")], "group", "").unwrap();
        d.set_routing("Mailer mail", &[("appName", "Mailer"), ("category", "email.arrived")], "zone", "").unwrap();
        assert!(!d.routing_for(&[("appName", "Mailer"), ("category", "email.arrived")]).exists, "an emptied rule is removed");
        assert!(d.routing_for(&[("appName", "Mailer")]).exists, "the app's own rule is untouched");
    }

    #[test]
    fn focus_modes_are_edited_as_text() {
        let mut d = Draft::from_raw("{\"zones\": {\"a\": {\"anchor\": \"top-left\"}}}").unwrap();
        assert_eq!(d.focus_view()[0]["name"], "urgent", "the built-in mode is shown when the file has no focus section");
        let n = d.add_focus_mode();
        assert_eq!(n, "mode2", "the built-in mode counts, and is written out with the first edit");
        assert!(d.value()["focus"]["modes"].get("urgent").is_some());
        d.set_focus_field(&n, "label", "Work").unwrap();
        d.set_focus_field(&n, "minPriority", "high").unwrap();
        d.set_focus_field(&n, "allow", "^Signal$\norigin:calendar and category:^email\n\n").unwrap();
        let m = &d.value()["focus"]["modes"][&n];
        assert_eq!((m["label"].as_str(), m["minPriority"].as_str()), (Some("Work"), Some("high")));
        assert_eq!(m["allow"][0]["appName"], "^Signal$", "a bare pattern means the app name");
        assert_eq!((m["allow"][1]["origin"].as_str(), m["allow"][1]["category"].as_str()), (Some("calendar"), Some("^email")));
        let v = d.focus_view();
        assert_eq!(v[1]["allow"], "appName:^Signal$\norigin:calendar and category:^email", "round-trips as text");
        assert!(d.set_focus_field(&n, "minPriority", "mega").is_err());
        assert!(d.set_focus_field(&n, "allow", "appName:").is_err(), "an empty pattern is refused");
        assert!(d.set_focus_field("ghost", "label", "x").is_err());
        assert!(d.set_focus_field(&n, "colour", "x").is_err());
        d.set_focus_field(&n, "allow", "").unwrap();
        assert!(d.value()["focus"]["modes"][&n].get("allow").is_none(), "an empty list is removed");
        d.delete_focus_mode(&n).unwrap();
        assert!(d.delete_focus_mode(&n).is_err());
    }

    #[test]
    fn rejects_text_that_is_not_an_object() {
        assert!(Draft::from_raw("{nope").is_err());
        assert!(Draft::from_raw("[1,2]").unwrap_err().contains("object"));
    }

    #[test]
    fn views_summarise_each_rule() {
        let d = Draft::from_raw(SAMPLE).unwrap();
        let v = d.rules();
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].title, "Business sites");
        assert_eq!(v[0].summary, "origin ~ example-shop|example-chat  →  business");
        assert_eq!(v[0].matches["origin"], "example-shop|example-chat");
        assert_eq!(v[0].matches["appName"], "");
        assert_eq!(v[1].timeout, "0");
        assert!(v[1].summary.contains("stays until dismissed"));
        assert_eq!(v[2].title, "(unnamed rule)");
        assert!(v[2].silent && v[2].summary.ends_with("silent"));
        assert_eq!(d.zone_names(), ["business", "personal"]);
    }

    #[test]
    fn editing_changes_only_what_was_asked() {
        let mut d = Draft::from_raw(SAMPLE).unwrap();
        assert!(!d.is_dirty());
        d.set_match(0, "appName", "^steam$");
        d.set_match(0, "origin", "");
        d.set_zone(0, Some("personal"));
        d.set_timeout(0, Some(1500.0));
        d.set_name(0, "Steam");
        assert!(d.is_dirty());
        let r = &d.rules()[0];
        assert_eq!((r.name.as_str(), r.zone.as_str(), r.timeout.as_str()), ("Steam", "personal", "1500"));
        assert_eq!(r.matches["appName"], "^steam$");
        assert_eq!(r.matches["origin"], "");
        // unknown keys survive, in the rule and at the top level
        assert_eq!(d.value()["rules"][0]["extra"], 1);
        assert_eq!(d.value()["futureSetting"]["keep"], "me");
        // whole-number timeouts are written as integers
        assert!(d.to_text().contains("\"timeout\": 1500,") || d.to_text().contains("\"timeout\": 1500\n"), "{}", d.to_text());
        assert!(!d.to_text().contains("1500.0"));
        d.set_zone(0, None);
        d.set_timeout(0, None);
        d.set_silent(0, true);
        let r = &d.rules()[0];
        assert!(r.zone.is_empty() && r.timeout.is_empty() && r.silent);
        d.set_silent(0, false);
        assert!(!d.rules()[0].silent);
    }

    #[test]
    fn add_delete_move_duplicate_keep_positions_straight() {
        let mut d = Draft::from_raw(SAMPLE).unwrap();
        let n = d.add_rule();
        assert_eq!((n, d.rule_count()), (3, 4));
        assert_eq!(d.rules()[3].summary, "everything  (no effect yet)");
        assert_eq!(d.move_rule(3, -1), 2);
        assert_eq!(d.rules()[2].title, "New rule");
        assert_eq!(d.move_rule(0, -1), 0, "moving the first rule up does nothing");
        assert_eq!(d.move_rule(3, 1), 3, "moving the last rule down does nothing");
        assert_eq!(d.duplicate_rule(0), Some(1));
        assert_eq!(d.rules()[1].title, "Business sites (copy)");
        d.delete_rule(1);
        assert_eq!(d.rule_count(), 4);
        d.delete_rule(99);
        assert_eq!(d.rule_count(), 4);
    }

    #[test]
    fn revert_and_mark_saved() {
        let mut d = Draft::from_raw(SAMPLE).unwrap();
        d.add_rule();
        assert!(d.is_dirty());
        d.revert();
        assert!(!d.is_dirty() && d.rule_count() == 3);
        d.add_rule();
        let text = d.to_text();
        d.mark_saved(&text);
        assert!(!d.is_dirty() && d.base_raw() == text && d.rule_count() == 4);
    }

    #[test]
    fn maps_daemon_errors_to_a_rule() {
        let d = Draft::from_raw(SAMPLE).unwrap();
        assert_eq!(d.error_index("rule 'Critical': unknown match field 'x'"), Some(1));
        assert_eq!(d.error_index("rule #3: bad regex for appName"), Some(2));
        assert_eq!(d.error_index("rule #9: whatever"), None);
        assert_eq!(d.error_index("defaultZone 'x' is not a defined zone"), None);
        assert_eq!(d.error_index("rule 'Nope': bad"), None);
    }

    #[test]
    fn saving_detects_hand_edits_and_can_be_forced() {
        let dir = tmpdir("save");
        let p = dir.join("rules.json");
        fs::write(&p, SAMPLE).unwrap();
        let mut d = Draft::from_raw(SAMPLE).unwrap();
        d.add_rule();
        // nobody touched the file: saved, and the previous contents are kept
        assert_eq!(save(&p, &d.to_text(), d.base_raw(), false), SaveOutcome::Saved);
        assert_eq!(fs::read_to_string(&p).unwrap(), d.to_text());
        assert_eq!(fs::read_to_string(dir.join("rules.json.bak")).unwrap(), SAMPLE);
        // someone edits the file by hand; saving from the OLD base must not clobber it
        fs::write(&p, "{\"rules\": []}\n").unwrap();
        d.add_rule();
        match save(&p, &d.to_text(), SAMPLE, false) {
            SaveOutcome::Conflict { disk } => assert_eq!(disk, "{\"rules\": []}\n"),
            other => panic!("expected a conflict, got {other:?}"),
        }
        assert_eq!(fs::read_to_string(&p).unwrap(), "{\"rules\": []}\n", "a conflict writes nothing");
        // forcing overwrites, and keeps the hand-edited version as the backup
        assert_eq!(save(&p, &d.to_text(), SAMPLE, true), SaveOutcome::Saved);
        assert_eq!(fs::read_to_string(dir.join("rules.json.bak")).unwrap(), "{\"rules\": []}\n");
    }

    #[test]
    fn saving_through_a_symlink_keeps_the_link() {
        let dir = tmpdir("link");
        let real = dir.join("config-rules.json");
        let link = dir.join("payload-rules.json");
        fs::write(&real, SAMPLE).unwrap();
        symlink(&real, &link).unwrap();
        let mut d = Draft::from_raw(SAMPLE).unwrap();
        d.add_rule();
        assert_eq!(save(&link, &d.to_text(), SAMPLE, false), SaveOutcome::Saved);
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "the symlink must survive");
        assert_eq!(fs::read_to_string(&real).unwrap(), d.to_text(), "the real file is what changed");
    }

    #[test]
    fn saving_a_file_that_does_not_exist_yet_works() {
        let dir = tmpdir("new");
        let p = dir.join("sub/rules.json");
        assert_eq!(save(&p, "{}\n", "", false), SaveOutcome::Saved);
        assert_eq!(fs::read_to_string(&p).unwrap(), "{}\n");
    }

    #[test]
    fn the_simulator_input_builds_the_daemons_request() {
        let mut i = SimInput::default();
        assert!(i.set("appName", "Brave Origin") && i.set("origin", "example-shop.com") && i.set("urgency", "critical"));
        assert!(i.set("timeout", "never") && i.set("ignoreState", "false"));
        assert!(!i.set("urgency", "loud") && !i.set("nonsense", "x") && !i.set("timeout", "soon"), "bad fields and values are refused");
        let draft = json!({"rules": []});
        let r = i.to_request(&draft);
        assert_eq!(r["notification"]["appName"], "Brave Origin");
        assert_eq!(r["notification"]["urgency"], "critical");
        assert_eq!(r["notification"]["expireTimeout"], 0);
        assert_eq!(r["ignoreState"], false);
        assert_eq!(r["config"], draft, "the draft is what gets evaluated");
        assert_eq!(SimInput::default().to_request(&draft)["notification"]["expireTimeout"], -1);
    }

    #[test]
    fn the_path_can_be_overridden_for_tests() {
        // not set in the normal environment of this test run; only check the default shape
        if std::env::var_os("NOTIFD_RULES").is_none() {
            assert!(rules_path().ends_with("notifd/rules.json"));
        }
    }
}
