//! The worker against a real daemon and against scripted fake daemons. No Qt, no display.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify_panel_core::view::Conn;
use notify_panel_core::worker::{Cmd, Msg, Snapshot, run};
use notify_testkit::fake::{fake_daemon, serve};
use notify_testkit::{Checks, TestDaemon, test_config};

/// A running worker plus the channels around it.
struct Harness {
    cmd: Sender<Cmd>,
    msgs: Receiver<Msg>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    seen: Vec<Msg>,
}

impl Harness {
    fn start(socket: PathBuf) -> Harness {
        let (ctx, crx) = mpsc::channel::<Cmd>();
        let (mtx, mrx) = mpsc::channel::<Msg>();
        let stop = Arc::new(AtomicBool::new(false));
        let s2 = stop.clone();
        let handle = thread::spawn(move || run(socket, crx, s2, move |m| drop(mtx.send(m))));
        Harness { cmd: ctx, msgs: mrx, stop, handle: Some(handle), seen: Vec::new() }
    }

    /// Wait for a message matching `pred`; everything received is kept in `seen`.
    fn wait(&mut self, timeout: Duration, mut pred: impl FnMut(&Msg) -> bool) -> Option<Msg> {
        let end = Instant::now() + timeout;
        while Instant::now() < end {
            if let Ok(m) = self.msgs.recv_timeout(Duration::from_millis(100)) {
                self.seen.push(m.clone());
                if pred(&m) {
                    return Some(m);
                }
            }
        }
        None
    }

    fn snapshot(m: &Msg) -> Option<&Snapshot> {
        if let Msg::Snapshot(s) = m { Some(s) } else { None }
    }

