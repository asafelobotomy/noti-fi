//! `notify serve` end to end: a throwaway session bus (no service files, so nothing is ever activated), notifd from this checkout
//! behind the proxy, and apps talking to it the way real ones do. Needs Quickshell and a Wayland display (CI: tests/ci-daemon.sh).

use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use notify_testkit::Checks;
use serde_json::{Value, json};
use zbus::blocking::{Connection, MessageIterator};
use zbus::zvariant::OwnedValue;

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

struct Setup {
    dir: PathBuf,
    bus: Child,
    address: String,
    serve: Child,
}

impl Drop for Setup {
    fn drop(&mut self) {
        // notify serve and everything it started (Quickshell, its private bus) share one process group
        let _ = Command::new("kill").args(["-TERM", &format!("-{}", self.serve.id())]).status();
        let _ = self.serve.wait();
        let _ = self.bus.kill();
        let _ = self.bus.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn setup() -> Setup {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("notify-serve-{}-{nanos}", std::process::id()));
    let (payload, run, state) = (dir.join("payload"), dir.join("run"), dir.join("state"));
    for d in [&payload, &run, &state] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::set_permissions(&run, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../notifd");
    for e in std::fs::read_dir(&src).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if n.ends_with(".qml") || n == "rules.json" {
            std::fs::copy(e.path(), payload.join(&n)).unwrap();
        }
    }
    let conf = dir.join("session.conf");
    std::fs::write(&conf, format!(
        "<busconfig><type>session</type><listen>unix:path={}</listen><auth>EXTERNAL</auth><policy context=\"default\"><allow send_destination=\"*\" eavesdrop=\"true\"/><allow eavesdrop=\"true\"/><allow own=\"*\"/></policy></busconfig>",
        dir.join("session-bus").display()
    )).unwrap();
    let mut bus = Command::new("dbus-daemon").args(["--nofork", "--nopidfile", "--print-address=1"]).arg(format!("--config-file={}", conf.display())).stdout(Stdio::piped()).spawn().expect("dbus-daemon");
    let mut address = String::new();
    std::io::BufRead::read_line(&mut std::io::BufReader::new(bus.stdout.take().unwrap()), &mut address).unwrap();
    let address = address.trim().to_string();
    // our own runtime folder (for notifd's socket), so the display has to be named by its full path
    let real_run = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    let wayland = std::env::var("WAYLAND_DISPLAY").map(|w| if w.starts_with('/') { w } else { format!("{real_run}/{w}") }).unwrap_or_default();
    let serve = Command::new(env!("CARGO_BIN_EXE_notify"))
        .env("WAYLAND_DISPLAY", wayland)
        .env("QT_QPA_PLATFORM", "wayland")
        .args(["serve", "--payload"])
        .arg(&payload)
        .env("DBUS_SESSION_BUS_ADDRESS", &address)
        .env("XDG_RUNTIME_DIR", &run)
        .env("XDG_STATE_HOME", &state)
        .env("NOTIFD_SOCKET", run.join("notifd.sock"))
        .env("QT_NO_XDG_DESKTOP_PORTAL", "1")
        .process_group(0)
        .spawn()
        .expect("notify serve");
    Setup { dir, bus, address, serve }
}

fn connect(s: &Setup) -> Connection {
    zbus::blocking::connection::Builder::address(s.address.as_str()).unwrap().build().unwrap()
}

fn wait_until(secs: u64, mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    f()
}

fn control(s: &Setup, method: &str, params: Value) -> Value {
    notify_client::Client::connect(s.dir.join("run/notifd.sock")).and_then(|mut c| c.call_raw(method, params)).unwrap_or(Value::Null)
}

fn notify(c: &Connection, summary: &str, actions: &[&str], timeout: i32) -> u32 {
    let hints: HashMap<String, OwnedValue> = HashMap::new();
    let acts: Vec<String> = actions.iter().map(|a| a.to_string()).collect();
    let r = c.call_method(Some(NAME), PATH, Some(NAME), "Notify", &("Steam", 0u32, "", summary, "x", acts, hints, timeout)).expect("Notify");
    r.body().deserialize::<u32>().unwrap()
}

#[test]
fn notify_serve_forwards_everything_and_answers_inhibit() {
    let s = setup();
    let mut k = Checks::default();
    let app = connect(&s);
    let owned = wait_until(30, || app.call_method(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", Some("org.freedesktop.DBus"), "NameHasOwner", &(NAME,)).ok().and_then(|r| r.body().deserialize::<bool>().ok()).unwrap_or(false));
    assert!(owned, "notify serve did not take the notification name");

    let caps: Vec<String> = app.call_method(Some(NAME), PATH, Some(NAME), "GetCapabilities", &()).unwrap().body().deserialize().unwrap();
    k.check("GetCapabilities is Quickshell's, plus inhibitions", caps.contains(&"actions".to_string()) && caps.contains(&"inhibitions".to_string()), || format!("{caps:?}"));
    let info: (String, String, String, String) = app.call_method(Some(NAME), PATH, Some(NAME), "GetServerInformation", &()).unwrap().body().deserialize().unwrap();
    k.check("GetServerInformation is passed through", !info.0.is_empty() && info.3 == "1.2", || format!("{info:?}"));

    // the signals an app gets come from the name's owner, as libnotify and KNotification expect
    let signals = MessageIterator::for_match_rule(zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).interface(NAME).unwrap().build(), &app, Some(32)).unwrap();
    let id = notify(&app, "through the proxy", &["open", "Open"], 0);
    let seen = wait_until(5, || control(&s, "history", json!({"limit": 5})).as_array().is_some_and(|h| h.iter().any(|e| e["id"] == id && e["summary"] == "through the proxy")));
    k.check("a notification sent to the session bus reaches notifd", id > 0 && seen, || control(&s, "history", json!({"limit": 5})).to_string());
    control(&s, "invoke", json!({"id": id, "key": "open"}));
    let next = |it: &mut MessageIterator| -> Option<(String, Vec<u8>)> {
        it.next().and_then(|m| m.ok()).map(|m| (m.header().member().map(|x| x.to_string()).unwrap_or_default(), m.body().data().to_vec()))
    };
    let mut it = signals;
    let mut got = Vec::new();
    for _ in 0..4 {
        match next(&mut it) {
            Some((member, _)) => { got.push(member.clone()); if member == "NotificationClosed" { break } }
            None => break,
        }
    }
    k.check("ActionInvoked and NotificationClosed reach the app through the proxy", got.contains(&"ActionInvoked".into()) && got.contains(&"NotificationClosed".into()), || format!("{got:?}"));

    // Inhibit holds popups until UnInhibit, or until the app leaves the bus
    let presenter = connect(&s);
    let hints: HashMap<String, OwnedValue> = HashMap::new();
    let cookie: u32 = presenter.call_method(Some(NAME), PATH, Some(NAME), "Inhibit", &("org.kde.okular", "presenting", hints.clone())).unwrap().body().deserialize().unwrap();
    let held = wait_until(5, || control(&s, "status", json!({}))["holding"] == "inhibited");
    let st = control(&s, "status", json!({}));
    k.check("Inhibit makes notifd hold popups, and says who asked", cookie > 0 && held && st["inhibitors"] == json!([{"app": "org.kde.okular", "reason": "presenting"}]), || st.to_string());
    let prop = app.call_method(Some(NAME), PATH, Some("org.freedesktop.DBus.Properties"), "Get", &(NAME, "Inhibited")).unwrap();
    let inhibited: bool = prop.body().deserialize::<OwnedValue>().ok().and_then(|v| bool::try_from(v).ok()).unwrap_or(false);
    k.check("the Inhibited property says so", inhibited, String::new);
    let id2 = notify(&app, "while presenting", &[], 3000);
    let h = wait_until(5, || control(&s, "history", json!({"limit": 5})).as_array().is_some_and(|h| h.iter().any(|e| e["id"] == id2)));
    let entry = control(&s, "history", json!({"limit": 5})).as_array().and_then(|h| h.iter().find(|e| e["id"] == id2).cloned()).unwrap_or_default();
    k.check("...a notification meanwhile is held (in history, not on screen)", h && entry["shown"] == false && entry["why"] == "inhibited", || entry.to_string());
    let denied = app.call_method(Some(NAME), PATH, Some(NAME), "UnInhibit", &(cookie,));
    k.check("another program cannot lift someone else's inhibition", denied.is_err(), || format!("{denied:?}"));
    drop(presenter);
    let freed = wait_until(5, || control(&s, "status", json!({}))["holding"] == "");
    k.check("the inhibition ends when its app leaves the bus", freed, || control(&s, "status", json!({})).to_string());
    let again = connect(&s);
    let c2: u32 = again.call_method(Some(NAME), PATH, Some(NAME), "Inhibit", &("", "call", hints)).unwrap().body().deserialize().unwrap();
    wait_until(5, || control(&s, "status", json!({}))["holding"] == "inhibited");
    again.call_method(Some(NAME), PATH, Some(NAME), "UnInhibit", &(c2,)).unwrap();
    k.check("UnInhibit lifts it", wait_until(5, || control(&s, "status", json!({}))["holding"] == ""), || control(&s, "status", json!({})).to_string());
    // a second notifd must not take the running one's control socket
    let second = Command::new(env!("CARGO_BIN_EXE_notify"))
        .args(["serve", "--payload"])
        .arg(s.dir.join("payload"))
        .env("DBUS_SESSION_BUS_ADDRESS", &s.address)
        .env("XDG_RUNTIME_DIR", s.dir.join("run"))
        .env("NOTIFD_SOCKET", s.dir.join("run/notifd.sock"))
        .output()
        .expect("second notify serve");
    let said = String::from_utf8_lossy(&second.stderr).into_owned();
    k.check("a second notify serve refuses while one answers, and the first keeps its socket", !second.status.success() && said.contains("already running") && control(&s, "ping", json!({}))["pong"] == true, || said.clone());
    k.finish();
}
