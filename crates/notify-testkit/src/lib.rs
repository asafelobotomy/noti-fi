//! Shared test harness: start a throwaway notifd on a private D-Bus session and drive it.
//! Used by the client contract tests and the CLI integration tests. Never touches your real notifications.

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use notify_client::Client;
use notify_protocol::*;

/// The project checkout (two levels above this crate).
pub fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

pub fn wait_until(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if f() {
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    f()
}

pub struct TestDaemon {
    pub dir: PathBuf,
    child: Child,
    pub sock: PathBuf,
    pub bus: String,
    pub config: Config,
}

impl TestDaemon {
    pub fn start(config: Config) -> TestDaemon {
        let root = project_root();
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("notifd-rust-{}-{nanos}", std::process::id()));
        let payload = dir.join("p");
        let state = dir.join("state");
        fs::create_dir_all(&payload).unwrap();
        fs::create_dir_all(&state).unwrap();
        // every QML file of the daemon (the folder is the payload; new files need no list here)
        for e in fs::read_dir(root.join("notifd")).unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "qml")) {
            fs::copy(e.path(), payload.join(e.file_name())).unwrap();
        }
        fs::write(payload.join("rules.json"), config.to_json_pretty()).unwrap();
        let sock = dir.join("control.sock");
        let busfile = dir.join("bus");
        let log = fs::File::create(dir.join("log")).unwrap();
        let child = Command::new("dbus-run-session")
            .args(["--", "bash", "-c"])
            .arg(format!("echo \"$DBUS_SESSION_BUS_ADDRESS\" > {}; exec qs -p {}", busfile.display(), payload.display()))
            .env("QT_NO_XDG_DESKTOP_PORTAL", "1")
            .env("XDG_STATE_HOME", &state)
            .env("NOTIFD_SOCKET", &sock)
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .process_group(0)
            .spawn()
            .expect("dbus-run-session must be installed");
        let end = Instant::now() + Duration::from_secs(45);
        loop {
            if let Ok(mut c) = Client::connect(&sock) {
                if c.ping().is_ok() {
                    break;
                }
            }
            assert!(Instant::now() < end, "daemon did not start; log:\n{}", fs::read_to_string(dir.join("log")).unwrap_or_default());
            thread::sleep(Duration::from_millis(300));
        }
        let bus = fs::read_to_string(&busfile).unwrap().trim().to_string();
        TestDaemon { dir, child, sock, bus, config }
    }

    /// The rules.json the daemon reads.
    pub fn rules_path(&self) -> PathBuf {
        self.dir.join("p/rules.json")
    }

    pub fn client(&self) -> Client {
        Client::connect(&self.sock).expect("connect")
    }

    pub fn write_config(&self, c: &Config) {
        fs::write(self.dir.join("p/rules.json"), c.to_json_pretty()).unwrap();
    }

    /// Send a real notification through the private D-Bus session.
    pub fn send(&self, summary: &str, app: &str, timeout_ms: i64, hints: &[&str], urgency: &str) -> String {
        let mut cmd = Command::new("notify-send");
        cmd.env("DBUS_SESSION_BUS_ADDRESS", &self.bus).args(["-p", "-a", app, "-u", urgency, "-t", &timeout_ms.to_string()]);
        for h in hints {
            cmd.args(["-h", h]);
        }
        let out = cmd.args([summary, "body text"]).output().expect("notify-send must be installed");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        let pgid = self.child.id();
        let _ = Command::new("kill").args(["-TERM", &format!("-{pgid}")]).stderr(Stdio::null()).status();
        let end = Instant::now() + Duration::from_secs(8);
        while Instant::now() < end {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let _ = Command::new("kill").args(["-KILL", &format!("-{pgid}")]).stderr(Stdio::null()).status();
        let _ = self.child.wait();
        let _ = &self.dir;
    }
}

// ------------------------------------------------------------------ tiny check collector

#[derive(Default)]
pub struct Checks {
    pub passed: usize,
    pub failed: Vec<String>,
}
impl Checks {
    pub fn check(&mut self, name: &str, ok: bool, detail: impl FnOnce() -> String) {
        if ok {
            self.passed += 1;
            println!("  [PASS] {name}");
        } else {
            let d = detail();
            println!("  [FAIL] {name}  ({d})");
            self.failed.push(format!("{name}: {d}"));
        }
    }
    /// Print the tally and fail the test if any check failed.
    pub fn finish(self) {
        println!("\n{} passed, {} failed", self.passed, self.failed.len());
        assert!(self.failed.is_empty(), "failures:\n{}", self.failed.join("\n"));
    }
}


