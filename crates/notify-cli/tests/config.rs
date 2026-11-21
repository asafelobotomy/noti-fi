//! `notify config`, `notify history --clear --dry-run`, `notify history --no-text`, against a small scripted notifd in a fake home.
//! (`notify set` and `notify settings` have their own file, settings.rs.) The plan documents are what uQ's Settings page shows
//! before a change; tests/uq/fixtures/ holds copies the page tests use, checked here against the real commands.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::Command;
use std::thread;

use notify_protocol::{Config, PROTOCOL_VERSION};
use serde_json::{Value, json};

const START: &str = r##"{ "maxVisible": 7, "theme": { "card": "#101010" }, "zones": { "personal": { "anchor": "top-right" } } }
"##;

struct Fixture {
    dir: PathBuf,
    sock: PathBuf,
    rules: PathBuf,
}

fn entry(id: i64, app: &str, shown: bool) -> Value {
    json!({"t": 1_789_000_000_000_i64 + id * 60_000, "id": id, "app": app, "summary": "SECRET title", "body": "SECRET body", "zone": "personal",
           "origin": "", "urgency": "normal", "shown": shown, "why": if shown { "" } else { "dnd" }, "icon": "chat"})
}

fn serve_one(stream: std::os::unix::net::UnixStream, rules: PathBuf, history: Vec<Value>) {
    let mut out = stream.try_clone().unwrap();
    let _ = writeln!(out, "{}", json!({"event": "hello", "protocol": PROTOCOL_VERSION, "daemon": "fake-1", "pid": 1, "seq": 0}));
    let mut line = String::new();
    let mut reader = BufReader::new(stream);
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let Ok(req) = serde_json::from_str::<Value>(&line) else { continue };
        let result = match req["method"].as_str().unwrap_or("") {
            "status" => {
                let mut s = notify_testkit::fake::status_json();
                s["history"] = json!(history.len());
                s
            }
            "history" => json!(history),
            "configGet" => {
                let raw = std::fs::read_to_string(&rules).unwrap_or_default();
                json!({"raw": raw, "effective": serde_json::from_str::<Config>(&raw).unwrap_or_default(), "error": ""})
            }
            "validate" => {
                let text = req["params"]["text"].as_str().unwrap_or("");
                match serde_json::from_str::<Value>(text) {
                    Ok(v) if v["theme"]["opacity"] == json!(0.05) => json!({"ok": false, "error": "theme.opacity: too see-through"}),
                    Ok(_) => json!({"ok": true, "rules": 0}),
                    Err(e) => json!({"ok": false, "error": e.to_string()}),
                }
            }
            _ => json!({}),
        };
        let _ = writeln!(out, "{}", json!({"id": req["id"], "result": result}));
    }
}

fn fixture(name: &str, history: Vec<Value>) -> Fixture {
    let dir = std::env::temp_dir().join(format!("notify-config-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rules = dir.join("rules.json");
    std::fs::write(&rules, START).unwrap();
    let sock = dir.join("control.sock");
    let listener = UnixListener::bind(&sock).unwrap();
    let r2 = rules.clone();
    thread::spawn(move || {
        for s in listener.incoming().flatten() {
            let (r3, h) = (r2.clone(), history.clone());
            thread::spawn(move || serve_one(s, r3, h));
        }
    });
    Fixture { dir, sock, rules }
}

impl Fixture {
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let o = Command::new(env!("CARGO_BIN_EXE_notify"))
            .args(args)
            .env("NOTIFD_SOCKET", &self.sock)
            .env("NOTIFD_RULES", &self.rules)
            .env("XDG_CONFIG_HOME", self.dir.join("config"))
            .env("XDG_DATA_HOME", self.dir.join("data"))
            .env("XDG_STATE_HOME", self.dir.join("state"))
            .output()
            .expect("run notify");
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
    }
    fn json(&self, args: &[&str]) -> Value {
        let (c, out, err) = self.run(args);
        assert_eq!(c, 0, "{args:?}: {err}");
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("{args:?}: not one JSON document ({e}): {out}"))
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

/// The page's tests read tests/uq/fixtures/<name>: it must be exactly what the command printed. UPDATE_FIXTURES=1 rewrites it.
fn same_as_fixture(name: &str, doc: &Value) {
    let path = format!("{}/../../tests/uq/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        std::fs::write(&path, serde_json::to_string_pretty(doc).unwrap() + "\n").unwrap();
    }
    let want: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(doc, &want, "tests/uq/fixtures/{name} is out of date (UPDATE_FIXTURES=1 cargo test -p notify-cli --test config)");
}

fn row<'a>(doc: &'a Value, key: &str) -> &'a Value {
    doc["settings"].as_array().unwrap().iter().find(|r| r["key"] == key).unwrap_or_else(|| panic!("no {key} in {doc}"))
}

