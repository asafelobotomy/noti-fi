//! Headless tests of the editor pages (rules, zones, appearance, apps, look and behaviour, focus): edit in the real UI, save, and check what the daemon accepted.

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
fn the_rules_page_shows_the_real_rules_and_validates_them() {
    let d = TestDaemon::start(test_config());
    let rules = d.rules_path();
    let r = run_panel_env(&d.sock, &["--page=rules", "--quit-after=4", "--report"], &[("NOTIFD_RULES", &rules)]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    let ui = r.ui();
    assert_eq!(ui["page"], "rules", "{ui}");
    assert_eq!(ui["rows"], 3, "the page must list the 3 rules of the daemon's config: {ui}");
    assert_eq!(ui["dirty"], false, "{ui}");
    assert_eq!(ui["valid"], true, "{ui}");
    assert_eq!(ui["banner"], "", "{ui}");
}

#[test]
fn the_rules_page_edits_simulates_and_saves_through_the_real_ui() {
    let d = TestDaemon::start(test_config());
    let rules = d.rules_path();
    let before = std::fs::read_to_string(&rules).unwrap();
    // without --selftest-save nothing may reach the disk, whatever the page does
    let r = run_panel_env(&d.sock, &["--page=rules", "--selftest-rules", "--report"], &[("NOTIFD_RULES", &rules)]);
    let ui = r.ui();
    assert_eq!(ui["simZoneBy"].as_str().map(|s| !s.is_empty()), Some(true), "the simulator must display which rule decided the zone: {ui}");
    assert_eq!(ui["valid"], true, "{ui}");
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    assert!(r.stderr.contains("SELFTEST-RULES failures=0"), "a rules invokable is missing or failed:\n{}", r.stderr);
    assert_eq!(std::fs::read_to_string(&rules).unwrap(), before, "an unsaved draft must never change rules.json");

    let r = run_panel_env(&d.sock, &["--page=rules", "--selftest-rules", "--selftest-save"], &[("NOTIFD_RULES", &rules)]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stderr.contains("SELFTEST-RULES failures=0"), "{}", r.stderr);
    let after: Value = serde_json::from_str(&std::fs::read_to_string(&rules).unwrap()).unwrap();
    let names: Vec<&str> = after["rules"].as_array().unwrap().iter().filter_map(|x| x["name"].as_str()).collect();
    assert_eq!(names.iter().filter(|n| n.starts_with("Selftest rule")).count(), 2, "the added rule and its duplicate were saved: {names:?}");
    assert!(rules.with_extension("json.bak").exists() || std::fs::read_dir(rules.parent().unwrap()).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains(".bak")), "saving must keep a backup");
    // the daemon accepted and applied the file
    let mut c = d.client();
    assert!(c.status().is_ok());
}