    fn states(&self) -> Vec<Conn> {
        self.seen.iter().filter_map(|m| if let Msg::State(c) = m { Some(c.clone()) } else { None }).collect()
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

const SHORT: Duration = Duration::from_secs(5);

// ---------------------------------------------------------------- tests

#[test]
fn worker_against_a_real_daemon() {
    let d = TestDaemon::start(test_config());
    let mut k = Checks::default();
    let mut h = Harness::start(d.sock.clone());

    let first = h.wait(SHORT, |m| Harness::snapshot(m).is_some());
    k.check("the first message is a full snapshot", first.is_some(), || "no snapshot".into());
    let s = first.as_ref().and_then(Harness::snapshot).unwrap();
    k.check("the snapshot carries zones, config and the daemon version", s.status.zones.len() == 3 && s.config.zones.len() == 3 && !s.version.is_empty(), || format!("{:?}", s.status));
    let connected = h.wait(SHORT, |m| matches!(m, Msg::State(Conn::Connected)));
    k.check("the state becomes connected", connected.is_some(), || format!("{:?}", h.states()));
    k.check("states arrive in order: connecting, then connected", h.states().first() == Some(&Conn::Connecting), || format!("{:?}", h.states()));

    h.cmd.send(Cmd::Dnd(true)).unwrap();
    let m = h.wait(SHORT, |m| Harness::snapshot(m).map(|s| s.status.dnd).unwrap_or(false));
    k.check("a Do Not Disturb command is applied and the new state comes back", m.is_some(), || "dnd never turned on".into());
    h.cmd.send(Cmd::Dnd(false)).unwrap();
    h.wait(SHORT, |m| Harness::snapshot(m).map(|s| !s.status.dnd).unwrap_or(false));

    h.cmd.send(Cmd::Mute("business".into(), true)).unwrap();
    let m = h.wait(SHORT, |m| Harness::snapshot(m).map(|s| s.status.muted == ["business"]).unwrap_or(false));
    k.check("a mute command is applied", m.is_some(), || "business never muted".into());
    h.cmd.send(Cmd::Mute("business".into(), false)).unwrap();

    h.cmd.send(Cmd::Mute("nowhere".into(), true)).unwrap();
    let m = h.wait(SHORT, |m| matches!(m, Msg::Failed(_)));
    k.check("a refused command is reported with the daemon's reason", matches!(&m, Some(Msg::Failed(t)) if t.contains("unknown zone")), || format!("{m:?}"));
    k.check("...and does not drop the connection", h.wait(Duration::from_millis(800), |m| matches!(m, Msg::State(c) if *c != Conn::Connected)).is_none(), || "state changed after a refused command".into());

    d.send("hello worker", "Steam", 800, &[], "normal");
    let m = h.wait(SHORT, |m| Harness::snapshot(m).map(|s| s.history.iter().any(|x| x.summary == "hello worker")).unwrap_or(false));
    k.check("a new notification shows up in the next snapshot's history", m.is_some(), || "not in history".into());

    h.cmd.send(Cmd::ClearHistory).unwrap();
    let m = h.wait(SHORT, |m| Harness::snapshot(m).map(|s| s.history.is_empty()).unwrap_or(false));
    k.check("clearing history is applied", m.is_some(), || "history not cleared".into());

    // a burst of notifications must not cause a snapshot per event
    h.seen.clear();
    while h.msgs.try_recv().is_ok() {}
    // sent from several threads at once, so they really arrive as a burst (one notify-send process per notification is slow)
    thread::scope(|sc| {
        for t in 0..6 {
            let d = &d;
            sc.spawn(move || {
                for i in 0..10 {
                    d.send(&format!("burst {}", t * 10 + i), "Steam", 300, &[], "normal");
                }
            });
        }
    });
    d.send("burst 59", "Steam", 300, &[], "normal");
    let last = h.wait(Duration::from_secs(8), |m| Harness::snapshot(m).map(|s| s.history.iter().any(|x| x.summary == "burst 59")).unwrap_or(false));
    thread::sleep(Duration::from_millis(500));
    while let Ok(m) = h.msgs.try_recv() {
        h.seen.push(m);
    }
    let snaps = h.seen.iter().filter(|m| matches!(m, Msg::Snapshot(_))).count();
    k.check("the last notification of a burst of 60 is in the final picture", last.is_some(), || "burst not fully reflected".into());
    k.check("60 notifications cause far fewer than 60 refreshes (events are coalesced)", snaps < 45, || format!("{snaps} snapshots"));

    println!("\n{} passed, {} failed", k.passed, k.failed.len());
    assert!(k.failed.is_empty(), "failures:\n{}", k.failed.join("\n"));
}

#[test]
fn no_daemon_is_reported_once_and_the_worker_keeps_trying() {
    let path = std::env::temp_dir().join(format!("notify-panel-absent-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let mut h = Harness::start(path);
    let m = h.wait(SHORT, |m| matches!(m, Msg::State(Conn::NoDaemon)));
    assert!(m.is_some(), "{:?}", h.states());
    // stay down for two more retry cycles: the state must not be re-announced
    h.wait(Duration::from_millis(2500), |_| false);
    let n = h.states().iter().filter(|c| **c == Conn::NoDaemon).count();
    assert_eq!(n, 1, "the same state must be reported once, got {:?}", h.states());
}

#[test]
fn an_incompatible_daemon_is_named_with_advice() {
    let (path, _) = fake_daemon("incompat", |_, s| serve(s, 999, None));
    let mut h = Harness::start(path);
    let m = h.wait(SHORT, |m| matches!(m, Msg::State(Conn::Incompatible { .. })));
    match m {
        Some(Msg::State(c)) => {
            assert_eq!(c, Conn::Incompatible { daemon: 999 });
            assert!(c.message().contains("notify update"), "{}", c.message());
        }
        other => panic!("expected an incompatible state, got {other:?} / {:?}", h.states()),
    }
    assert!(h.seen.iter().all(|m| !matches!(m, Msg::Snapshot(_))), "no data may be shown from an incompatible daemon");
}

#[test]
fn a_dropped_connection_is_noticed_and_the_worker_reconnects() {
    // first connection: answers subscribe, configGet, status, history, then hangs up; later ones stay
    let (path, count) = fake_daemon("flaky", |n, s| serve(s, 1, if n == 1 { Some(4) } else { None }));
    let mut h = Harness::start(path);
    // wait until we have seen: Connected, Lost, Connected
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let st = h.states();
        let ok = st.iter().filter(|c| **c == Conn::Connected).count() >= 2 && st.iter().any(|c| matches!(c, Conn::Lost(_)));
        if ok || Instant::now() > end {
            break;
        }
        h.wait(Duration::from_millis(200), |_| false);
    }
    let st = h.states();
    assert!(st.iter().filter(|c| **c == Conn::Connected).count() >= 2, "expected two connections, states: {st:?}");
    assert!(st.iter().any(|c| matches!(c, Conn::Lost(_))), "the drop must be reported: {st:?}");
    assert!(*count.lock().unwrap() >= 2);
    assert_eq!(st.last(), Some(&Conn::Connected), "ends connected: {st:?}");
    let snaps = h.seen.iter().filter(|m| matches!(m, Msg::Snapshot(_))).count();
    assert!(snaps >= 2, "a fresh snapshot must follow each connection, got {snaps}");
}

#[test]
fn stopping_the_worker_ends_its_thread_promptly() {
    let (path, _) = fake_daemon("stop", |_, s| serve(s, 1, None));
    let h = Harness::start(path);
    thread::sleep(Duration::from_millis(500));
    let t = Instant::now();
    drop(h); // sets the flag and joins
    assert!(t.elapsed() < Duration::from_secs(2), "worker took {:?} to stop", t.elapsed());
}

// ---------------------------------------------------------------- the rules editor's commands

#[test]
fn rules_editing_commands_against_a_real_daemon() {
    use notify_panel_core::draft::{Draft, SaveOutcome};
    use serde_json::json;

    let d = TestDaemon::start(test_config());
    let mut k = Checks::default();
    let mut h = Harness::start(d.sock.clone());
    let first = h.wait(SHORT, |m| Harness::snapshot(m).is_some());
    let snap = first.as_ref().and_then(Harness::snapshot).expect("a snapshot");
    let mut draft = Draft::from_raw(&snap.raw).expect("the daemon's raw config text parses into a draft");
    k.check("the snapshot carries rules.json exactly as on disk", snap.raw == std::fs::read_to_string(d.rules_path()).unwrap() && draft.rule_count() == 3, || format!("{} rules", draft.rule_count()));

    // ---- validate
    h.cmd.send(Cmd::Validate { token: 1, config: draft.value().clone() }).unwrap();
    let m = h.wait(SHORT, |m| matches!(m, Msg::Validated { token: 1, .. }));
    k.check("a good draft is validated ok by the daemon", matches!(&m, Some(Msg::Validated { ok: true, .. })), || format!("{m:?}"));
    let mut bad = draft.value().clone();
    bad["defaultZone"] = json!("ghost");
    h.cmd.send(Cmd::Validate { token: 2, config: bad }).unwrap();
    let m = h.wait(SHORT, |m| matches!(m, Msg::Validated { token: 2, .. }));
    k.check("a bad draft comes back not ok, with the daemon's reason", matches!(&m, Some(Msg::Validated { ok: false, error, .. }) if error.contains("defaultZone")), || format!("{m:?}"));

    // a burst of 40 validations: the newest is answered, most of the others are skipped
    h.seen.clear();
    for t in 100..140u64 {
        h.cmd.send(Cmd::Validate { token: t, config: draft.value().clone() }).unwrap();
    }
    let newest = h.wait(SHORT, |m| matches!(m, Msg::Validated { token: 139, .. }));
    let answered = h.seen.iter().filter(|m| matches!(m, Msg::Validated { .. })).count();
    k.check("the newest validation of a burst is answered", newest.is_some(), || "token 139 never answered".into());
    k.check("...and far fewer than 40 were actually run (edits are coalesced)", answered < 20, || format!("{answered} answers"));

    // ---- simulate against the draft
    draft.add_rule();
    draft.move_rule(3, -3); // to the top
    draft.set_name(0, "all to system");
    draft.set_zone(0, Some("system"));
    let req = json!({"config": draft.value(), "notification": {"appName": "Steam", "summary": "x"}});
    h.cmd.send(Cmd::Simulate { token: 7, request: req }).unwrap();
    let m = h.wait(SHORT, |m| matches!(m, Msg::Simulated { token: 7, .. }));
    match &m {
        Some(Msg::Simulated { result: Ok(r), .. }) => {
            k.check("simulate evaluates the DRAFT (not the running config)", r.zone == "system" && r.decided_by.zone.as_deref() == Some("all to system"), || format!("{r:?}"));
            k.check("...and reports which rule positions matched", r.matched_idx == [0] && r.decided_idx.zone == Some(0), || format!("{r:?}"));
        }
        other => k.check("simulate answers", false, || format!("{other:?}")),
    }
    let mut broken = draft.value().clone();
    broken["defaultZone"] = json!("ghost");
    h.cmd.send(Cmd::Simulate { token: 8, request: json!({"config": broken, "notification": {"appName": "x"}}) }).unwrap();
    let m = h.wait(SHORT, |m| matches!(m, Msg::Simulated { token: 8, .. }));
    k.check("simulating an invalid draft says why instead of guessing", matches!(&m, Some(Msg::Simulated { result: Err(e), .. }) if e.contains("defaultZone")), || format!("{m:?}"));

    // ---- preview
    h.cmd.send(Cmd::Preview { config: draft.value().clone(), seconds: 3 }).unwrap();
    let m = h.wait(SHORT, |m| Harness::snapshot(m).map(|s| s.status.previewing && s.status.active > 0).unwrap_or(false));
    k.check("a preview with the draft shows popups (the daemon reports previewing)", m.is_some(), || "no previewing snapshot".into());
    h.cmd.send(Cmd::EndPreview).unwrap();
    let m = h.wait(SHORT, |m| Harness::snapshot(m).map(|s| !s.status.previewing && s.status.active == 0).unwrap_or(false));
    k.check("ending the preview clears them", m.is_some(), || "still previewing".into());

    // ---- save, conflict, force
    let base = draft.base_raw().to_string();
    let text = draft.to_text();
    h.cmd.send(Cmd::Save { path: d.rules_path(), text: text.clone(), expected_base: base.clone(), force: false }).unwrap();
    let m = h.wait(SHORT, |m| matches!(m, Msg::Saved { .. }));
    k.check("a save reports success", matches!(&m, Some(Msg::Saved { outcome: SaveOutcome::Saved, .. })), || format!("{m:?}"));
    let m = h.wait(SHORT, |m| Harness::snapshot(m).map(|s| s.raw == text).unwrap_or(false));
    k.check("the daemon reloads it and the next snapshot carries the saved text", m.is_some(), || "snapshot never showed the saved text".into());
    k.check("the daemon accepted what the panel wrote", d.client().config_status().map(|s| s.ok && s.rules == 4).unwrap_or(false), || "daemon rejected or did not reload".into());

    // someone edits the file by hand; a save from the old base must be refused
    let hand = std::fs::read_to_string(d.rules_path()).unwrap().replace("\"defaultTimeout\": 1500", "\"defaultTimeout\": 1700");
    std::fs::write(d.rules_path(), &hand).unwrap();
    h.wait(SHORT, |m| Harness::snapshot(m).map(|s| s.raw == hand).unwrap_or(false));
    draft.add_rule();
    h.cmd.send(Cmd::Save { path: d.rules_path(), text: draft.to_text(), expected_base: text.clone(), force: false }).unwrap();
    let m = h.wait(SHORT, |m| matches!(m, Msg::Saved { .. }));
    k.check("saving over a hand edit is refused as a conflict", matches!(&m, Some(Msg::Saved { outcome: SaveOutcome::Conflict { .. }, .. })), || format!("{m:?}"));
    k.check("...and nothing was written", std::fs::read_to_string(d.rules_path()).unwrap() == hand, || "file changed".into());
    h.cmd.send(Cmd::Save { path: d.rules_path(), text: draft.to_text(), expected_base: text, force: true }).unwrap();
    let m = h.wait(SHORT, |m| matches!(m, Msg::Saved { .. }));
    k.check("forcing overwrites", matches!(&m, Some(Msg::Saved { outcome: SaveOutcome::Saved, .. })) && std::fs::read_to_string(d.rules_path()).unwrap() == draft.to_text(), || format!("{m:?}"));
    k.check("...keeping the hand-edited version as a backup", std::fs::read_to_string(d.dir.join("p/rules.json.bak")).map(|b| b == hand).unwrap_or(false), || "no .bak with the hand edit".into());

    println!("\n{} passed, {} failed", k.passed, k.failed.len());
    assert!(k.failed.is_empty(), "failures:\n{}", k.failed.join("\n"));
}

#[test]
fn app_discovery_combines_what_the_daemon_saw_with_what_is_installed() {
    let d = TestDaemon::start(test_config());
    let mut k = Checks::default();
    let mut h = Harness::start(d.sock.clone());
    h.wait(SHORT, |m| matches!(m, Msg::State(Conn::Connected)));

    // an installed app that never sends, plus a fixture config dir
    let root = std::env::temp_dir().join(format!("notify-worker-apps-{}", std::process::id()));
    let apps = root.join("share/applications");
    std::fs::create_dir_all(&apps).unwrap();
    std::fs::create_dir_all(root.join("config")).unwrap();
    std::fs::write(apps.join("quietchat.desktop"), "[Desktop Entry]\nType=Application\nName=Quiet Chat\nCategories=Network;InstantMessaging;\n").unwrap();
    let roots = notify_apps::Roots { data_dirs: vec![root.join("share")], config_dir: root.join("config"), flatpak: false };

    d.send("hello", "Workerapp", 300, &[], "normal");
    d.send("hello again", "Workerapp", 300, &[], "normal");
    let mut c = d.client();
    notify_testkit::wait_until(SHORT, || c.seen_apps().map(|a| a.iter().any(|x| x.app == "Workerapp" && x.count >= 2)).unwrap_or(false));

    h.cmd.send(Cmd::ScanApps { token: 1, roots: roots.clone() }).unwrap();
    let m = h.wait(SHORT, |m| matches!(m, Msg::Apps { token: 1, .. }));
    let apps_found = match &m {
        Some(Msg::Apps { result: Ok(a), .. }) => a.clone(),
        other => panic!("no apps answer: {other:?}"),
    };
    let w = apps_found.iter().find(|a| a.name == "Workerapp");
    k.check("an app that sent notifications is listed as sent, with its count", w.is_some_and(|a| a.sent && a.count >= 2), || format!("{apps_found:?}"));
    let q = apps_found.iter().find(|a| a.id == "quietchat");
    k.check("an installed messaging app that never sent is listed as capable but silent", q.is_some_and(|a| a.capable && !a.sent), || format!("{apps_found:?}"));
    k.check("sent apps come before silent ones", apps_found.iter().position(|a| a.name == "Workerapp") < apps_found.iter().position(|a| a.id == "quietchat"), || format!("{apps_found:?}"));

    // a bad Flatpak id is refused with a message, and nothing else breaks
    h.cmd.send(Cmd::FlatpakPermission { token: 2, roots, app: "--help".into(), allow: Some(false) }).unwrap();
    let m = h.wait(SHORT, |m| matches!(m, Msg::Failed(_)));
    k.check("an invalid Flatpak id is refused", matches!(&m, Some(Msg::Failed(t)) if t.contains("not a valid app id")), || format!("{m:?}"));
    let _ = std::fs::remove_dir_all(&root);

    println!("\n{} passed, {} failed", k.passed, k.failed.len());
    assert!(k.failed.is_empty(), "failures:\n{}", k.failed.join("\n"));
}