#[test]
fn show_lists_every_setting_with_its_default_and_whether_the_file_sets_it() {
    let f = fixture("show", vec![]);
    let doc = f.json(&["--json", "config", "show"]);
    assert_eq!(row(&doc, "maxVisible"), &json!({"key": "maxVisible", "type": "number", "default": 5, "value": 7, "set": true}));
    assert_eq!(row(&doc, "width")["value"], 380);
    assert_eq!(row(&doc, "width")["set"], false);
    assert_eq!(row(&doc, "theme.card")["type"], "color");
    assert_eq!(row(&doc, "history.persist")["type"], "bool");
    assert!(row(&doc, "center.anchor")["choices"].as_array().unwrap().contains(&json!("top-left")));
    assert_eq!(f.json(&["config", "show", "--json"]), doc, "bare `config` and `config show` agree");
    let (c, out, _) = f.run(&["config"]);
    assert!(c == 0 && out.contains("maxVisible: 7") && out.contains("width: 380  (default)"), "{out}");
    let fixture: Value =
        serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/uq/fixtures/config.json")).unwrap()).unwrap();
    let mut shown = doc.clone();
    shown["file"] = fixture["file"].clone(); // the path differs per machine
    assert_eq!(shown, fixture, "tests/uq/fixtures/config.json is out of date");
}

#[test]
fn show_works_without_a_daemon() {
    let f = fixture("show-offline", vec![]);
    let (c, out, err) = Command::new(env!("CARGO_BIN_EXE_notify"))
        .args(["--json", "config", "show"])
        .env("NOTIFD_SOCKET", f.dir.join("nothing.sock"))
        .env("NOTIFD_RULES", &f.rules)
        .output()
        .map(|o| (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned()))
        .unwrap();
    assert_eq!(c, 0, "{err}");
    assert!(out.contains("maxVisible"));
}

#[test]
fn set_and_reset_change_one_key_and_keep_the_rest() {
    let f = fixture("set", vec![]);
    for (key, value, want) in [
        ("width", "420", json!(420)),
        ("sound.enabled", "off", json!(false)),
        ("theme.card", "#222233", json!("#222233")),
        ("theme.timerStyle", "line,circle", json!("line,circle")),
        ("center.anchor", "top-left", json!("top-left")),
    ] {
        let (c, out, err) = f.run(&["config", "set", key, value]);
        assert_eq!(c, 0, "{key}: {err}");
        assert!(out.contains(key), "{out}");
        let path: Vec<&str> = key.split('.').collect();
        let got = path.iter().fold(f.file(), |v, k| v[*k].clone());
        assert_eq!(got, want, "{key} is in the file");
        assert_eq!(row(&f.json(&["--json", "config", "show"]), key)["value"], want);
    }
    assert_eq!(f.file()["maxVisible"], 7);
    assert_eq!(f.file()["zones"]["personal"]["anchor"], "top-right");
    let (c, out, err) = f.run(&["config", "reset", "width"]);
    assert_eq!(c, 0, "{err}");
    assert!(out.contains("default"), "{out}");
    assert!(f.file().get("width").is_none());
    assert_eq!(row(&f.json(&["--json", "config", "show"]), "width")["set"], false);
    let (c, out, _) = f.run(&["--json", "config", "set", "maxVisible", "7"]);
    assert!(c == 0 && out.contains("already"), "an unchanged value says so: {out}");
}