#[test]
fn the_zones_and_appearance_pages_show_the_real_settings() {
    let d = TestDaemon::start(test_config());
    let rules = d.rules_path();
    let r = run_panel_env(&d.sock, &["--page=zones", "--quit-after=4", "--report"], &[("NOTIFD_RULES", &rules)]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    let ui = r.ui();
    assert_eq!(ui["page"], "zones", "{ui}");
    assert_eq!(ui["zones"], 3, "{ui}");
    assert_eq!(ui["valid"], true, "{ui}");
    assert_eq!(ui["maxVisible"], 3, "the test config has maxVisible 3: {ui}");
    let corners = [&ui["topLeft"], &ui["topRight"], &ui["bottomLeft"], &ui["bottomRight"]];
    assert!(corners.iter().all(|c| c.is_string()) && corners.iter().filter(|c| !c.as_str().unwrap().is_empty()).count() >= 2, "zones must be drawn in their corners: {ui}");

    let r = run_panel_env(&d.sock, &["--page=appearance", "--quit-after=4", "--report"], &[("NOTIFD_RULES", &rules)]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    let ui = r.ui();
    assert_eq!(ui["page"], "appearance", "{ui}");
    assert!(ui["card"].as_str().unwrap_or("").starts_with('#'), "the card colour must be shown: {ui}");
    assert_eq!(ui["valid"], true, "{ui}");
}

#[test]
fn zones_and_appearance_edits_reach_the_daemon_only_when_saved() {
    let d = TestDaemon::start(test_config());
    let rules = d.rules_path();
    let before = std::fs::read_to_string(&rules).unwrap();
    let r = run_panel_env(&d.sock, &["--selftest-zones"], &[("NOTIFD_RULES", &rules)]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    assert!(r.stderr.contains("SELFTEST-ZONES failures=0 toasts=2"), "two refusals were expected and no missing command:\n{}", r.stderr);
    assert_eq!(std::fs::read_to_string(&rules).unwrap(), before, "an unsaved draft must never change rules.json");

    let r = run_panel_env(&d.sock, &["--selftest-zones", "--selftest-save"], &[("NOTIFD_RULES", &rules)]);
    assert!(r.stderr.contains("SELFTEST-ZONES failures=0 toasts=2"), "{}", r.stderr);
    let after: Value = serde_json::from_str(&std::fs::read_to_string(&rules).unwrap()).unwrap();
    assert_eq!(after["theme"]["card"], "#101010", "{after}");
    assert_eq!(after["maxVisible"], 4);
    assert_eq!(after["sound"]["enabled"], false);
    let z = after["zones"].as_object().unwrap().values().find(|z| z["label"] == "Selftest zone").expect("the new zone was saved");
    assert_eq!((z["anchor"].as_str(), z["x"].as_f64(), z["y"].as_f64()), (Some("free"), Some(0.25), Some(0.75)), "a dragged zone is saved as a free position");
    assert_eq!(z["accent"], "#ff8800");
    // the running daemon accepted and applied it
    let mut c = d.client();
    assert!(wait_until(Duration::from_secs(5), || c.config_get().map(|g| g.raw.contains("Selftest zone")).unwrap_or(false)), "the daemon must serve the saved config");
}

/// A folder of fake launchers: one messaging app that never sends, and one that is not a notifying app at all.
fn fake_launchers() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("notify-smoke-launchers-{}-{}", std::process::id(), Instant::now().elapsed().as_nanos()));
    let apps = root.join("applications");
    std::fs::create_dir_all(&apps).unwrap();
    std::fs::write(apps.join("quietchat.desktop"), "[Desktop Entry]\nType=Application\nName=Quiet Chat\nCategories=Network;InstantMessaging;\n").unwrap();
    std::fs::write(apps.join("calculator.desktop"), "[Desktop Entry]\nType=Application\nName=Calculator\nCategories=Utility;\n").unwrap();
    root
}

fn isolated_apps<'a>(launchers: &'a std::path::Path) -> Vec<(&'static str, &'a std::path::Path)> {
    vec![("XDG_DATA_DIRS", launchers), ("NOTIFY_APPS_ISOLATED", std::path::Path::new("1"))]
}

#[test]
fn the_apps_page_lists_senders_and_apps_that_could_send() {
    let d = TestDaemon::start(test_config());
    let rules = d.rules_path();
    let launchers = fake_launchers();
    d.send("hi", "Smokeapp", 300, &[], "normal");
    let mut c = d.client();
    wait_until(Duration::from_secs(5), || c.seen_apps().map(|a| a.iter().any(|x| x.app == "Smokeapp")).unwrap_or(false));
    let mut env = isolated_apps(&launchers);
    env.push(("NOTIFD_RULES", &rules));
    let r = run_panel_env(&d.sock, &["--page=apps", "--quit-after=5", "--report"], &env);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    let ui = r.ui();
    assert_eq!(ui["page"], "apps", "{ui}");
    assert_eq!(ui["sending"], 1, "{ui}");
    assert_eq!(ui["silent"], 1, "only the messaging app counts as able to send; the calculator does not: {ui}");
    let names: Vec<&str> = ui["names"].as_array().unwrap().iter().filter_map(|n| n.as_str()).collect();
    assert_eq!(names, vec!["Smokeapp", "Quiet Chat"], "senders first, then the silent app: {ui}");
}

