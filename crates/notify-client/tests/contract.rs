//! Contract tests: the typed Rust client against a REAL notifd.
//!
//! Starts a throwaway daemon on a private D-Bus session (your real notifications are never touched), writes its
//! config *from the Rust `Config` type*, and checks that every message the daemon sends parses into the Rust
//! types and every request the client sends is accepted. Popups flash on screen briefly while this runs.
//!
//! Run:  cargo test -p notify-client --test contract -- --nocapture

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use notify_client::{Client, Error};
use notify_protocol::*;
use notify_testkit::{Checks, TestDaemon, test_config, wait_until};

const SHORT: Duration = Duration::from_secs(4);

// ------------------------------------------------------------------ the suite

#[test]
fn contract_against_a_real_daemon() {
    let cfg = test_config();
    let d = TestDaemon::start(cfg.clone());
    let mut k = Checks::default();
    println!("daemon up on {}", d.sock.display());

    // ---- greeting
    let mut c = d.client();
    let h = c.hello().clone();
    k.check("hello parses and the protocol version matches", h.protocol == PROTOCOL_VERSION && !h.daemon.is_empty() && h.pid > 0, || format!("{h:?}"));

    // ---- config model contract: what Rust writes, the daemon runs, and hands back unchanged
    let cg = c.config_get().expect("configGet parses into the Rust types");
    k.check("the daemon accepted the config written from Rust types", c.config_status().map(|s| s.ok).unwrap_or(false), || "config rejected".into());
    k.check("configGet.raw is the file we wrote", Config::from_json(&cg.raw).map(|x| x == cfg).unwrap_or(false), || cg.raw.clone());
    k.check("the daemon's effective config equals the Rust config (no field lost or invented)", cg.effective == cfg, || format!("{:?}\n!=\n{:?}", cg.effective, cfg));
    let shipped = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../notifd/rules.json")).unwrap();
    let shipped_cfg = Config::from_json(&shipped);
    k.check("the shipped rules.json parses into the Rust Config", shipped_cfg.is_ok(), || format!("{:?}", shipped_cfg.as_ref().err()));
    if let Ok(sc) = shipped_cfg {
        k.check("...and survives a Rust round trip", Config::from_json(&sc.to_json_pretty()).map(|x| x == sc).unwrap_or(false), || "round trip differs".into());
        k.check("...and the daemon accepts it", c.validate(&sc).map(|v| v.ok).unwrap_or(false), || "daemon rejected the shipped config".into());
    }

    // ---- status and controls
    let st = c.status().unwrap();
    k.check("status parses; zones match", {
        let mut z = st.zones.clone();
        z.sort();
        z == ["business", "personal", "system"]
    }, || format!("{st:?}"));
    let st = c.dnd("on").unwrap();
    k.check("dnd on is reflected in the typed status", st.dnd && st.holding == "dnd", || format!("{st:?}"));
    c.dnd("off").unwrap();
    let st = c.mute("business", true).unwrap();
    k.check("mute is reflected in the typed status", st.muted == ["business"], || format!("{st:?}"));
    c.mute("business", false).unwrap();
    let e = c.dnd("sideways").unwrap_err();
    k.check("bad parameters surface as Error::Daemon with code bad_params", e.code() == Some("bad_params"), || format!("{e}"));
    let e = c.mute("nowhere", true).unwrap_err();
    k.check("an unknown zone is bad_params", e.code() == Some("bad_params"), || format!("{e}"));
    let e = c.reply(999_999, "x").unwrap_err();
    k.check("replying to a missing notification is not_found", e.code() == Some("not_found"), || format!("{e}"));
    let e = c.call_raw("nonsense", serde_json::json!({})).unwrap_err();
    k.check("an unknown method is unknown_method", e.code() == Some("unknown_method"), || format!("{e}"));
    let st = c.set_auto("fullscreen", true).unwrap();
    k.check("setAuto fullscreen is reflected", st.auto.fullscreen && st.holding == "fullscreen", || format!("{st:?}"));
    c.set_auto("fullscreen", false).unwrap();

    // ---- validate
    let mut bad = cfg.clone();
    bad.default_zone = "ghost".into();
    let v = c.validate(&bad).unwrap();
    k.check("validate rejects a draft with the daemon's reason", !v.ok && v.error.as_deref().unwrap_or("").contains("defaultZone"), || format!("{v:?}"));
    let v = c.validate_text("{ not json").unwrap();
    k.check("validate_text rejects broken JSON", !v.ok, || format!("{v:?}"));
    k.check("validating never changes the running config", c.config_get().map(|g| g.effective == cfg).unwrap_or(false), || "config changed".into());

    // ---- simulate
    let sim = |c: &mut Client, app: &str, urgency: Urgency, ignore: bool, config: Option<Config>| {
        c.simulate(&SimRequest {
            config,
            notification: SimNotification { app_name: app.into(), summary: "x".into(), urgency, expire_timeout: -1, ..Default::default() },
            ignore_state: ignore,
        })
    };
    let r = sim(&mut c, "kernel", Urgency::Normal, false, None).unwrap();
    k.check("simulate: kernel goes to system, decided by the named rule", r.zone == "system" && r.decided_by.zone.as_deref() == Some("system") && r.would_show, || format!("{r:?}"));
    let r = sim(&mut c, "Steam", Urgency::Normal, false, None).unwrap();
    k.check("simulate: unmatched app uses the default zone and timeout", r.zone == "personal" && r.decided_by.zone.is_none() && r.timeout == 1500 && r.matched.is_empty(), || format!("{r:?}"));
    let mut draft = cfg.clone();
    draft.rules.insert(0, Rule { name: Some("all to system".into()), matches: BTreeMap::new(), zone: Some("system".into()), timeout: None, silent: None, ..Default::default() });
    let r = sim(&mut c, "Steam", Urgency::Normal, false, Some(draft)).unwrap();
    k.check("simulate can evaluate a typed draft config", r.zone == "system" && r.decided_by.zone.as_deref() == Some("all to system"), || format!("{r:?}"));
    c.dnd("on").unwrap();
    let held = sim(&mut c, "Steam", Urgency::Normal, false, None).unwrap();
    let crit = sim(&mut c, "Steam", Urgency::Critical, false, None).unwrap();
    let ign = sim(&mut c, "Steam", Urgency::Normal, true, None).unwrap();
    c.dnd("off").unwrap();
    k.check("simulate: DND holds normal, passes critical, and ignore_state shows the rule outcome", held.hold == "dnd" && !held.would_show && crit.would_show && ign.would_show, || format!("{held:?} {crit:?} {ign:?}"));
    let e = sim(&mut c, "x", Urgency::Normal, false, Some(Config { default_zone: "ghost".into(), ..cfg.clone() })).unwrap_err();
    k.check("simulate with an invalid draft is bad_params", e.code() == Some("bad_params"), || format!("{e}"));

    // ---- events (typed)
    let mut ev = d.client();
    let (snap, seq0) = ev.subscribe().unwrap();
    k.check("subscribe returns a typed status snapshot", snap.zones.len() == 3, || format!("{snap:?}"));
    c.dnd("on").unwrap();
    let e = ev.wait_event(SHORT, |e| matches!(e, Event::State(s) if s.dnd)).unwrap();
    k.check("a typed state event arrives when DND changes", e.is_some(), || "no state event".into());
    c.dnd("off").unwrap();
    wait_until(SHORT, || c.status().map(|s| s.active == 0).unwrap_or(false));

    let id: i64 = d.send("event test", "Brave Origin", 900, &["string:x-kde-origin-name:onlyfans.com"], "normal").parse().unwrap();
    let e = ev.wait_event(SHORT, |e| matches!(e, Event::Notification(n) if n.summary == "event test")).unwrap();
    k.check("a typed notification event carries zone, origin and outcome", matches!(&e, Some(EventEnvelope { event: Event::Notification(n), .. }) if n.zone == "business" && n.origin == "onlyfans.com" && n.shown && n.urgency == Urgency::Normal && n.id == id), || format!("{e:?}"));
    let e = ev.wait_event(Duration::from_secs(6), |e| matches!(e, Event::Closed(c) if c.id == id)).unwrap();
    k.check("a closed event follows", e.is_some(), || "no closed event".into());

    let first: i64 = d.send("Download 10%", "Steam", 4000, &[], "normal").parse().unwrap();
    ev.wait_event(SHORT, |e| matches!(e, Event::Notification(n) if n.summary == "Download 10%")).unwrap();
    let mut cmd = Command::new("notify-send");
    cmd.env("DBUS_SESSION_BUS_ADDRESS", &d.bus).args(["-r", &first.to_string(), "-a", "Steam", "-t", "4000", "Download 90%", "almost"]);
    cmd.status().unwrap();
    let e = ev.wait_event(SHORT, |e| matches!(e, Event::Updated(u) if u.summary == "Download 90%" && u.id == first)).unwrap();
    k.check("a replacement arrives as a typed updated event", e.is_some(), || "no updated event".into());
    c.dismiss_all().unwrap();

    let mut broken = cfg.clone();
    broken.default_zone = "ghost".into();
    d.write_config(&broken);
    let e = ev.wait_event(Duration::from_secs(6), |e| matches!(e, Event::Config(c) if !c.ok)).unwrap();
    k.check("a rejected config pushes a typed config event with the reason", matches!(&e, Some(EventEnvelope { event: Event::Config(c), .. }) if !c.error.is_empty()), || format!("{e:?}"));
    d.write_config(&cfg);
    let e = ev.wait_event(Duration::from_secs(6), |e| matches!(e, Event::Config(c) if c.ok)).unwrap();
    k.check("the good config is accepted again (event)", e.is_some(), || "no ok config event".into());

    let pv = c.preview(&PreviewRequest { seconds: Some(2), ..Default::default() }).unwrap();
    k.check("preview returns a typed result", pv.ok && pv.shown == 5 && pv.seconds == 2, || format!("{pv:?}"));
    let a = ev.wait_event(SHORT, |e| matches!(e, Event::Preview(p) if p.state == PreviewState::Started)).unwrap();
    let b = ev.wait_event(Duration::from_secs(8), |e| matches!(e, Event::Preview(p) if p.state == PreviewState::Ended)).unwrap();
    k.check("preview start and end are typed events", a.is_some() && b.is_some(), || "missing preview events".into());
    let mut draft = cfg.clone();
    draft.max_visible = 1;
    let pv = c.preview(&PreviewRequest { config: Some(draft), seconds: Some(2), ..Default::default() }).unwrap();
    k.check("preview accepts a typed draft config", pv.ok, || format!("{pv:?}"));
    c.end_preview().unwrap();
    let e = c.preview(&PreviewRequest { config: Some(Config { default_zone: "ghost".into(), ..cfg.clone() }), ..Default::default() }).unwrap_err();
    k.check("preview with an invalid draft is bad_params", e.code() == Some("bad_params"), || format!("{e}"));
    wait_until(SHORT, || c.status().map(|s| s.active == 0).unwrap_or(false));

    // ---- history (typed)
    c.dnd("on").unwrap();
    d.send("held one", "Steam", 500, &[], "normal");
    let held = wait_until(SHORT, || c.history(5).map(|h| h.iter().any(|x| x.summary == "held one")).unwrap_or(false));
    let hist = c.history(10).unwrap();
    c.dnd("off").unwrap();
    k.check("history parses and records why a notification was held", held && hist.iter().any(|x| x.summary == "held one" && !x.shown && x.why == "dnd"), || format!("{hist:?}"));

    // ---- apps that have sent notifications (kept even though history is off)
    d.send("who am i", "Contractapp", 400, &[], "normal");
    d.send("who am i again", "Contractapp", 400, &[], "normal");
    let got = wait_until(SHORT, || c.seen_apps().map(|a| a.iter().any(|x| x.app == "Contractapp" && x.count >= 2)).unwrap_or(false));
    let seen = c.seen_apps().unwrap();
    k.check("seenApps lists each app once with a count and a recent time", got && seen.iter().filter(|x| x.app == "Contractapp").count() == 1 && seen.iter().all(|x| x.last > 1_600_000_000_000), || format!("{seen:?}"));
    d.send("typed", "Contractapp", 300, &["string:category:im.received"], "critical");
    let typed = wait_until(SHORT, || c.seen_apps().map(|a| a.iter().any(|x| x.app == "Contractapp" && x.types.iter().any(|t| t.category == "im.received"))).unwrap_or(false));
    let seen = c.seen_apps().unwrap();
    let t = seen.iter().find(|x| x.app == "Contractapp").and_then(|x| x.types.iter().find(|t| t.category == "im.received"));
    k.check("seenApps carries each type with channel, capabilities and count", typed && t.is_some_and(|t| t.channel == "app" && t.caps.contains(&"critical".to_string()) && t.count == 1 && t.key == "im.received|app"), || format!("{seen:?}"));
    k.check("seenApps is newest first", seen.windows(2).all(|w| w[0].last >= w[1].last), || format!("{seen:?}"));

    // ---- ordering
    let mut seqs = Vec::new();
    c.dnd("on").unwrap();
    c.dnd("off").unwrap();
    d.send("order test", "Steam", 400, &[], "normal");
    while let Ok(Some(e)) = ev.next_event(Duration::from_millis(1200)) {
        seqs.push(e.seq);
    }
    k.check("event sequence numbers are strictly increasing and continue from the snapshot", !seqs.is_empty() && seqs.windows(2).all(|w| w[1] > w[0]) && seqs[0] >= seq0, || format!("{seqs:?} (snapshot {seq0})"));
    c.dismiss_all().unwrap();

    // ---- several clients at once
    let mut many: Vec<Client> = (0..4).map(|_| d.client()).collect();
    for m in &mut many {
        m.subscribe().unwrap();
    }
    d.send("fan-out", "Steam", 500, &[], "normal");
    let got: Vec<Option<u64>> = many.iter_mut().map(|m| m.wait_event(SHORT, |e| matches!(e, Event::Notification(n) if n.summary == "fan-out")).unwrap().map(|e| e.seq)).collect();
    k.check("four subscribers receive the same event with the same seq", got.iter().all(Option::is_some) && got.windows(2).all(|w| w[0] == w[1]), || format!("{got:?}"));

    // ---- the log must be clean
    drop(many);
    let log = fs::read_to_string(d.dir.join("log")).unwrap_or_default();
    let bad: Vec<&str> = log.lines().filter(|l| ["TypeError", "ReferenceError", "Cannot close destroyed", "is not a function"].iter().any(|w| l.contains(w))).collect();
    k.check("the daemon log has no script errors", bad.is_empty(), || bad.join("\n"));

    println!("\n{} passed, {} failed", k.passed, k.failed.len());
    assert!(k.failed.is_empty(), "contract failures:\n{}", k.failed.join("\n"));
}

