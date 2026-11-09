//! `notify serve`: runs notifd (Quickshell) on a private bus and owns `org.freedesktop.Notifications` for it on the session bus.
//!
//! Quickshell's notification server cannot be extended, and it only answers on a bus where it owns the name. So notifd's
//! Quickshell gets a bus of its own, and this program stands in front of it: every call is passed straight through, every
//! signal (ActionInvoked, NotificationClosed, ...) is passed back, and `Inhibit` (which Quickshell does not have) is answered
//! here. If Quickshell or the private bus stops, this stops too, and systemd starts the whole set again.

mod front;
mod private;

use std::path::Path;
use std::process::{Child, Command};
use std::sync::mpsc::{Receiver, channel};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use zbus::blocking::{Connection, MessageIterator};
use zbus::fdo::RequestNameFlags;
use zbus::MatchRule;
use zbus::message::Type as MessageType;

pub use front::{Inhibition, Inhibitions, NAME, PATH};
use front::Front;
use private::PrivateBus;

/// The signals apps listen for, and their argument types.
const SIGNALS: [&str; 4] = ["ActionInvoked", "NotificationClosed", "ActivationToken", "NotificationReplied"];

/// The per-user runtime folder (mode 0700). Without it there is no safe place for the private bus: a shared /tmp is not one.
fn runtime_dir() -> Result<std::path::PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()).map(Into::into).context("XDG_RUNTIME_DIR is not set: no private place for notifd's bus")
}

