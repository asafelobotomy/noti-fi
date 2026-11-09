//! Tests of lint.rs: the static checks must agree with what notifd/Validate.qml refuses.
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
    (v["center"], v["autoDnd"]["ignoreCaptureBy"], v["theme"]["radius"]) = (json!({"anchor": "free", "keep": 5000}), json!(vec!["x"; 21]), json!(99));
    v["zones"]["system"]["group"] = json!("x");
    v["zones"]["system"]["width"] = json!(10);
    v["rules"] = json!([
        {"name": "a", "opacity": 0, "silent": "false", "match": {"category": "^email"}},
        {"name": "b", "clickThrough": "yes", "fontSize": 400, "group": 5, "timerStyle": "sparkle"},
        {"name": "ok", "opacity": 0.5, "clickThrough": false, "noFocus": true, "width": 400, "fontFamily": "Serif", "fontSize": 14, "group": "g", "match": {"channel": "web"}}
    ]);
    let e = lint_config(&v);
    for want in ["theme: opacity must be", "zone 'system': group can only be set on a rule", "zone 'system': width must be", "rule 'a': opacity must be", "rule 'b': clickThrough must be", "rule 'b': fontSize must be", "rule 'b': group must be", "rule 'b': timerStyle must be",
                 "center.anchor must be", "center.keep must be", "ignoreCaptureBy must be", "theme.radius must be", "rule 'a': silent must be"] {
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
    // lookbehind and named groups are valid ES (the daemon's regex engine)
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