// ------------------------------------------------------------------ client behaviour that needs no daemon

/// Serve a scripted greeting on a temporary socket.
fn fake_server(script: impl FnOnce(std::os::unix::net::UnixStream) + Send + 'static) -> (PathBuf, thread::JoinHandle<()>) {
    let path = std::env::temp_dir().join(format!("notify-fake-{}-{:?}.sock", std::process::id(), thread::current().id()));
    let _ = fs::remove_file(&path);
    let listener = UnixListener::bind(&path).unwrap();
    let h = thread::spawn(move || {
        let (s, _) = listener.accept().unwrap();
        script(s);
    });
    (path, h)
}

#[test]
fn refuses_an_incompatible_daemon() {
    let (p, h) = fake_server(|mut s| {
        s.write_all(b"{\"event\":\"hello\",\"protocol\":999,\"daemon\":\"9.9\",\"pid\":1,\"seq\":0}\n").unwrap();
        thread::sleep(Duration::from_millis(300));
    });
    let e = Client::connect(&p).err().expect("must refuse");
    assert!(matches!(e, Error::VersionMismatch { daemon: 999, client: PROTOCOL_VERSION }), "{e}");
    assert!(e.to_string().contains("notify update"), "the message should say what to do: {e}");
    h.join().unwrap();
}

