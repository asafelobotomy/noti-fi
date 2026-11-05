//! Headless smoke tests of the REAL panel: the actual binary, the actual QML, run offscreen against a real
//! throwaway daemon or a scripted fake one. Settings go to a temporary folder, never your real config.
//!
//! The panel's own hooks make this possible: `--report` prints a JSON summary on exit, `--quit-after=N` ends the
//! run, and `--selftest` drives every UI command (a missing or renamed command is counted as a failure).

#![allow(unused_imports, dead_code)]

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use notify_client::Client;
use notify_protocol::Event;
use notify_testkit::fake::{fake_daemon, serve};
use notify_testkit::{Checks, TestDaemon, test_config, wait_until};
use serde_json::Value;

mod common;
use common::*;

#[test]
fn the_panel_shows_the_daemons_real_state() {
    let d = TestDaemon::start(test_config());
    let mut k = Checks::default();
    let mut c = d.client();
    c.dnd("on").unwrap();
    c.mute("business", true).unwrap();
    d.send("seed one", "Steam", 400, &[], "normal");
    d.send("seed two", "kernel", 400, &[], "normal");
    wait_until(Duration::from_secs(4), || c.history(10).map(|h| h.len() >= 2).unwrap_or(false));

    let r = run_panel(&d.sock, &["--quit-after=3", "--report"]);
    let rep = r.report();
    k.check("the panel starts and exits cleanly", r.code == 0, || format!("exit {} / {}", r.code, r.stderr));
    k.check("it connects and reports the daemon's version", rep["state"] == "connected" && rep["connected"] == true && rep["daemon_version"].as_str().map(|v| !v.is_empty()) == Some(true), || rep.to_string());
    k.check("it shows the state we set: Do Not Disturb on", rep["dnd"] == true && rep["holding"] == "dnd", || rep.to_string());
    k.check("it shows all three zones", rep["zones"] == 3, || rep.to_string());
    k.check("it shows the seeded history", rep["history_total"].as_u64().unwrap_or(0) >= 2 && rep["history_shown"] == rep["history_total"], || rep.to_string());
    k.check("it connected exactly once", rep["connections"] == 1, || rep.to_string());
    let ui = r.ui();
    k.check("the Overview DISPLAYS three zone cards", ui["page"] == "overview" && ui["zones"] == 3, || ui.to_string());
    k.check("...the Do Not Disturb switch is shown on, with the explanation", ui["switchOn"] == true && ui["subtitle"].as_str().unwrap_or("").starts_with("On:"), || ui.to_string());
    k.check("...and no error banner is shown", ui["banner"] == "", || ui.to_string());
    k.check("the QML produced no errors or warnings that indicate a broken UI", r.qml_errors().is_empty(), || r.qml_errors().join("\n"));

    println!("\n{} passed, {} failed", k.passed, k.failed.len());
    assert!(k.failed.is_empty(), "failures:\n{}", k.failed.join("\n"));
}

