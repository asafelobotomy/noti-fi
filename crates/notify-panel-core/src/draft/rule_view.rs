//! One rule as the editor shows it.

use super::*;

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

pub(super) fn number_text(v: &Value) -> String {
    match v.as_f64() {
        Some(n) if n.fract() == 0.0 => format!("{}", n as i64),
        Some(n) => n.to_string(),
        None => String::new(),
    }
}

pub(super) fn rule_view(index: usize, r: &Value) -> RuleView {
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