#[test]
fn per_app_changes_become_rules_only_when_saved() {
    let d = TestDaemon::start(test_config());
    let rules = d.rules_path();
    let launchers = fake_launchers();
    d.send("hi", "Appstest", 300, &[], "normal");
    let mut c = d.client();
    wait_until(Duration::from_secs(5), || c.seen_apps().map(|a| a.iter().any(|x| x.app == "Appstest")).unwrap_or(false));
    let mut env = isolated_apps(&launchers);
    env.push(("NOTIFD_RULES", &rules));
    let before = std::fs::read_to_string(&rules).unwrap();

    let r = run_panel_env(&d.sock, &["--selftest-apps"], &env);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    assert!(r.stderr.contains("SELFTEST-APPS failures=0 toasts=3"), "three refusals were expected:\n{}", r.stderr);
    assert_eq!(std::fs::read_to_string(&rules).unwrap(), before, "unsaved per-app changes must not touch rules.json");

    let r = run_panel_env(&d.sock, &["--selftest-apps", "--selftest-save"], &env);
    assert!(r.stderr.contains("SELFTEST-APPS failures=0 toasts=3"), "{}", r.stderr);
    let after: Value = serde_json::from_str(&std::fs::read_to_string(&rules).unwrap()).unwrap();
    let first = &after["rules"][0];
    assert_eq!(first["name"], "App: Appstest", "{after}");
    assert_eq!(first["match"]["appName"], "^Appstest$");
    assert_eq!(first["silent"], true);
    assert_eq!(first["zone"], "business");
    assert_eq!(first["timeout"], 2500);

    // and the daemon really applies it: the app's notification is now held as silent
    let mut c = d.client();
    wait_until(Duration::from_secs(5), || c.config_get().map(|g| g.raw.contains("App: Appstest")).unwrap_or(false));
    d.send("after the rule", "Appstest", 300, &[], "normal");
    let held = wait_until(Duration::from_secs(5), || c.history(5).map(|h| h.iter().any(|x| x.summary == "after the rule" && !x.shown)).unwrap_or(false));
    assert!(held, "the muted app's notification must not be shown: {:?}", c.history(5));
}

