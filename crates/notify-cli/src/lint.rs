//! Static checks for rules.json. The running daemon is the authority; this catches mistakes without one.
//! Regexes are checked with a real ECMAScript engine, so what passes here is what the daemon accepts.

use serde_json::Value;

use crate::payload;

/// What a rule can test; `group` is the group (zone) a notification is in, decided by the rules that do not test it.
pub const MATCH_FIELDS: [&str; 9] = ["appName", "desktopEntry", "origin", "summary", "body", "urgency", "category", "channel", "group"];
/// Patterns run on text that apps control, so the daemon refuses long ones (see `maxPattern` in shell.qml).
pub const MAX_PATTERN: usize = 500;
pub const PRIORITIES: [&str; 4] = ["low", "normal", "high", "urgent"];
pub const CHARTS: [&str; 3] = ["bars", "line", "heatmap"];
pub const ANCHORS: [&str; 10] = ["top-left", "top-center", "top-right", "middle-left", "center", "middle-right", "bottom-left", "bottom-center", "bottom-right", "free"];

/// Why a regex does not work in the daemon (None = fine). Adds a hint when it looks like Python/PCRE syntax.
pub fn regex_problem(pat: &str) -> Option<String> {
    if pat.chars().count() > MAX_PATTERN {
        return Some(format!("the pattern is longer than {MAX_PATTERN} characters"));
    }
    let err = match regress::Regex::with_flags(pat, "i") {
        Ok(_) => return None,
        Err(e) => e.to_string(),
    };
    let hint = if pat.contains("(?P<") || pat.contains("(?P=") {
        Some("named groups are written (?<name>...) in JavaScript, not (?P<name>...)")
    } else if has_inline_flags(pat) {
        Some("inline flags like (?i) are not supported (matching is already case-insensitive)")
    } else if pat.contains("(?#") {
        Some("(?#...) comments are not supported")
    } else if pat.contains("\\A") || pat.contains("\\Z") {
        Some("\\A and \\Z are not supported; use ^ and $")
    } else {
        None
    };
    Some(match hint {
        Some(h) => h.to_string(),
        None => err,
    })
}

