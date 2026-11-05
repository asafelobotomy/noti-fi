//! Per-app routing edits (the Apps page) and the rules they create.

use super::*;

impl Draft {
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
}

/// Escape text for use inside an ECMAScript regular expression.
pub(super) fn regex_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        if "\\^$.*+?()[]{}|/-".contains(c) {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct AppRouting {
    pub exists: bool,
    pub muted: bool,
    pub zone: String,
    pub timeout: String,
    /// Look and behaviour keys as text ("" = not set).
    pub style: Map<String, Value>,
}
