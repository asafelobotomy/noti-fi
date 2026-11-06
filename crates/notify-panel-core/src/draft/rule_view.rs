//! One rule as the editor shows it.

use super::*;

/// One rule as the UI shows it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RuleView {
    pub index: usize,
    /// The name, or "(unnamed rule)".
    pub title: String,
    pub name: String,
    /// "website is example-shop or example-chat  →  business"; a real regex is shown as "origin ~ pattern"
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

/// The words a pattern stands for when it is a plain list (written the way the editor's tags write it), or None when it is a real regex.
fn plain_words(field: &str, p: &str) -> Option<Vec<String>> {
    const META: &str = ".^$*+?()[]{}|\\";
    let inner = match field {
        "appName" | "desktopEntry" => p.strip_prefix("^(").and_then(|x| x.strip_suffix(")$")).or_else(|| p.strip_prefix('^').and_then(|x| x.strip_suffix('$'))),
        // a closed set of values: a bare word means the same as the anchored one
        "urgency" | "channel" => p.strip_prefix("^(").and_then(|x| x.strip_suffix(")$")).or_else(|| p.strip_prefix('^').and_then(|x| x.strip_suffix('$'))).or(Some(p)),
        "origin" => p.strip_prefix("\\b(").and_then(|x| x.strip_suffix(")\\b")).or_else(|| p.strip_prefix("\\b").and_then(|x| x.strip_suffix("\\b"))),
        "category" => p.strip_prefix("^(").and_then(|x| x.strip_suffix(')')).or_else(|| p.strip_prefix('^')),
        _ => Some(p),
    }?;
    let (mut out, mut cur, mut chars) = (Vec::new(), String::new(), inner.chars());
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(n) if META.contains(n) => cur.push(n),
                _ => return None,
            },
            '|' if !cur.is_empty() => out.push(std::mem::take(&mut cur)),
            c if META.contains(c) => return None,
            c => cur.push(c),
        }
    }
    if cur.is_empty() {
        return None;
    }
    out.push(cur);
    Some(out)
}

/// One condition in plain words: "website is a or b", "title has “x”"; a real regex stays as "field ~ pattern".
fn condition_text(field: &str, p: &str) -> String {
    let Some(words) = plain_words(field, p) else { return format!("{field} ~ {p}") };
    let (what, has) = match field {
        "appName" => ("app", false),
        "desktopEntry" => ("desktop entry", false),
        "origin" => ("website", false),
        "summary" => ("title", true),
        "body" => ("text", true),
        "urgency" => ("urgency", false),
        "category" => ("kind", false),
        _ => ("arrives as", false),
    };
    if has {
        format!("{what} has {}", words.iter().map(|w| format!("“{w}”")).collect::<Vec<_>>().join(" or "))
    } else if field == "category" {
        format!("{what} starts with {}", words.join(" or "))
    } else {
        format!("{what} is {}", words.join(" or "))
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
            tests.push(condition_text(f, p));
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