fn has_inline_flags(p: &str) -> bool {
    let b = p.as_bytes();
    let mut i = 0;
    while i + 2 < b.len() {
        if b[i] == b'(' && b[i + 1] == b'?' {
            let mut j = i + 2;
            while j < b.len() && b"aiLmsux".contains(&b[j]) {
                j += 1;
            }
            if j > i + 2 && j < b.len() && (b[j] == b':' || b[j] == b')') {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_hex_colour(s: &str) -> bool {
    let h = match s.strip_prefix('#') {
        Some(h) => h,
        None => return false,
    };
    (3..=8).contains(&h.len()) && h.bytes().all(|b| b.is_ascii_hexdigit())
}

fn non_negative_number(v: &Value) -> bool {
    v.as_f64().map(|n| n >= 0.0).unwrap_or(false)
}

/// Problems with the look-and-behaviour keys of a rule, zone or theme (the same checks the daemon makes).
fn style_problems(o: &serde_json::Map<String, Value>, allow_group: bool) -> Vec<String> {
    let mut e = Vec::new();
    let num = |k: &str, lo: f64, hi: f64| o.get(k).map(|v| v.as_f64().is_some_and(|n| n >= lo && n <= hi));
    if num("opacity", 0.1, 1.0) == Some(false) {
        e.push("opacity must be a number from 0.1 to 1".to_string());
    }
    if num("width", 120.0, 4000.0) == Some(false) {
        e.push("width must be a number from 120 to 4000".to_string());
    }
    if num("fontSize", 6.0, 48.0) == Some(false) {
        e.push("fontSize must be a number from 6 to 48".to_string());
    }
    if o.get("timerStyle").is_some_and(|v| !v.as_str().is_some_and(|s| s.split(',').all(|t| ["none", "line", "circle", "frame"].contains(&t.trim())))) {
        e.push("timerStyle must be none, or any of line, circle, frame separated by commas".to_string());
    }
    for k in ["clickThrough", "noFocus"] {
        if o.get(k).is_some_and(|v| !v.is_boolean()) {
            e.push(format!("{k} must be true or false"));
        }
    }
    if o.get("fontFamily").is_some_and(|v| !v.as_str().is_some_and(|s| s.len() <= 100)) {
        e.push("fontFamily must be a font name".to_string());
    }
    if o.get("priority").is_some_and(|v| !v.as_str().is_some_and(|p| PRIORITIES.contains(&p))) {
        e.push(format!("priority must be {}", PRIORITIES.join(", ")));
    }
    for k in ["media", "stack"] {
        if o.get(k).is_some_and(|v| !v.is_boolean()) {
            e.push(format!("{k} must be true or false"));
        }
    }
    if let Some(g) = o.get("group") {
        if !allow_group {
            e.push("group can only be set on a rule".to_string());
        } else if !g.as_str().is_some_and(|s| s.len() <= 60) {
            e.push("group must be a short name".to_string());
        }
    }
    e
}

/// Problems with the `focus` section (the same checks the daemon makes).
fn focus_problems(f: &Value) -> Vec<String> {
    let mut e = Vec::new();
    let Some(modes) = f.get("modes").and_then(Value::as_object) else { return vec!["modes must be an object".into()] };
    for (name, m) in modes {
        if name.is_empty() || name.len() > 40 || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            e.push(format!("mode '{name}': the name may only use letters, digits, - and _"));
        }
        let Some(mo) = m.as_object() else {
            e.push(format!("mode '{name}' must be an object"));
            continue;
        };
        if mo.get("minPriority").is_some_and(|v| !v.as_str().is_some_and(|p| PRIORITIES.contains(&p))) {
            e.push(format!("mode '{name}': minPriority must be {}", PRIORITIES.join(", ")));
        }
        if let Some(allow) = mo.get("allow") {
            let Some(list) = allow.as_array() else {
                e.push(format!("mode '{name}': allow must be a list"));
                continue;
            };
            for a in list {
                let Some(ao) = a.as_object() else {
                    e.push(format!("mode '{name}': each allow entry must be an object of patterns"));
                    continue;
                };
                for (k, pat) in ao {
                    if !MATCH_FIELDS.contains(&k.as_str()) {
                        e.push(format!("mode '{name}': unknown match field '{k}'"));
                    } else if let Some(why) = pat.as_str().and_then(regex_problem) {
                        e.push(format!("mode '{name}': bad regex for {k}: {why}"));
                    }
                }
            }
        }
    }
    e
}

/// Mistakes the daemon would reject (the same rules it enforces when it loads the file).
pub fn lint_config(cfg: &Value) -> Vec<String> {
    let mut errs = Vec::new();
    let Some(obj) = cfg.as_object() else { return vec!["top level must be a JSON object".into()] };
    let zones = obj.get("zones").and_then(Value::as_object);
    match zones {
        Some(z) if !z.is_empty() => {}
        _ => errs.push("zones: need at least one zone".into()),
    }
    let empty = serde_json::Map::new();
    let zones = zones.unwrap_or(&empty);
    for (z, zz) in zones {
        match zz.get("anchor").and_then(Value::as_str) {
            Some(a) if ANCHORS.contains(&a) => {
                if let Some(zo) = zz.as_object() {
                    errs.extend(style_problems(zo, false).into_iter().map(|p| format!("zone '{z}': {p}")));
                }
                if a == "free" {
                    for k in ["x", "y"] {
                        if !zz.get(k).and_then(Value::as_f64).is_some_and(|n| (0.0..=1.0).contains(&n)) {
                            errs.push(format!("zone '{z}': a free zone needs {k} between 0 and 1"));
                        }
                    }
                }
                if let Some(acc) = zz.get("accent") {
                    if !acc.as_str().map(is_hex_colour).unwrap_or(false) {
                        errs.push(format!("zone '{z}': accent must be a #hex colour"));
                    }
                }
            }
            _ => errs.push(format!("zone '{z}': anchor must be one of {}", ANCHORS.join(", "))),
        }
    }
    let dz = obj.get("defaultZone").and_then(Value::as_str).unwrap_or("personal");
    if !zones.contains_key(dz) {
        errs.push(format!("defaultZone '{dz}' is not a defined zone"));
    }
    for k in ["defaultTimeout", "width", "margin", "spacing", "maxVisible"] {
        if let Some(v) = obj.get(k) {
            if !non_negative_number(v) {
                errs.push(format!("{k} must be a number >= 0"));
            }
        }
    }
    if let Some(th) = obj.get("theme").and_then(Value::as_object) {
        errs.extend(style_problems(th, false).into_iter().map(|p| format!("theme: {p}")));
    }
    if let Some(f) = obj.get("focus") {
        errs.extend(focus_problems(f).into_iter().map(|p| format!("focus: {p}")));
    }
    if let Some(t) = obj.get("transfer") {
        if !t.get("chart").is_none_or(|c| c.as_str().is_some_and(|c| CHARTS.contains(&c))) {
            errs.push(format!("transfer.chart must be {}", CHARTS.join(", ")));
        }
    }
    let ad = obj.get("autoDnd");
    if let Some(a) = ad {
        let ok = a.as_object().map(|m| ["fullscreen", "screenShare"].iter().all(|k| m.get(*k).map(Value::is_boolean).unwrap_or(true))).unwrap_or(false);
        if !ok {
            errs.push("autoDnd.fullscreen and autoDnd.screenShare must be true or false".into());
        }
    }
    let rules = match obj.get("rules") {
        None => return errs,
        Some(Value::Array(r)) => r,
        Some(_) => {
            errs.push("rules must be a list".into());
            return errs;
        }
    };
    for (i, r) in rules.iter().enumerate() {
        let Some(ro) = r.as_object() else {
            errs.push(format!("rule #{}: must be an object", i + 1));
            continue;
        };
        let name = ro.get("name").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("#{}", i + 1));
        if let Some(z) = ro.get("zone") {
            // the System group always exists (notifd adds it when the file leaves it out)
            if !z.as_str().map(|s| s == "system" || zones.contains_key(s)).unwrap_or(false) {
                errs.push(format!("rule '{name}': unknown zone '{}'", z.as_str().unwrap_or("?")));
            }
        }
        errs.extend(style_problems(ro, true).into_iter().map(|p| format!("rule '{name}': {p}")));
        if let Some(t) = ro.get("timeout") {
            if !non_negative_number(t) {
                errs.push(format!("rule '{name}': timeout must be a number >= 0"));
            }
        }
        let Some(m) = ro.get("match") else { continue };
        let Some(m) = m.as_object() else {
            errs.push(format!("rule '{name}': match must be an object"));
            continue;
        };
        if m.contains_key("group") && ro.contains_key("zone") {
            errs.push(format!("rule '{name}': a rule for a group cannot move it to another group"));
        }
        for (k, pat) in m {
            if !MATCH_FIELDS.contains(&k.as_str()) {
                errs.push(format!("rule '{name}': unknown match field '{k}' (use {})", MATCH_FIELDS.join(", ")));
                continue;
            }
            match pat.as_str() {
                None => errs.push(format!("rule '{name}': the pattern for {k} must be text")),
                Some(p) => {
                    if let Some(why) = regex_problem(p) {
                        errs.push(format!("rule '{name}': bad regex for {k}: {why}"));
                    }
                }
            }
        }
    }
    errs
}

/// Keys the daemon silently ignores (almost always typos), found by walking the JSON Schema.
pub fn lint_warnings(cfg: &Value) -> Vec<String> {
    let Ok(schema) = serde_json::from_str::<Value>(payload::SCHEMA) else { return Vec::new() };
    let mut out = Vec::new();
    walk(cfg, &schema, "", &mut out);
    out
}

fn walk(val: &Value, sch: &Value, path: &str, out: &mut Vec<String>) {
    let join = |k: &str| if path.is_empty() { k.to_string() } else { format!("{path}.{k}") };
    match val {
        Value::Object(map) => {
            let props = sch.get("properties").and_then(Value::as_object);
            let extra = sch.get("additionalProperties");
            for (k, v) in map {
                if let Some(ps) = props.and_then(|p| p.get(k)) {
                    walk(v, ps, &join(k), out);
                } else if extra == Some(&Value::Bool(false)) && k != "name" {
                    out.push(format!("unknown setting '{}' (the daemon ignores it; typo?)", join(k)));
                } else if let Some(es) = extra.filter(|e| e.is_object()) {
                    walk(v, es, &join(k), out);
                }
            }
        }
        Value::Array(items) => {
            if let Some(is) = sch.get("items").filter(|i| i.is_object()) {
                for (i, it) in items.iter().enumerate() {
                    walk(it, is, &format!("{path}[{i}]"), out);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shipped_defaults_lint_clean() {
        let v: Value = serde_json::from_str(payload::DEFAULT_RULES).unwrap();
        assert!(lint_config(&v).is_empty(), "{:?}", lint_config(&v));
        assert!(lint_warnings(&v).is_empty(), "{:?}", lint_warnings(&v));
    }

    #[test]
    fn catches_what_the_daemon_rejects() {
        let mut v: Value = serde_json::from_str(payload::DEFAULT_RULES).unwrap();
        v["zones"]["system"]["anchor"] = json!("middle");
        v["rules"] = json!([
            {"name": "py", "match": {"appName": "(?P<x>steam)"}},
            {"name": "flag", "match": {"appName": "(?i)steam"}},
            {"name": "zone", "zone": "ghost"},
            {"name": "open", "match": {"summary": "(abc"}},
            {"name": "field", "match": {"nope": "a"}}
        ]);
        let e = lint_config(&v).join("\n");
        for want in ["anchor must be", "(?P<name>", "inline flags", "unknown zone 'ghost'", "bad regex for summary", "unknown match field 'nope'"] {
            assert!(e.contains(want) || e.contains(&want.replace("(?P<name>", "(?<name>")), "missing `{want}` in:\n{e}");
        }
    }

    #[test]
    fn checks_look_and_behavior_keys_like_the_daemon() {
        let mut v: Value = serde_json::from_str(payload::DEFAULT_RULES).unwrap();
        v["theme"]["opacity"] = json!(5);
        v["zones"]["system"]["group"] = json!("x");
        v["zones"]["system"]["width"] = json!(10);
        v["rules"] = json!([
            {"name": "a", "opacity": 0, "match": {"category": "^email"}},
            {"name": "b", "clickThrough": "yes", "fontSize": 400, "group": 5, "timerStyle": "sparkle"},
            {"name": "ok", "opacity": 0.5, "clickThrough": false, "noFocus": true, "width": 400, "fontFamily": "Serif", "fontSize": 14, "group": "g", "match": {"channel": "web"}}
        ]);
        let e = lint_config(&v);
        for want in ["theme: opacity must be", "zone 'system': group can only be set on a rule", "zone 'system': width must be", "rule 'a': opacity must be", "rule 'b': clickThrough must be", "rule 'b': fontSize must be", "rule 'b': group must be", "rule 'b': timerStyle must be"] {
            assert!(e.iter().any(|x| x.contains(want)), "missing `{want}` in {e:?}");
        }
        assert!(!e.iter().any(|x| x.contains("rule 'ok'")), "a valid rule must pass: {e:?}");
        assert!(lint_warnings(&json!({"rules": [{"name": "ok", "opacity": 0.5, "group": "g", "match": {"category": "x"}}]})).is_empty(), "the schema must know the new keys");
    }

    #[test]
    fn refuses_patterns_the_daemon_refuses() {
        assert!(regex_problem(&"a".repeat(500)).is_none());
        assert!(regex_problem(&"a".repeat(501)).is_some_and(|w| w.contains("longer than 500")));
    }

    #[test]
    fn accepts_javascript_only_regex_features() {
        // lookbehind and named groups are valid ES; the old Python lint wrongly rejected some of these shapes
        assert!(regex_problem(r"(?<=a)b").is_none());
        assert!(regex_problem(r"(?<name>steam)").is_none());
        assert!(regex_problem(r"\b(example-shop|example-chat)\b").is_none());
        assert!(regex_problem("(").is_some());
    }

    #[test]
    fn warns_about_typos_but_not_errors() {
        let mut v: Value = serde_json::from_str(payload::DEFAULT_RULES).unwrap();
        v["defualtZone"] = json!("x");
        assert!(lint_config(&v).is_empty());
        assert!(lint_warnings(&v).iter().any(|w| w.contains("defualtZone")));
    }
}
