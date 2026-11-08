//! The bridge end to end on a private D-Bus: a fake app reports jobs the way KIO does, a fake notification
//! server records what the bridge sends. Needs only `dbus-daemon`; never touches the real session.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, Proxy};
use zbus::interface;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

#[derive(Clone, Debug)]
struct Sent {
    app: String,
    replaces: u32,
    id: u32,
    summary: String,
    body: String,
    actions: Vec<String>,
    hints: HashMap<String, OwnedValue>,
    expire: i32,
}

#[derive(Default)]
struct Log {
    sent: Vec<Sent>,
    closed: Vec<u32>,
    next: u32,
}

struct FakeServer(Arc<Mutex<Log>>);

#[interface(name = "org.freedesktop.Notifications")]
impl FakeServer {
    #[allow(clippy::too_many_arguments)]
    fn notify(&self, app: String, replaces: u32, _icon: String, summary: String, body: String, actions: Vec<String>, hints: HashMap<String, OwnedValue>, expire: i32) -> u32 {
        let mut l = self.0.lock().unwrap();
        let id = if replaces != 0 {
            replaces
        } else {
            l.next += 1;
            l.next
        };
        l.sent.push(Sent { app, replaces, id, summary, body, actions, hints, expire });
        id
    }
    fn close_notification(&self, id: u32) {
        self.0.lock().unwrap().closed.push(id);
    }
    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into(), "actions".into()]
    }
}

struct Bus(Child);
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    f()
}