use std::collections::BTreeMap;

/// The config every test daemon starts with (three zones, three rules, short timeouts, watchers off).
pub fn test_config() -> Config {
    let mut zones = BTreeMap::new();
    zones.insert("business".to_string(), Zone { accent: Some("#c084fc".into()), label: Some("Business".into()), ..Zone::at(Anchor::TopLeft) });
    zones.insert("personal".to_string(), Zone { accent: Some("#60a5fa".into()), label: Some("Personal".into()), ..Zone::at(Anchor::TopRight) });
    zones.insert("system".to_string(), Zone { accent: Some("#fbbf24".into()), label: Some("System".into()), ..Zone::at(Anchor::BottomRight) });
    let rule = |name: &str, field: &str, pat: &str, zone: Option<&str>, timeout: Option<f64>, silent: Option<bool>| Rule {
        name: Some(name.into()),
        matches: BTreeMap::from([(field.to_string(), pat.to_string())]),
        zone: zone.map(Into::into),
        timeout,
        silent,
        ..Default::default()
    };
    Config {
        default_timeout: 1500.0,
        max_visible: 3,
        history: HistorySettings { limit: 50, persist: false },
        auto_dnd: AutoDndSettings { fullscreen: true, screen_share: true, watch: Some(false), ignore_capture_by: vec![] },
        zones,
        rules: vec![
            rule("business sites", "origin", "example-shop|example-chat", Some("business"), None, None),
            rule("system", "appName", "^kernel$", Some("system"), None, None),
            rule("quiet app", "appName", "^quietapp$", None, None, Some(true)),
        ],
        ..Config::default()
    }
}


/// A scripted fake notifd, for testing how clients behave when the daemon misbehaves.
pub mod fake {
    use std::collections::BTreeMap;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::thread;

    use notify_protocol::{Auto, Config, Status};
    use serde_json::{Value, json};

    pub fn status_json() -> Value {
        let s = Status {
            dnd: false,
            muted: vec![],
            zones: vec!["personal".into()],
            active: 0,
            queued: 0,
            per_zone: BTreeMap::new(),
            cap: 5,
            history: 0,
            cfg_error: String::new(),
            auto: Auto::default(),
            holding: String::new(),
            previewing: false,
            focus: String::new(),
            stacks: Vec::new(),
            unread: 0,
            unread_per_zone: Default::default(),
            center: false,
            kept: 0,
            inhibitors: vec![],
            sharing: false,
            sharing_by: vec![],
            latest: None,
            screens: Default::default(),
        };
        serde_json::to_value(s).unwrap()
    }

    /// Answer requests like a (minimal) notifd until `max_requests` have been served, then hang up.
    pub fn serve(mut stream: UnixStream, protocol: u32, max_requests: Option<usize>) {
        let hello = json!({"event": "hello", "protocol": protocol, "daemon": "fake-1", "pid": 1, "seq": 0});
        let _ = writeln!(stream, "{hello}");
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut served = 0;
        let mut line = String::new();
        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            let Ok(req) = serde_json::from_str::<Value>(&line) else { continue };
            let result = match req["method"].as_str().unwrap_or("") {
                "subscribe" => json!({"status": status_json(), "seq": 0}),
                "status" | "dnd" | "mute" | "clearHistory" => status_json(),
                "configGet" => json!({"raw": "{}", "effective": serde_json::to_value(Config::default()).unwrap(), "error": ""}),
                "history" => json!([]),
                _ => json!({}),
            };
            let _ = writeln!(stream, "{}", json!({"id": req["id"], "result": result}));
            served += 1;
            if max_requests.map(|m| served >= m).unwrap_or(false) {
                return; // hang up: the worker must notice and reconnect
            }
        }
    }

    pub fn fake_daemon(name: &str, handler: impl Fn(usize, UnixStream) + Send + 'static) -> (PathBuf, Arc<Mutex<usize>>) {
        let path = std::env::temp_dir().join(format!("notify-panel-fake-{}-{name}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        let count = Arc::new(Mutex::new(0usize));
        let c2 = count.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let n = {
                    let mut g = c2.lock().unwrap();
                    *g += 1;
                    *g
                };
                handler(n, stream);
            }
        });
        (path, count)
    }

}