#[test]
fn look_and_behavior_edits_are_saved_and_accepted_by_the_daemon() {
    let d = TestDaemon::start(test_config());
    let rules = d.rules_path();
    let before = std::fs::read_to_string(&rules).unwrap();
    let r = run_panel_env(&d.sock, &["--selftest-style"], &[("NOTIFD_RULES", &rules)]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    assert!(r.stderr.contains("SELFTEST-STYLE failures=0 toasts=3 dirty=true valid=true"), "three refusals were expected and the draft must stay valid:\n{}", r.stderr);
    assert_eq!(std::fs::read_to_string(&rules).unwrap(), before, "unsaved edits must not touch rules.json");

    let r = run_panel_env(&d.sock, &["--selftest-style", "--selftest-save"], &[("NOTIFD_RULES", &rules)]);
    assert!(r.stderr.contains("SELFTEST-STYLE failures=0 toasts=3"), "{}", r.stderr);
    let after: Value = serde_json::from_str(&std::fs::read_to_string(&rules).unwrap()).unwrap();
    let names: Vec<&str> = after["rules"].as_array().unwrap().iter().filter_map(|r| r["name"].as_str()).collect();
    assert_eq!(&names[..2], &["App: Styleapp mail", "App: Styleapp"], "the kind's rule goes before the app's, both before the user's rules: {names:?}");
    let user = after["rules"].as_array().unwrap().iter().find(|r| r["name"] == "business sites").expect("the user's rule is still there");
    assert_eq!((user["opacity"].as_f64(), user["clickThrough"].as_bool(), user["noFocus"].as_bool(), user["width"].as_i64()), (Some(0.6), Some(true), Some(false), Some(480)), "{user}");
    assert_eq!((user["fontFamily"].as_str(), user["group"].as_str()), (Some("Serif"), Some("Sites")));
    assert_eq!(after["zones"]["personal"]["opacity"], 0.5);
    assert_eq!((after["theme"]["opacity"].as_f64(), after["theme"]["fontSize"].as_i64()), (Some(0.9), Some(14)));
    let kind = &after["rules"][0];
    assert_eq!(kind["match"]["category"], "^email\\.arrived$");
    assert_eq!(kind["group"], "Styleapp mail");
    assert_eq!(after["rules"][1]["fontSize"], 16);

    // the running daemon accepts the file and the typed client reads it back (new keys included)
    let mut c = d.client();
    assert!(wait_until(Duration::from_secs(5), || c.config_get().map(|g| g.raw.contains("Styleapp mail") && g.error.is_empty()).unwrap_or(false)), "the daemon must load the saved config");
    let g = c.config_get().unwrap();
    assert_eq!(g.effective.theme.font_size, 14.0);
    assert_eq!(g.effective.zones["personal"].opacity, Some(0.5));
    let sim = c.call_raw("simulate", serde_json::json!({"notification": {"appName": "Styleapp", "category": "email.arrived"}, "ignoreState": true})).unwrap();
    assert_eq!(sim["style"]["group"], "Styleapp mail", "{sim}");
    assert_eq!(sim["style"]["fontSize"], 16, "the app-level rule supplies what the kind's rule does not: {sim}");
}

#[test]
fn focus_modes_priority_stack_and_media_settings_work_end_to_end() {
    let d = TestDaemon::start(test_config());
    let rules = d.rules_path();
    // the Overview shows the focus choice
    let r = run_panel_env(&d.sock, &["--quit-after=3", "--report"], &[("NOTIFD_RULES", &rules)]);
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    let ui = r.ui();
    assert_eq!(ui["page"], "overview", "{ui}");
    assert!(ui["focusModes"].as_u64().unwrap_or(0) >= 1, "the default 'Urgent only' mode must be offered: {ui}");
    assert_eq!(ui["focus"], "", "{ui}");

    let before = std::fs::read_to_string(&rules).unwrap();
    let r = run_panel_env(&d.sock, &["--selftest-focus"], &[("NOTIFD_RULES", &rules)]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    assert!(r.stderr.contains("SELFTEST-FOCUS failures=0 toasts=3 dirty=true valid=true"), "three refusals were expected and the draft must stay valid:\n{}", r.stderr);
    assert_eq!(std::fs::read_to_string(&rules).unwrap(), before, "unsaved edits must not touch rules.json");
    // turning a focus mode on is immediate, not part of the draft
    let mut c = d.client();
    assert_eq!(c.status().unwrap().focus, "urgent", "the panel's focus switch reached the daemon");
    c.focus("off").unwrap();

    let r = run_panel_env(&d.sock, &["--selftest-focus", "--selftest-save"], &[("NOTIFD_RULES", &rules)]);
    assert!(r.stderr.contains("SELFTEST-FOCUS failures=0 toasts=3"), "{}", r.stderr);
    let after: Value = serde_json::from_str(&std::fs::read_to_string(&rules).unwrap()).unwrap();
    let m = &after["focus"]["modes"]["mode2"];
    assert_eq!((m["label"].as_str(), m["minPriority"].as_str(), m["allow"][0]["appName"].as_str()), (Some("Work"), Some("high"), Some("^Signal$")), "{after}");
    assert_eq!(after["transfer"]["chart"], "heatmap");
    let r0 = &after["rules"][0];
    assert_eq!((r0["priority"].as_str(), r0["stack"].as_bool(), r0["media"].as_bool()), (Some("urgent"), Some(true), Some(false)), "{r0}");

    // the daemon accepts it, the typed client parses it, and the new mode behaves
    assert!(wait_until(Duration::from_secs(5), || c.config_get().map(|g| g.raw.contains("mode2") && g.error.is_empty()).unwrap_or(false)), "the daemon must load the saved config");
    let g = c.config_get().unwrap();
    assert_eq!(g.effective.focus.modes["mode2"].min_priority.as_deref(), Some("high"));
    assert_eq!(g.effective.transfer.chart, "heatmap");
    let mut sim = |app: &str| c.call_raw("simulate", serde_json::json!({"notification": {"appName": app}, "ignoreState": true, "focus": "mode2"})).unwrap();
    assert_eq!(sim("Signal")["wouldShow"], true, "an allowed app gets through the mode");
    let other = sim("Somebody");
    assert_eq!((other["wouldShow"].clone(), other["hold"].clone()), (serde_json::json!(false), serde_json::json!("focus mode2")), "{other}");
}