fn job_server(c: &Connection, iface: &'static str) -> Proxy<'static> {
    Proxy::new(c, "org.kde.JobViewServer", "/JobViewServer", iface).unwrap()
}

fn view(c: &Connection, path: &OwnedObjectPath, iface: &'static str) -> Proxy<'static> {
    Proxy::new(c, "org.kde.JobViewServer", path.to_string(), iface).unwrap()
}

fn props(p: Vec<(&str, Value<'static>)>) -> HashMap<String, Value<'static>> {
    p.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

fn hint_str(s: &Sent, k: &str) -> String {
    s.hints.get(k).and_then(|v| <&str>::try_from(&**v).ok()).unwrap_or("").to_string()
}

#[test]
fn bridge_end_to_end() {
    if Command::new("dbus-daemon").arg("--version").output().is_err() {
        eprintln!("dbus-daemon is not installed; skipping");
        return;
    }
    let tmp = std::env::temp_dir().join(format!("notify-jobs-test-{}", std::process::id()));
    let apps = tmp.join("share/applications");
    std::fs::create_dir_all(&apps).unwrap();
    std::fs::write(apps.join("org.kde.dolphin.desktop"), "[Desktop Entry]\nName=Dolphin\nIcon=system-file-manager\n").unwrap();
    let mut child = Command::new("dbus-daemon").args(["--session", "--nofork", "--print-address=1"]).stdout(Stdio::piped()).spawn().unwrap();
    let mut addr = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut addr).unwrap();
    let _bus = Bus(child);
    // SAFETY: set before any other thread of this test binary reads the environment (one test in this file)
    unsafe {
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", addr.trim());
        std::env::set_var("XDG_DATA_HOME", tmp.join("share"));
        std::env::set_var("XDG_DATA_DIRS", tmp.join("none"));
    }

    let log = Arc::new(Mutex::new(Log::default()));
    let server = zbus::blocking::connection::Builder::session().unwrap().name("org.freedesktop.Notifications").unwrap().serve_at("/org/freedesktop/Notifications", FakeServer(log.clone())).unwrap().build().unwrap();
    std::thread::spawn(|| notify_jobs::run().unwrap());
    let app = Connection::session().unwrap();
    let dbus = zbus::blocking::fdo::DBusProxy::new(&app).unwrap();
    assert!(wait(Duration::from_secs(10), || dbus.name_has_owner("org.kde.JobViewServer".try_into().unwrap()).unwrap_or(false)), "bridge did not take the job server");
    assert!(dbus.name_has_owner("org.kde.kuiserver".try_into().unwrap()).unwrap());
    let kui = Proxy::new(&app, "org.kde.kuiserver", "/JobViewServer", "org.kde.kuiserver").unwrap();
    assert!(!kui.call::<_, _, bool>("requiresJobTracker", &()).unwrap(), "KIO must not open its own progress windows");

    // 1. a V3 job (what KIO in KF6 uses): shown at once because it is "immediate", with progress and buttons
    let path: OwnedObjectPath = job_server(&app, "org.kde.JobViewServerV2").call("requestView", &("org.kde.dolphin", 3i32, props(vec![("immediate", Value::from(true))]))).unwrap();
    let job = view(&app, &path, "org.kde.JobViewV3");
    let upd = |p: Vec<(&str, Value<'static>)>| job.call::<_, _, ()>("update", &(props(p),)).unwrap();
    upd(vec![("title", Value::from("Copying")), ("totalBytes", Value::from(1000u64)), ("processedBytes", Value::from(400u64)), ("totalFiles", Value::from(2u64)), ("processedFiles", Value::from(1u64))]);
    assert!(wait(Duration::from_secs(3), || !log.lock().unwrap().sent.is_empty()), "the job was never shown");
    let first = log.lock().unwrap().sent[0].clone();
    assert_eq!(first.app, "Dolphin", "app name comes from the desktop file");
    assert_eq!(first.summary, "Copying");
    assert!(first.body.starts_with("1 of 2 files · 400 B of 1000 B"), "{}", first.body);
    assert_eq!(first.actions, ["suspend", "Pause", "cancel", "Cancel"]);
    assert_eq!(first.expire, 0);
    assert_eq!(hint_str(&first, "desktop-entry"), "org.kde.dolphin");
    assert_eq!(hint_str(&first, "category"), "transfer");
    assert_eq!(first.hints.get("value").and_then(|v| i32::try_from(&**v).ok()), Some(40));

    // 2. a burst of updates is drawn at most four times a second, always into the same notification
    for i in 0..40u64 {
        upd(vec![("processedBytes", Value::from(400 + i * 10))]);
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(400));
    {
        let l = log.lock().unwrap();
        assert!(l.sent.len() <= 6, "updates were not throttled: {} draws", l.sent.len());
        assert!(l.sent.len() >= 2, "updates were not drawn");
        assert!(l.sent.iter().skip(1).all(|s| s.replaces == first.id));
        assert!(l.sent.last().unwrap().body.contains("790 B of 1000 B"), "last update not drawn: {}", l.sent.last().unwrap().body);
    }

    // 3. another program cannot change someone else's job
    let other = Connection::session().unwrap();
    view(&other, &path, "org.kde.JobViewV3").call::<_, _, ()>("update", &(props(vec![("title", Value::from("Hijacked"))]),)).unwrap();
    std::thread::sleep(Duration::from_millis(350));
    assert!(log.lock().unwrap().sent.iter().all(|s| s.summary != "Hijacked"));

    // 4. the popup's Cancel button reaches the app as cancelRequested
    let mut cancels = Proxy::new(&app, "org.kde.JobViewServer", path.to_string(), "org.kde.JobViewV3").unwrap().receive_signal("cancelRequested").unwrap();
    server.emit_signal(None::<&str>, "/org/freedesktop/Notifications", "org.freedesktop.Notifications", "ActionInvoked", &(first.id, "cancel")).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if cancels.next().is_some() {
            let _ = tx.send(());
        }
    });
    assert!(rx.recv_timeout(Duration::from_secs(3)).is_ok(), "cancelRequested was not emitted");

    // 5. when it ends, the same notification turns into the result
    upd(vec![("processedBytes", Value::from(1000u64)), ("processedFiles", Value::from(2u64)), ("destUrl", Value::from("file:///tmp"))]);
    job.call::<_, _, ()>("terminate", &(0u32, "", HashMap::<String, Value>::new())).unwrap();
    assert!(wait(Duration::from_secs(3), || log.lock().unwrap().sent.last().is_some_and(|s| s.summary == "Copied")), "no result notification");
    let last = log.lock().unwrap().sent.last().unwrap().clone();
    assert_eq!(last.replaces, first.id);
    assert_eq!(hint_str(&last, "category"), "transfer.complete");
    assert_eq!(last.actions, ["default", "Open", "open", "Open folder"]);
    assert_eq!(last.expire, -1);
    assert!(wait(Duration::from_secs(2), || job.introspect().is_err()), "the finished job's object was not removed");

    // 6. a quick job that ends within 500 ms is never shown
    let before = log.lock().unwrap().sent.len();
    let quick: OwnedObjectPath = job_server(&app, "org.kde.JobViewServerV2").call("requestView", &("org.kde.dolphin", 1i32, props(vec![]))).unwrap();
    let q = view(&app, &quick, "org.kde.JobViewV3");
    q.call::<_, _, ()>("update", &(props(vec![("title", Value::from("Examining"))]),)).unwrap();
    q.call::<_, _, ()>("terminate", &(0u32, "", HashMap::<String, Value>::new())).unwrap();
    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(log.lock().unwrap().sent.len(), before, "a quick job was shown");

    // 7. an old-style (V1/JobViewV2) app: named from its desktop file, shown after 500 ms
    let v1: OwnedObjectPath = job_server(&other, "org.kde.JobViewServer").call("requestView", &("dolphin", "", 1i32)).unwrap();
    let j1 = view(&other, &v1, "org.kde.JobViewV2");
    j1.call::<_, _, ()>("setInfoMessage", &("Moving",)).unwrap();
    j1.call::<_, _, ()>("setPercent", &(70u32,)).unwrap();
    j1.call::<_, _, bool>("setDescriptionField", &(1u32, "Destination", "/data/x")).unwrap();
    assert!(wait(Duration::from_secs(3), || log.lock().unwrap().sent.last().is_some_and(|s| s.summary == "Moving")), "V1 job not shown");
    let m = log.lock().unwrap().sent.last().unwrap().clone();
    assert_eq!(m.app, "Dolphin");
    assert!(m.body.contains("Destination: /data/x"), "{}", m.body);
    assert_eq!(m.actions, ["cancel", "Cancel"]);

    // 8. the app quits mid-job: its popup closes
    drop(j1);
    drop(other);
    assert!(wait(Duration::from_secs(3), || log.lock().unwrap().closed.contains(&m.id)), "popup of a vanished app was not closed");
    let _ = std::fs::remove_dir_all(&tmp);
}