#[test]
fn reports_a_refused_connection() {
    let (p, h) = fake_server(|mut s| {
        s.write_all(b"{\"event\":\"error\",\"code\":\"too_many_connections\",\"message\":\"too many control connections\"}\n").unwrap();
        thread::sleep(Duration::from_millis(300));
    });
    let e = Client::connect(&p).err().expect("must fail");
    assert!(matches!(&e, Error::Refused(m) if m.contains("too_many_connections")), "{e}");
    h.join().unwrap();
}

#[test]
fn survives_a_greeting_split_across_packets_and_a_closed_peer() {
    let (p, h) = fake_server(|mut s| {
        s.write_all(b"{\"event\":\"hello\",\"proto").unwrap();
        s.flush().unwrap();
        thread::sleep(Duration::from_millis(200));
        s.write_all(b"col\":1,\"daemon\":\"x\",\"pid\":7,\"seq\":3}\n").unwrap();
        thread::sleep(Duration::from_millis(100));
        // then hang up without answering the request
    });
    let mut c = Client::connect(&p).expect("a split greeting must still parse");
    assert_eq!(c.hello().pid, 7);
    assert_eq!(c.hello().seq, 3);
    let e = c.status().unwrap_err();
    assert!(matches!(e, Error::Closed), "{e}");
    h.join().unwrap();
}

#[test]
fn times_out_when_the_daemon_stays_silent() {
    let (p, h) = fake_server(|mut s| {
        s.write_all(b"{\"event\":\"hello\",\"protocol\":1,\"daemon\":\"x\",\"pid\":7,\"seq\":0}\n").unwrap();
        thread::sleep(Duration::from_millis(900));
    });
    let mut c = Client::connect(&p).unwrap();
    let e = c.call_raw_timeout("status", serde_json::json!({}), Duration::from_millis(300)).unwrap_err();
    assert!(matches!(e, Error::Timeout), "{e}");
    h.join().unwrap();
}

#[test]
fn a_missing_socket_gives_a_helpful_error() {
    let e = Client::connect("/nonexistent/notifd.sock").err().unwrap();
    assert!(matches!(e, Error::Connect { .. }), "{e}");
    assert!(e.to_string().contains("/nonexistent/notifd.sock"), "{e}");
}
