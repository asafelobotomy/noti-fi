//! Integration tests: the real `notify` binary against a real (private) notifd.
//! Runs in a fake home so nothing on your machine is read or changed except by the throwaway daemon.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use notify_testkit::{Checks, TestDaemon, test_config, wait_until};
use serde_json::Value;

struct Cli {
    sock: PathBuf,
    bus: String,
    home: PathBuf,
}

impl Cli {
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let o = Command::new(env!("CARGO_BIN_EXE_notify"))
            .args(args)
            .env("NOTIFD_SOCKET", &self.sock)
            .env("DBUS_SESSION_BUS_ADDRESS", &self.bus)
            .env("XDG_CONFIG_HOME", self.home.join("config"))
            .env("XDG_DATA_HOME", self.home.join("data"))
            .env("XDG_STATE_HOME", self.home.join("state"))
            .output()
            .expect("run notify");
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
    }
    fn json(&self, args: &[&str]) -> Value {
        let (_, out, err) = self.run(args);
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("not JSON ({e}): {out}\n{err}"))
    }
}

#[test]
fn the_notify_binary_end_to_end() {
    let d = TestDaemon::start(test_config());
    let home = d.dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let cli = Cli { sock: d.sock.clone(), bus: d.bus.clone(), home };
    let mut k = Checks::default();

    // ---- discoverability
    let (c, out, _) = cli.run(&["--help"]);
    k.check("--help lists the commands", c == 0 && ["status", "doctor", "takeover", "update", "restore", "simulate", "preview"].iter().all(|w| out.contains(w)), || out.clone());
    let (c, out, _) = cli.run(&["--version"]);
    k.check("--version prints a version", c == 0 && out.starts_with("notify "), || out.clone());

    // ---- status
    let (c, out, _) = cli.run(&["status"]);
    k.check("status says who owns notifications and the daemon state", c == 0 && out.contains("Notification ownership") && out.contains("Do Not Disturb off"), || out.clone());
    let j = cli.json(&["status", "--json"]);
    k.check("status --json has owner, daemon and config sections", j.get("daemon").is_some() && j["daemon"]["zones"].as_array().map(|z| z.len()) == Some(3) && j["config"]["ok"] == true, || j.to_string());

    // ---- Do Not Disturb and mutes
    let (c, out, _) = cli.run(&["dnd", "on"]);
    k.check("dnd on turns it on", c == 0 && out.contains("Do Not Disturb: ON"), || out.clone());
    let j = cli.json(&["dnd", "status", "--json"]);
    k.check("dnd status --json reflects it", j["dnd"] == true && j["holding"] == "dnd", || j.to_string());
    cli.run(&["dnd", "off"]);
    let (c, out, _) = cli.run(&["dnd", "toggle"]);
    k.check("dnd toggle flips it", c == 0 && out.contains("ON"), || out.clone());
    cli.run(&["dnd", "toggle"]);
    // focus mode
    let (c, out, _) = cli.run(&["focus", "list"]);
    k.check("focus list shows the default 'urgent' mode", c == 0 && out.contains("urgent") && out.contains("Urgent only"), || out.clone());
    let (c, out, _) = cli.run(&["focus", "urgent"]);
    k.check("focus <mode> turns it on and the status line says so", c == 0 && out.contains("focus: urgent"), || out.clone());
    let j = cli.json(&["focus", "--json"]);
    k.check("focus status --json reflects it", j["focus"] == "urgent" && j["holding"] == "focus", || j.to_string());
    let (c, _, err) = cli.run(&["focus", "ghost"]);
    k.check("an unknown focus mode is refused with the names that exist", c != 0 && err.contains("urgent"), || err.clone());
    let (c, out, _) = cli.run(&["focus", "off"]);
    k.check("focus off turns it off", c == 0 && !out.contains("focus:"), || out.clone());
    let (c, out, _) = cli.run(&["mute", "business"]);
    k.check("mute silences a zone", c == 0 && out.contains("muted zones: business"), || out.clone());
    let (c, out, _) = cli.run(&["unmute", "business"]);
    k.check("unmute restores it", c == 0 && out.contains("muted zones: none"), || out.clone());
    let (c, _, err) = cli.run(&["mute", "nowhere"]);
    k.check("muting an unknown zone fails with the daemon's reason", c == 1 && err.contains("unknown zone"), || err.clone());

    // ---- simulate
    let (c, out, _) = cli.run(&["simulate", "x", "--app", "kernel"]);
    k.check("simulate names the zone and the rule that decided it", c == 0 && out.contains("zone:     system (rule 'system')") && out.contains("outcome:  shown"), || out.clone());
    let j = cli.json(&["simulate", "x", "--app", "Brave Origin", "--origin", "example-shop.com", "--json"]);
    k.check("simulate --json returns typed fields", j["zone"] == "business" && j["wouldShow"] == true && j["matched"][0] == "business sites", || j.to_string());
    let (c, out, _) = cli.run(&["simulate", "x", "--app", "Steam", "--timeout", "0"]);
    k.check("simulate honours --timeout 0 (never expires)", c == 0 && out.contains("stays until dismissed"), || out.clone());
    cli.run(&["dnd", "on"]);
    let (_, held, _) = cli.run(&["simulate", "x", "--app", "Steam"]);
    let (_, free, _) = cli.run(&["simulate", "x", "--app", "Steam", "--ignore-state"]);
    cli.run(&["dnd", "off"]);
    k.check("simulate shows the hold, and --ignore-state shows the rule outcome", held.contains("NOT shown: dnd") && free.contains("outcome:  shown"), || format!("{held}\n{free}"));

    // ---- rules lint (daemon-checked)
    let draft_dir = d.dir.join("drafts");
    std::fs::create_dir_all(&draft_dir).unwrap();
    let good = draft_dir.join("good.json");
    std::fs::write(&good, test_config().to_json_pretty()).unwrap();
    let (c, out, _) = cli.run(&["rules", "--file", good.to_str().unwrap()]);
    k.check("rules --file accepts a good draft and says the daemon agrees", c == 0 && out.contains("the running daemon accepts this config"), || out.clone());
    let bad = draft_dir.join("bad.json");
    std::fs::write(&bad, r#"{"defualtZone":"x","zones":{"a":{"anchor":"middle"}},"rules":[{"name":"r","match":{"appName":"(?P<x>a)"}}]}"#).unwrap();
    let (c, out, _) = cli.run(&["rules", "--file", bad.to_str().unwrap()]);
    k.check("rules --file rejects a bad draft with every reason", c == 1 && out.contains("anchor must be") && out.contains("named groups") && out.contains("unknown setting 'defualtZone'") && out.contains("REJECTS"), || out.clone());
    let j = cli.json(&["rules", "--file", bad.to_str().unwrap(), "--json"]);
    k.check("rules --json lists errors, warnings and the daemon verdict", j["errors"].as_array().map(|e| !e.is_empty()) == Some(true) && j["warnings"][0].as_str().unwrap_or("").contains("defualtZone") && j["daemon"]["ok"] == false, || j.to_string());

    // ---- history, send, test
    let (c, _, _) = cli.run(&["send", "hello from the cli", "body", "-a", "Steam", "-t", "800"]);
    k.check("send delivers a notification", c == 0, || "send failed".into());
    let got = wait_until(Duration::from_secs(4), || cli.run(&["history", "-n", "3"]).1.contains("hello from the cli"));
    k.check("history shows what was sent", got, || cli.run(&["history"]).1);
    cli.run(&["dnd", "on"]);
    cli.run(&["send", "held by dnd", "-a", "Steam", "-t", "500"]);
    wait_until(Duration::from_secs(4), || cli.run(&["history", "--missed"]).1.contains("held by dnd"));
    let (_, out, _) = cli.run(&["history", "--missed"]);
    cli.run(&["dnd", "off"]);
    k.check("history --missed lists held notifications with the reason", out.contains("held by dnd") && out.contains("[dnd]"), || out.clone());
    let j = cli.json(&["history", "-n", "2", "--json"]);
    k.check("history --json is a typed list", j.as_array().map(|a| a.len() == 2 && a[0]["summary"].is_string() && a[0]["shown"].is_boolean()) == Some(true), || j.to_string());
    let (_, out, _) = cli.run(&["history", "--app", "^steam$", "-n", "5"]);
    k.check("history --app filters by regex", out.contains("Steam") && !out.contains("Brave"), || out.clone());
    let before = cli.json(&["history", "-n", "100", "--json"]).as_array().map(|a| a.len()).unwrap_or(0);
    let (c, out, _) = cli.run(&["test", "-t", "600"]);
    k.check("test sends the six sample notifications", c == 0 && out.contains("sent 6"), || out.clone());
    let grew = wait_until(Duration::from_secs(6), || cli.json(&["history", "-n", "100", "--json"]).as_array().map(|a| a.len()).unwrap_or(0) >= before + 6);
    k.check("...and they all reach the daemon", grew, || "fewer than 6 new entries".into());
    let (c, out, _) = cli.run(&["history", "--clear"]);
    k.check("history --clear empties it", c == 0 && out.contains("history cleared") && cli.json(&["history", "--json"]).as_array().map(|a| a.is_empty()) == Some(true), || out.clone());

    // ---- preview, dismiss, reply
    let (c, out, _) = cli.run(&["preview", "--seconds", "2"]);
    k.check("preview shows five sample popups", c == 0 && out.contains("previewing 5 sample popups for 2s"), || out.clone());
    let (c, _, _) = cli.run(&["preview", "--end"]);
    k.check("preview --end stops it", c == 0 && wait_until(Duration::from_secs(4), || cli.json(&["status", "--json"])["daemon"]["active"] == 0), || "still previewing".into());
    cli.run(&["send", "stay", "-a", "Steam", "-t", "0"]);
    wait_until(Duration::from_secs(4), || cli.json(&["status", "--json"])["daemon"]["active"] == 1);
    let (c, _, _) = cli.run(&["dismiss"]);
    k.check("dismiss clears popups", c == 0 && wait_until(Duration::from_secs(4), || cli.json(&["status", "--json"])["daemon"]["active"] == 0), || "popup remained".into());
    let (c, _, err) = cli.run(&["reply", "999999", "hi"]);
    k.check("reply to a missing notification fails clearly", c == 1 && err.contains("no such notification"), || err.clone());

    // ---- doctor (fake home: nothing installed)
    let j = cli.json(&["doctor", "--json"]);
    k.check("doctor --json has checks, status, rules, apps and a summary", ["checks", "status", "rules", "apps", "summary"].iter().all(|w| j.get(*w).is_some()), || j.to_string());
    let (c, out, _) = cli.run(&["doctor"]);
    k.check("doctor (text) reports takeover not installed in a fresh home", c == 0 && out.contains("takeover not installed"), || out.clone());

    // ---- no daemon
    let o = Command::new(env!("CARGO_BIN_EXE_notify")).args(["dnd", "status"]).env("NOTIFD_SOCKET", d.dir.join("nothing-here.sock")).output().unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    k.check("without a daemon, commands fail with advice", o.status.code() == Some(1) && err.contains("notifd is not running") && err.contains("systemctl --user start notifd"), || err.to_string());
    let o = Command::new(env!("CARGO_BIN_EXE_notify")).args(["--json", "dnd", "status"]).env("NOTIFD_SOCKET", d.dir.join("nothing-here.sock")).output().unwrap();
    let last: Value = serde_json::from_str(String::from_utf8_lossy(&o.stdout).trim()).unwrap_or(Value::Null);
    k.check("...and in --json mode with a failed result object", last["type"] == "result" && last["ok"] == false, || last.to_string());

    println!("\n{} passed, {} failed", k.passed, k.failed.len());
    assert!(k.failed.is_empty(), "failures:\n{}", k.failed.join("\n"));
}
