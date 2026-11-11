//! `notify settings` and `notify set`, against a small scripted notifd (no Quickshell needed) in a fake home.
//!
//! The scripted daemon answers `status`, `configGet` (reading the rules.json it was given, as notifd does) and `validate`
//! (the config must parse; a limit of 1000 is refused, to prove a daemon refusal is honoured). The real daemon's own
//! acceptance of the files `set` writes is covered by the daemon suites.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

use notify_protocol::{Config, PROTOCOL_VERSION};
use notify_testkit::fake::status_json;
use serde_json::{Value, json};

const START: &str = r#"{
  "maxVisible": 7,
  "zones": {
    "personal": { "anchor": "top-right", "label": "Personal" },
    "work": { "anchor": "top-left" }
  },
  "rules": [
    { "name": "Quiet chat", "match": { "appName": "^chat$" }, "silent": true },
    { "match": { "appName": "^other$" }, "timeout": 2000 }
  ]
}
"#;

struct Fixture {
    dir: PathBuf,
    sock: PathBuf,
    rules: PathBuf,
}

fn serve_one(stream: std::os::unix::net::UnixStream, rules: PathBuf) {
    let mut out = stream.try_clone().unwrap();
    let hello = json!({"event": "hello", "protocol": PROTOCOL_VERSION, "daemon": "fake-1", "pid": 1, "seq": 0});
    let _ = writeln!(out, "{hello}");
    let mut line = String::new();
    let mut reader = BufReader::new(stream);
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let Ok(req) = serde_json::from_str::<Value>(&line) else { continue };
        let result = match req["method"].as_str().unwrap_or("") {
            "status" => status_json(),
            "configGet" => {
                let raw = std::fs::read_to_string(&rules).unwrap_or_default();
                let effective = serde_json::from_str::<Config>(&raw).unwrap_or_default();
                let error = if raw.contains("BROKEN") { "rule 3: something is wrong" } else { "" };
                json!({"raw": raw, "effective": effective, "error": error})
            }
            "validate" => {
                let text = req["params"]["text"].as_str().unwrap_or("");
                match serde_json::from_str::<Config>(text) {
                    Ok(c) if c.history.limit == 1000 => json!({"ok": false, "error": "history.limit: too big for this test"}),
                    Ok(c) => json!({"ok": true, "rules": c.rules.len()}),
                    Err(e) => json!({"ok": false, "error": e.to_string()}),
                }
            }
            _ => json!({}),
        };
        let _ = writeln!(out, "{}", json!({"id": req["id"], "result": result}));
    }
}