#[test]
fn selftest_drives_every_command_through_the_real_ui() {
    let d = TestDaemon::start(test_config());
    let mut k = Checks::default();
    // history with two apps, so the "steam" filter has something to exclude
    let mut c = d.client();
    d.send("from steam", "Steam", 300, &[], "normal");
    d.send("from kernel", "kernel", 300, &[], "normal");
    wait_until(Duration::from_secs(4), || c.history(10).map(|h| h.len() >= 2).unwrap_or(false));

    // watch what the daemon is told while the panel runs
    let mut watcher = d.client();
    watcher.subscribe().unwrap();
    let states: Arc<Mutex<Vec<(bool, Vec<String>)>>> = Arc::new(Mutex::new(Vec::new()));
    let (s2, stop) = (states.clone(), Arc::new(std::sync::atomic::AtomicBool::new(false)));
    let st2 = stop.clone();
    let th = thread::spawn(move || {
        while !st2.load(std::sync::atomic::Ordering::Relaxed) {
            if let Ok(Some(e)) = watcher.next_event(Duration::from_millis(100)) {
                if let Event::State(s) = e.event {
                    let mut g = s2.lock().unwrap();
                    let cur = (s.dnd, s.muted.clone());
                    if g.last() != Some(&cur) {
                        g.push(cur);
                    }
                }
            }
        }
    });

    let r = run_panel(&d.sock, &["--selftest", "--report"]);
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    th.join().unwrap();
    let seen = states.lock().unwrap().clone();
    let rep = r.report();

    k.check("the selftest finishes and exits cleanly", r.code == 0, || format!("exit {} / {}", r.code, r.stderr));
    k.check("every command the UI uses exists (no silent renames): SELFTEST failures=0", r.stderr.contains("SELFTEST failures=0") || r.stdout.contains("SELFTEST failures=0"), || r.stderr.clone());
    k.check("a refused command reaches the UI as a toast (exactly one expected)", r.stderr.contains("toasts=1") || r.stdout.contains("toasts=1"), || r.stderr.clone());
    let sequence_ok = {
        let want: Vec<(bool, Vec<&str>)> = vec![(true, vec![]), (true, vec!["business"]), (true, vec![]), (false, vec![])];
        let got: Vec<(bool, Vec<&str>)> = seen.iter().map(|(d, m)| (*d, m.iter().map(String::as_str).collect())).collect();
        // the first entry may be the initial state; compare the tail
        got.len() >= want.len() && got[got.len() - want.len()..] == want[..]
    };
    k.check("the daemon was told, in order: DND on, mute business, unmute, DND off", sequence_ok, || format!("{seen:?}"));
    let filtered = r.stderr.lines().chain(r.stdout.lines()).find_map(|l| l.split("SELFTEST filtered=").nth(1)).unwrap_or("").trim().to_string();
    let (shown, total) = filtered.split_once('/').map(|(a, b)| (a.parse::<u32>().unwrap_or(99), b.trim_matches('"').parse::<u32>().unwrap_or(0))).unwrap_or((99, 0));
    k.check("the history filter ran in Rust and narrowed the list (steam only)", total >= 2 && shown >= 1 && shown < total, || format!("filtered={filtered}"));
    k.check("clearing history through the UI emptied the daemon's history", c.history(10).map(|h| h.is_empty()).unwrap_or(false), || "history not empty".into());
    k.check("at exit the panel still reports connected", rep["state"] == "connected", || rep.to_string());
    k.check("the QML produced no errors or warnings that indicate a broken UI", r.qml_errors().is_empty(), || r.qml_errors().join("\n"));

    println!("\n{} passed, {} failed", k.passed, k.failed.len());
    assert!(k.failed.is_empty(), "failures:\n{}", k.failed.join("\n"));
}

#[test]
fn without_a_daemon_the_panel_says_so_and_does_not_crash() {
    let nowhere = std::env::temp_dir().join(format!("notify-panel-nowhere-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&nowhere);
    let r = run_panel(&nowhere, &["--quit-after=3", "--report"]);
    let rep = r.report();
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(rep["state"], "no-daemon", "{rep}");
    assert_eq!(rep["connected"], false);
    assert!(rep["state_message"].as_str().unwrap_or("").contains("systemctl --user start notifd"), "{rep}");
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    let ui = r.ui();
    assert!(ui["banner"].as_str().unwrap_or("").contains("notifd is not running"), "the banner must be VISIBLE, ui = {ui}");
    assert_eq!(ui["subtitle"], "Not connected", "{ui}");
    assert_eq!(ui["zones"], 0, "{ui}");
}

#[test]
fn an_incompatible_daemon_is_refused_with_advice() {
    let (path, _) = fake_daemon("panel-incompat", |_, s| serve(s, 999, None));
    let r = run_panel(&path, &["--quit-after=3", "--report"]);
    let rep = r.report();
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(rep["state"], "incompatible", "{rep}");
    assert!(rep["state_message"].as_str().unwrap_or("").contains("notify update"), "{rep}");
    assert_eq!(rep["zones"], 0, "no data may be shown from an incompatible daemon");
    let ui = r.ui();
    assert!(ui["banner"].as_str().unwrap_or("").contains("notify update"), "the advice must be VISIBLE, ui = {ui}");
}

#[test]
fn the_panel_reconnects_after_the_daemon_hangs_up() {
    let (path, count) = fake_daemon("panel-flaky", |n, s| serve(s, 1, if n == 1 { Some(4) } else { None }));
    let r = run_panel(&path, &["--quit-after=6", "--report"]);
    let rep = r.report();
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(*count.lock().unwrap() >= 2, "the panel must reconnect");
    assert!(rep["connections"].as_u64().unwrap_or(0) >= 2, "{rep}");
    assert_eq!(rep["state"], "connected", "{rep}");
}

#[test]
fn the_history_page_opens_directly() {
    let d = TestDaemon::start(test_config());
    let r = run_panel(&d.sock, &["--page=history", "--quit-after=3", "--report"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(r.report()["state"], "connected");
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    let ui = r.ui();
    assert_eq!(ui["page"], "history", "{ui}");
    assert!(ui["summary"].as_str().unwrap_or("").starts_with("Showing "), "the History page must display its count, ui = {ui}");
    let _ = Client::connect(&d.sock).unwrap();
    let _ = std::io::stdout().flush();
}

