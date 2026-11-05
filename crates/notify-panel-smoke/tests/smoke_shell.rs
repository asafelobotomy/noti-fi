//! Headless tests of how the panel fits the desktop session: one panel at a time, start at login.

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
fn a_second_launch_brings_the_first_panel_forward_instead_of_opening_another() {
    let d = TestDaemon::start(test_config());
    let sock = std::env::temp_dir().join(format!("notify-panel-single-{}.sock", std::process::id()));
    let settings = std::env::temp_dir().join(format!("notify-panel-single-settings-{}", std::process::id()));
    std::fs::create_dir_all(&settings).unwrap();
    let launch = |args: &[&str]| {
        Command::new(panel_binary())
            .args(args)
            .env("QT_QPA_PLATFORM", "offscreen")
            .env("QT_FORCE_STDERR_LOGGING", "1")
            .env("QT_LOGGING_RULES", "qml.debug=true")
            .env("QT_NO_XDG_DESKTOP_PORTAL", "1")
            .env_remove("JOURNAL_STREAM")
            .env("NOTIFD_SOCKET", &d.sock)
            .env("NOTIFY_PANEL_NO_TRAY", "1")
            .env("NOTIFY_PANEL_TEST", "1")
            .env("NOTIFY_PANEL_SOCKET", &sock)
            .env("XDG_CONFIG_HOME", &settings)
            .env("XDG_DATA_HOME", &settings)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start the panel")
    };
    let first = launch(&["--quit-after=14"]);
    assert!(wait_until(Duration::from_secs(10), || sock.exists()), "the first panel must start listening");
    thread::sleep(Duration::from_secs(2));
    let started = Instant::now();
    let second = launch(&["--quit-after=14"]).wait_with_output().unwrap();
    assert!(second.status.success(), "the second launch exits cleanly: {}", String::from_utf8_lossy(&second.stderr));
    assert!(started.elapsed() < Duration::from_secs(8), "the second launch must not start a whole second panel ({:?})", started.elapsed());
    let out = first.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    assert!(err.contains("SHOW requested by another launch"), "the first panel must be asked to show itself:\n{err}");
    // a leftover socket from a crash does not stop a later start
    let again = run_panel_env(&d.sock, &["--quit-after=2"], &[("NOTIFY_PANEL_SOCKET", &sock)]);
    assert_eq!(again.code, 0, "{}", again.stderr);
}

#[test]
fn start_at_login_can_be_turned_on_and_off_and_the_background_flag_is_accepted() {
    let d = TestDaemon::start(test_config());
    let r = run_panel(&d.sock, &["--selftest-autostart", "--background"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    assert!(r.stderr.contains("SELFTEST-AUTOSTART on=true"), "{}", r.stderr);
    assert!(r.stderr.contains("SELFTEST-AUTOSTART off=true failures=0"), "{}", r.stderr);
    let r = run_panel(&d.sock, &["--page=general", "--quit-after=3", "--report"]);
    let ui = r.ui();
    assert_eq!((ui["page"].as_str(), ui["trayAvailable"].as_bool(), ui["closeToTray"].as_bool()), (Some("general"), Some(false), Some(true)), "without a tray the options are shown as unavailable: {ui}");
    assert_eq!(ui["version"], env!("CARGO_PKG_VERSION"), "the page shows the installed version: {ui}");
    assert_eq!((ui["updateState"].as_str(), ui["autoCheck"].as_bool()), (Some("idle"), Some(false)), "nothing is checked, and the daily check is off, until the user asks: {ui}");
}


#[test]
fn the_selftest_hooks_refuse_to_touch_real_settings() {
    let d = TestDaemon::start(test_config());
    let settings = std::env::temp_dir().join(format!("notify-panel-refuse-{}", std::process::id()));
    std::fs::create_dir_all(&settings).unwrap();
    // no NOTIFY_PANEL_TEST: this is what running the hook by accident would look like (the tray stays off so the real one is not touched)
    let out = Command::new(panel_binary())
        .args(["--selftest", "--quit-after=20"])
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .env("QT_LOGGING_RULES", "qml.debug=true")
        .env("QT_NO_XDG_DESKTOP_PORTAL", "1")
        .env_remove("JOURNAL_STREAM")
        .env_remove("NOTIFY_PANEL_TEST")
        .env("NOTIFY_PANEL_NO_TRAY", "1")
        .env("NOTIFY_PANEL_SOCKET", settings.join("p.sock"))
        .env("NOTIFD_SOCKET", &d.sock)
        .env("XDG_CONFIG_HOME", &settings)
        .env("XDG_DATA_HOME", &settings)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(err.contains("SELFTEST refused"), "the hook must refuse outside test mode:\n{err}");
    assert!(!err.contains("SELFTEST failures="), "it must not have run:\n{err}");
    // and the daemon's state was not touched (the hook would have switched Do Not Disturb on and off and cleared history)
    let mut c = d.client();
    assert!(!c.status().unwrap().dnd);
}

#[test]
fn a_background_start_shows_the_window_when_no_tray_appears() {
    let d = TestDaemon::start(test_config());
    // started at login (--background) on a desktop without a tray: after a few seconds the window must appear rather than leave an invisible app
    let r = run_panel(&d.sock, &["--background", "--quit-after=9", "--report"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    assert_eq!(r.ui()["page"], "overview", "the window and its first page must have been shown: {}", r.ui());
}

#[test]
fn the_system_page_runs_the_health_checks_and_gates_every_action_behind_a_plan() {
    let d = TestDaemon::start(test_config());
    let r = run_panel(&d.sock, &["--page=system", "--quit-after=6", "--report"]);
    assert!(r.qml_errors().is_empty(), "{:?}", r.qml_errors());
    let ui = r.ui();
    assert_eq!(ui["page"], "system", "{ui}");
    assert!(ui["checks"].as_u64().unwrap_or(0) > 0, "the health checks are listed: {ui}");
    assert_eq!(ui["busy"], "", "nothing is still running at the end: {ui}");
    assert_eq!(ui["planned"], "", "no action is armed until its plan was shown: {ui}");
}