fn fixture(name: &str, initial: &str) -> Fixture {
    let dir = std::env::temp_dir().join(format!("notify-settings-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rules = dir.join("rules.json");
    std::fs::write(&rules, initial).unwrap();
    let sock = dir.join("control.sock");
    let listener = UnixListener::bind(&sock).unwrap();
    let r2 = rules.clone();
    thread::spawn(move || {
        for s in listener.incoming().flatten() {
            let r3 = r2.clone();
            thread::spawn(move || serve_one(s, r3));
        }
    });
    Fixture { dir, sock, rules }
}

impl Fixture {
    fn run_at(&self, sock: &Path, args: &[&str]) -> (i32, String, String) {
        let o = Command::new(env!("CARGO_BIN_EXE_notify"))
            .args(args)
            .env("NOTIFD_SOCKET", sock)
            .env("NOTIFD_RULES", &self.rules)
            .env("XDG_CONFIG_HOME", self.dir.join("config"))
            .env("XDG_DATA_HOME", self.dir.join("data"))
            .env("XDG_STATE_HOME", self.dir.join("state"))
            .output()
            .expect("run notify");
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
    }
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        self.run_at(&self.sock, args)
    }
    fn settings(&self) -> Value {
        let (c, out, err) = self.run(&["settings", "--json"]);
        assert_eq!(c, 0, "settings failed: {err}");
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("not one JSON document ({e}): {out}"))
    }
    fn file(&self) -> Value {
        serde_json::from_str(&std::fs::read_to_string(&self.rules).unwrap()).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn settings_reports_the_effective_values_and_rule_names_only() {
    let f = fixture("report", START);
    let j = f.settings();
    // the uQ page's fixture is this command's output (tests/uq/fixtures/settings.json)
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/uq/fixtures/settings.json")).unwrap()).unwrap();
    assert_eq!(j, fixture, "tests/uq/fixtures/settings.json is out of date");
    assert_eq!(j["dnd"]["allowCritical"], true);
    assert_eq!(j["history"], json!({"limit": 200, "persist": false}));
    assert_eq!(j["autoDnd"], json!({"fullscreen": true, "screenShare": true}));
    assert_eq!(j["rules"], json!([{"name": "Quiet chat"}, {"name": "Rule 2"}]), "names only; an unnamed rule is numbered");
    let zones = j["zones"].as_array().unwrap();
    assert!(zones.contains(&json!({"id": "personal", "label": "Personal"})), "{zones:?}");
    assert!(zones.contains(&json!({"id": "work", "label": "work"})), "a zone with no label is shown by its id: {zones:?}");
    let (c, out, _) = f.run(&["settings"]);
    assert!(c == 0 && out.contains("history.limit: 200") && out.contains("Quiet chat"), "{out}");
}

#[test]
fn each_key_round_trips_and_nothing_else_changes() {
    let f = fixture("roundtrip", START);
    let cases = [
        ("history.limit", "500", json!(["history", "limit"]), json!(500)),
        ("history.persist", "on", json!(["history", "persist"]), json!(true)),
        ("dnd.allowCritical", "off", json!(["dnd", "allowCritical"]), json!(false)),
        ("autoDnd.fullscreen", "off", json!(["autoDnd", "fullscreen"]), json!(false)),
        ("autoDnd.screenShare", "off", json!(["autoDnd", "screenShare"]), json!(false)),
    ];
    for (key, value, path, want) in cases {
        let (c, out, err) = f.run(&["set", key, value]);
        assert_eq!(c, 0, "{key} {value}: {err}");
        assert!(out.contains(key), "the answer names the key: {out}");
        let file = f.file();
        assert_eq!(file[path[0].as_str().unwrap()][path[1].as_str().unwrap()], want, "{key} is in the file");
        let j = f.settings();
        assert_eq!(j[path[0].as_str().unwrap()][path[1].as_str().unwrap()], want, "{key} reads back");
        assert_eq!(file["maxVisible"], 7, "other settings survive");
        assert_eq!(file["rules"].as_array().unwrap().len(), 2, "rules survive");
        assert_eq!(file["zones"]["work"]["anchor"], "top-left", "zones survive");
    }
    // and back
    assert_eq!(f.run(&["set", "history.persist", "off"]).0, 0);
    assert_eq!(f.settings()["history"]["persist"], false);
}

#[test]
fn json_mode_ends_with_one_result_line() {
    let f = fixture("json", START);
    let (c, out, _) = f.run(&["--json", "set", "history.limit", "100"]);
    let last: Value = serde_json::from_str(out.lines().last().unwrap()).unwrap();
    assert!(c == 0 && last["type"] == "result" && last["ok"] == true, "{out}");
}

#[test]
fn an_unknown_key_or_a_bad_value_is_refused_with_exit_2_and_nothing_written() {
    let f = fixture("refuse", START);
    let before = std::fs::read_to_string(&f.rules).unwrap();
    for args in [
        vec!["set", "theme.card", "red"],
        vec!["set", "history", "50"],
        vec!["set", "history.limit", "7"],
        vec!["set", "history.limit", "on"],
        vec!["set", "history.limit", "-1"],
        vec!["set", "history.persist", "maybe"],
        vec!["set", "history.persist", "true"],
        vec!["set", "dnd.allowCritical", "1"],
        vec!["set", "autoDnd.screenShare", "200"],
        vec!["set", "autoDnd.fullscreen", ""],
    ] {
        let (c, _, err) = f.run(&args);
        assert_eq!(c, 2, "{args:?} should be a usage error: {err}");
        assert_eq!(std::fs::read_to_string(&f.rules).unwrap(), before, "{args:?} wrote something");
    }
}

#[test]
fn a_refusal_by_the_daemon_writes_nothing() {
    let f = fixture("daemon-refuses", START);
    let before = std::fs::read_to_string(&f.rules).unwrap();
    let (c, _, err) = f.run(&["set", "history.limit", "1000"]);
    assert_eq!(c, 1, "{err}");
    assert!(err.contains("too big for this test"), "the daemon's reason is shown: {err}");
    assert_eq!(std::fs::read_to_string(&f.rules).unwrap(), before);
}

#[test]
fn a_config_with_an_error_elsewhere_is_left_untouched() {
    let broken = START.replace("\"maxVisible\": 7", "\"maxVisible\": 7, \"BROKEN\": 1");
    let f = fixture("broken", &broken);
    let (c, _, err) = f.run(&["set", "history.limit", "50"]);
    assert_eq!(c, 1, "{err}");
    assert!(err.contains("rule 3: something is wrong") && err.contains("nothing was changed"), "{err}");
    assert_eq!(std::fs::read_to_string(&f.rules).unwrap(), broken);
}

#[test]
fn a_value_already_set_is_not_rewritten() {
    let f = fixture("same", START);
    assert_eq!(f.run(&["set", "history.limit", "500"]).0, 0);
    let after_first = std::fs::read_to_string(&f.rules).unwrap();
    let (c, out, _) = f.run(&["set", "history.limit", "500"]);
    assert!(c == 0 && out.contains("already"), "{out}");
    assert_eq!(std::fs::read_to_string(&f.rules).unwrap(), after_first);
}

#[test]
fn a_missing_rules_file_is_created_with_only_the_one_setting() {
    let f = fixture("fresh", START);
    std::fs::remove_file(&f.rules).unwrap();
    let (c, _, err) = f.run(&["set", "dnd.allowCritical", "off"]);
    assert_eq!(c, 0, "{err}");
    assert_eq!(f.file(), json!({"dnd": {"allowCritical": false}}));
}

#[test]
fn without_a_daemon_both_commands_fail_and_nothing_is_written() {
    let f = fixture("no-daemon", START);
    let before = std::fs::read_to_string(&f.rules).unwrap();
    let nowhere = f.dir.join("nothing-here.sock");
    for args in [vec!["set", "history.limit", "50"], vec!["settings", "--json"]] {
        let (c, out, err) = f.run_at(&nowhere, &args);
        assert_eq!(c, 1, "{args:?}: {err}");
        // text mode reports on stderr; --json mode as a failed result object on stdout (as every command does)
        assert!(format!("{out}{err}").contains("not running"), "{args:?}: {out}{err}");
    }
    assert_eq!(std::fs::read_to_string(&f.rules).unwrap(), before);
}

/// The page's other fixtures parse as the protocol types the real commands print.
#[test]
fn the_uq_page_fixtures_have_the_protocols_shapes() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/uq/fixtures/");
    let read = |name: &str| -> Value { serde_json::from_str(&std::fs::read_to_string(format!("{dir}{name}")).unwrap()).unwrap() };
    let running = read("status.json");
    let status: notify_protocol::Status = serde_json::from_value(running["daemon"].clone()).expect("status.json: daemon is a Status");
    assert_eq!(serde_json::to_value(&status).unwrap(), running["daemon"], "no field is missing or invented");
    let stopped = read("status-not-running.json");
    assert!(stopped["daemon"].is_null() && stopped.as_object().unwrap().keys().eq(running.as_object().unwrap().keys()));
    let focus = read("focus-list.json");
    let modes: std::collections::BTreeMap<String, notify_protocol::FocusMode> = serde_json::from_value(focus["modes"].clone()).expect("focus-list.json: modes");
    assert_eq!(modes.len(), 2);
}