#[test]
fn a_bad_key_or_value_is_a_usage_error_and_writes_nothing() {
    let f = fixture("refuse", vec![]);
    let before = std::fs::read_to_string(&f.rules).unwrap();
    for args in [
        vec!["config", "set", "nonsense", "1"],
        vec!["config", "set", "zones", "1"],
        vec!["config", "set", "width", "wide"],
        vec!["config", "set", "sound.enabled", "maybe"],
        vec!["config", "set", "theme.card", "red"],
        vec!["config", "set", "theme.timerStyle", "spinning"],
        vec!["config", "set", "center.anchor", "nowhere"],
        vec!["config", "reset", "nonsense"],
    ] {
        let (c, _, err) = f.run(&args);
        assert_eq!(c, 2, "{args:?}: {err}");
        assert_eq!(std::fs::read_to_string(&f.rules).unwrap(), before, "{args:?} wrote something");
    }
    let (c, _, err) = f.run(&["config", "set", "width", "-5"]);
    assert_ne!(c, 0, "a negative size is refused: {err}");
    let (c, _, err) = f.run(&["config", "set", "theme.opacity", "0.05"]);
    assert!(c == 1 && err.contains("too see-through"), "the daemon's reason is shown: {err}");
    assert_eq!(std::fs::read_to_string(&f.rules).unwrap(), before);
}

#[test]
fn dry_run_prints_a_plan_and_writes_nothing() {
    let f = fixture("dry", vec![]);
    let before = std::fs::read_to_string(&f.rules).unwrap();
    let doc = f.json(&["--json", "config", "set", "--dry-run", "width", "420"]);
    assert_eq!(doc["summary"], "Set width to 420");
    assert_eq!(doc["changes"], json!([{"group": "rules.json", "label": "width", "detail": "380 to 420", "reversible": true}]));
    assert_eq!((doc["would_refuse"].clone(), doc["reason"].clone()), (json!(false), json!("")));
    let refused = f.json(&["--json", "config", "set", "--dry-run", "theme.opacity", "0.05"]);
    assert_eq!(refused["would_refuse"], true);
    assert!(refused["reason"].as_str().unwrap().contains("too see-through"));
    let same = f.json(&["--json", "config", "set", "--dry-run", "maxVisible", "7"]);
    assert_eq!(same["changes"], json!([]));
    same_as_fixture("config-set-plan.json", &doc);
    let reset = f.json(&["--json", "config", "reset", "--dry-run", "maxVisible"]);
    assert_eq!(reset["changes"][0]["detail"], "7 to 5");
    assert_eq!(std::fs::read_to_string(&f.rules).unwrap(), before);
}

#[test]
fn clear_history_dry_run_counts_by_app_and_clears_nothing() {
    let f = fixture("history", vec![entry(1, "Chat", true), entry(2, "Mail", true), entry(3, "Chat", false)]);
    let doc = f.json(&["--json", "history", "--clear", "--dry-run"]);
    assert_eq!(doc["summary"], "Clear 3 notifications");
    assert_eq!(doc["would_remove"], 3);
    assert_eq!(
        doc["changes"],
        json!([
            {"group": "History", "label": "Chat", "detail": "2 notifications", "reversible": false},
            {"group": "History", "label": "Mail", "detail": "1 notification", "reversible": false},
        ])
    );
    assert_eq!((doc["oldest"].clone(), doc["newest"].clone()), (json!(1_789_000_060_000_i64), json!(1_789_000_180_000_i64)));
    assert_eq!(doc["would_refuse"], false);
    same_as_fixture("history-clear-plan.json", &doc);
    let text = f.run(&["history", "--clear", "--dry-run"]).1;
    assert!(text.contains("Clear 3 notifications") && text.contains("Chat: 2 notifications"), "{text}");
    let empty = fixture("history-empty", vec![]);
    let doc = empty.json(&["--json", "history", "--clear", "--dry-run"]);
    assert_eq!((doc["would_refuse"].clone(), doc["reason"].clone(), doc["changes"].clone()), (json!(true), json!("The history is already empty."), json!([])));
    assert_ne!(f.run(&["history", "--dry-run"]).0, 0, "--dry-run is only for --clear");
}

#[test]
fn no_text_leaves_the_words_out() {
    let f = fixture("notext", vec![entry(1, "Chat", true), entry(2, "Mail", false)]);
    let (c, out, err) = f.run(&["--json", "history", "-n", "5", "--no-text"]);
    assert_eq!(c, 0, "{err}");
    assert!(!out.contains("SECRET"), "{out}");
    let rows: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(rows[1]["app"], "Mail");
    assert_eq!(rows[1]["why"], "dnd");
    assert_eq!(rows[0]["icon"], "chat");
    same_as_fixture("history.json", &rows);
    assert!(f.run(&["--json", "history"]).1.contains("SECRET"), "without the flag the text is there");
}
