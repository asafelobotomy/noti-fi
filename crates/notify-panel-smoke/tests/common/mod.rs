//! Shared helpers for the panel's headless tests: build and locate the real binary, run it offscreen against a daemon, and read what it printed.
#![allow(dead_code)]

use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

/// Build (a no-op when up to date) and locate the real panel binary.
pub fn panel_binary() -> &'static std::path::Path {
    use std::sync::OnceLock;
    static BIN: OnceLock<std::path::PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let st = Command::new(cargo).args(["build", "-p", "notify-panel"]).current_dir(&root).status().expect("run cargo");
        assert!(st.success(), "building notify-panel failed");
        let target = std::env::var_os("CARGO_TARGET_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("target"));
        target.join("debug/notify-panel")
    })
}

pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn report(&self) -> Value {
        for l in self.stdout.lines() {
            if let Ok(v) = serde_json::from_str::<Value>(l) {
                if let Some(r) = v.get("report") {
                    return r.clone();
                }
            }
        }
        Value::Null
    }
    /// What the UI was actually displaying when it quit (the `UI {...}` line the app prints with --report).
    pub fn ui(&self) -> Value {
        self.stderr
            .lines()
            .chain(self.stdout.lines())
            .find_map(|l| l.split("UI ").nth(1).and_then(|j| serde_json::from_str::<Value>(j.trim()).ok()))
            .unwrap_or(Value::Null)
    }
    /// Any QML warning or error (they carry a `qrc:/...` source location), except one known Kirigami quirk:
    /// its page row creates pages without a parent for a moment and Qt says so; the pages display correctly.
    pub fn qml_errors(&self) -> Vec<String> {
        self.stderr
            .lines()
            .filter(|l| l.contains("qrc:/") || l.contains("TypeError") || l.contains("ReferenceError"))
            .filter(|l| !l.contains("Created graphical object was not placed in the graphics scene"))
            .map(String::from)
            .collect()
    }
}

pub fn run_panel(socket: &std::path::Path, args: &[&str]) -> Run {
    run_panel_env(socket, args, &[])
}

pub fn run_panel_env(socket: &std::path::Path, args: &[&str], env: &[(&str, &std::path::Path)]) -> Run {
    let settings = std::env::temp_dir().join(format!("notify-panel-settings-{}-{}", std::process::id(), Instant::now().elapsed().as_nanos()));
    std::fs::create_dir_all(&settings).unwrap();
    let o = Command::new(panel_binary())
        .args(args)
        .env("QT_QPA_PLATFORM", "offscreen")
        // Qt logs to the systemd journal when it can; force stderr so the test can read QML warnings and console.log
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .env("QT_LOGGING_RULES", "qml.debug=true")
        .env("QT_NO_XDG_DESKTOP_PORTAL", "1")
        .env_remove("JOURNAL_STREAM")
        .env("NOTIFD_SOCKET", socket)
        .env("NOTIFY_PANEL_SOCKET", std::env::temp_dir().join(format!("notify-panel-{}-{}.sock", std::process::id(), Instant::now().elapsed().as_nanos())))
        .env("NOTIFY_PANEL_NO_TRAY", "1")
        .env("NOTIFY_PANEL_TEST", "1")
        .envs(env.iter().copied())
        .env("XDG_CONFIG_HOME", &settings)
        .env("XDG_DATA_HOME", &settings)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run notify-panel");
    // A panel whose QML fails to load keeps running with no window; never let a test hang on it.
    let pid = o.id();
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = done.clone();
    thread::spawn(move || {
        let start = Instant::now();
        while !flag.load(std::sync::atomic::Ordering::SeqCst) {
            if start.elapsed() > Duration::from_secs(90) {
                let _ = Command::new("kill").arg(pid.to_string()).status();
                return;
            }
            thread::sleep(Duration::from_millis(200));
        }
    });
    let o = o.wait_with_output().expect("wait for notify-panel");
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    Run { code: o.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&o.stdout).into_owned(), stderr: String::from_utf8_lossy(&o.stderr).into_owned() }
}