/// Quickshell, stopped and reaped however `serve` ends (an error part-way through start-up included).
struct KillOnDrop(Child);
impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Set while a helper thread runs; the supervisor stops `serve` when one ends (a dead forwarder would leave apps without
/// ActionInvoked, a dead watcher would keep inhibitions forever), and systemd starts it again.
fn running(flag: &Arc<AtomicBool>) -> Arc<AtomicBool> {
    flag.store(true, Ordering::SeqCst);
    flag.clone()
}

/// Where Qt's accessibility (screen readers) lives: asked of the session bus, since Quickshell no longer sees it.
fn a11y_address(session: &Connection) -> Option<String> {
    let r = session.call_method(Some("org.a11y.Bus"), "/org/a11y/bus", Some("org.a11y.Bus"), "GetAddress", &()).ok()?;
    r.body().deserialize::<String>().ok()
}

fn owner_on(conn: &Connection, name: &str) -> bool {
    conn.call_method(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", Some("org.freedesktop.DBus"), "NameHasOwner", &(name,))
        .ok()
        .and_then(|r| r.body().deserialize::<bool>().ok())
        .unwrap_or(false)
}

/// Pass Quickshell's signals on to the apps on the session bus, unchanged.
fn forward_signals(inner: &Connection, session: Connection, alive: Arc<AtomicBool>) -> Result<()> {
    let rule = MatchRule::builder().msg_type(MessageType::Signal).interface(NAME)?.path(PATH)?.build();
    let it = MessageIterator::for_match_rule(rule, inner, Some(64))?;
    std::thread::spawn(move || {
        let _done = Flag(alive);
        for m in it.flatten() {
            let h = m.header();
            let Some(member) = h.member().map(|m| m.to_string()) else { continue };
            if !SIGNALS.contains(&member.as_str()) {
                continue;
            }
            let body = m.body();
            let sent = match member.as_str() {
                "NotificationClosed" => body.deserialize::<(u32, u32)>().map(|b| session.emit_signal(None::<&str>, PATH, NAME, member.as_str(), &b)),
                _ => body.deserialize::<(u32, String)>().map(|b| session.emit_signal(None::<&str>, PATH, NAME, member.as_str(), &b)),
            };
            if let Ok(Err(e)) | Err(e) = sent {
                eprintln!("notify-bus: could not pass on {member}: {e}");
            }
        }
    });
    Ok(())
}

/// Apps that leave the bus lose their inhibitions.
fn watch_departures(session: &Connection, inh: Arc<Mutex<Inhibitions>>, changed: std::sync::mpsc::Sender<()>, alive: Arc<AtomicBool>) -> Result<()> {
    let rule = MatchRule::builder().msg_type(MessageType::Signal).sender("org.freedesktop.DBus")?.interface("org.freedesktop.DBus")?.member("NameOwnerChanged")?.build();
    let it = MessageIterator::for_match_rule(rule, session, Some(64))?;
    std::thread::spawn(move || {
        let _done = Flag(alive);
        for m in it.flatten() {
            let Ok((name, _old, new)) = m.body().deserialize::<(String, String, String)>() else { continue };
            if name.starts_with(':') && new.is_empty() && inh.lock().unwrap().drop_sender(&name) {
                eprintln!("notify-bus: {name} left; its inhibition ends");
                let _ = changed.send(());
            }
        }
    });
    Ok(())
}

/// Tell notifd who inhibits (it holds popups while anyone does), and announce `Inhibited` to the session.
fn tell_notifd(rx: Receiver<()>, inh: Arc<Mutex<Inhibitions>>, session: Connection) {
    std::thread::spawn(move || {
        let mut last = None;
        while rx.recv().is_ok() {
            while rx.try_recv().is_ok() {} // coalesce bursts
            // keep trying until notifd takes the list (it may be starting or reloading); a newer change replaces it
            for attempt in 0u32.. {
                while rx.try_recv().is_ok() {}
                let list = inh.lock().unwrap().list();
                match notify_client::Client::connect_default().and_then(|mut c| c.call_raw("inhibit", serde_json::json!({ "by": list }))) {
                    Ok(_) => break,
                    Err(e) => {
                        if attempt == 20 {
                            eprintln!("notify-bus: notifd does not take the inhibitions yet ({e}); still trying");
                        }
                        std::thread::sleep(Duration::from_millis(if attempt < 20 { 250 } else { 1000 }));
                    }
                }
            }
            let now = !inh.lock().unwrap().by_cookie.is_empty();
            if last != Some(now) {
                last = Some(now);
                if let Ok(iface) = session.object_server().interface::<_, Front>(PATH) {
                    let emitter = iface.signal_emitter().clone();
                    let _ = zbus::block_on(iface.get().inhibited_changed(&emitter));
                }
            }
        }
    });
}

/// Clears its flag when the thread that holds it ends, however it ends.
struct Flag(Arc<AtomicBool>);
impl Drop for Flag {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn alive(c: &mut Child) -> bool {
    matches!(c.try_wait(), Ok(None))
}

/// Run notifd from `payload` with Quickshell at `qs`, until either stops.
pub fn serve(payload: &Path, qs: &Path) -> Result<()> {
    let runtime = runtime_dir()?;
    PrivateBus::sweep(&runtime);
    // a second notifd would take over the control socket's path (Quickshell deletes the file in its way) and leave the
    // running one unreachable: refuse while one answers there
    if notify_client::Client::connect_default().and_then(|mut c| c.ping()).is_ok() {
        bail!("notifd is already running (its control socket at {} answers); stop it first", notify_client::default_socket_path().display());
    }
    let session = Connection::session().context("connecting to the session bus")?;
    let bus = PrivateBus::start(&runtime)?;
    let inner = zbus::blocking::connection::Builder::address(bus.address.as_str())?.build().context("connecting to the private bus")?;

    let mut cmd = Command::new(qs);
    cmd.arg("-p").arg(payload).env("DBUS_SESSION_BUS_ADDRESS", &bus.address).env("NOTIFD_BEHIND_PROXY", "1");
    if let Some(a) = a11y_address(&session) {
        cmd.env("AT_SPI_BUS_ADDRESS", a);
    }
    let mut qs = KillOnDrop(cmd.spawn().with_context(|| format!("starting {}", qs.display()))?);
    let qs_child = &mut qs.0;
    let deadline = Instant::now() + Duration::from_secs(20);
    while !owner_on(&inner, NAME) {
        if !alive(qs_child) {
            bail!("Quickshell exited before it took the notification name on the private bus");
        }
        if Instant::now() > deadline {
            bail!("Quickshell did not take the notification name on the private bus within 20 s");
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let inh = Arc::new(Mutex::new(Inhibitions::default()));
    let (changed, rx) = channel();
    session.object_server().at(PATH, Front { inner: inner.clone(), inhibitions: inh.clone(), changed: changed.clone() })?;
    let (forwarding, watching) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
    forward_signals(&inner, session.clone(), running(&forwarding))?;
    watch_departures(&session, inh.clone(), changed.clone(), running(&watching))?;
    tell_notifd(rx, inh, session.clone());
    // queue if someone (Plasma) holds the name, and do not let it be taken from us (no AllowReplacement), as the job bridge does
    let reply = session.request_name_with_flags(NAME, RequestNameFlags::ReplaceExisting.into()).context("requesting org.freedesktop.Notifications")?;
    eprintln!("notify-bus: {NAME}: {reply:?} (Quickshell runs on a private bus behind it)");
    let _ = changed.send(()); // start notifd with an empty list (a restart forgets earlier inhibitions, as their apps' cookies are gone too)

    let mut bus = bus;
    loop {
        if !alive(qs_child) {
            bail!("Quickshell stopped");
        }
        if !alive(&mut bus.child) {
            bail!("the private bus stopped");
        }
        if !forwarding.load(Ordering::SeqCst) || !watching.load(Ordering::SeqCst) {
            bail!("a helper thread stopped (signals or departures are no longer followed)");
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}
