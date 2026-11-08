//! The job bridge: shows file operations (Dolphin's copy, move, delete, extract; any KIO job) through notifd.
//!
//! KDE apps do not send those as notifications: they report them to `org.kde.JobViewServer`, which Plasma's
//! notification applet owns and draws. With notifd in charge that applet is gone, so this owns the job server
//! names instead and turns every job into one notification with a progress value that it keeps updating.
//! Plasma claims the names without allowing replacement, so this must start before plasmashell
//! (`notifd-jobs.service` is ordered so); started later, it waits in the D-Bus queue until Plasma lets go.

pub mod apps;
pub mod model;
mod server;
mod url;
mod worker;

use std::sync::{Arc, mpsc};

use anyhow::{Context, Result};
use zbus::blocking::{Connection, Proxy};
use zbus::fdo::{RequestNameFlags, RequestNameReply};

use server::{Ev, Kuiserver, ServerV1, ServerV2, Shared};

pub const JOB_SERVER: &str = "org.kde.JobViewServer";
pub const KUISERVER: &str = "org.kde.kuiserver";

/// Tell systemd we are ready (Type=notify), so plasmashell starts only after we hold the names.
fn sd_ready() {
    use std::os::unix::net::UnixDatagram;
    let Some(sock) = std::env::var_os("NOTIFY_SOCKET") else { return };
    let Ok(d) = UnixDatagram::unbound() else { return };
    let s = sock.to_string_lossy();
    let sent = if let Some(abs) = s.strip_prefix('@') {
        use std::os::linux::net::SocketAddrExt;
        std::os::unix::net::SocketAddr::from_abstract_name(abs.as_bytes()).and_then(|a| d.send_to_addr(b"READY=1", &a))
    } else {
        d.send_to(b"READY=1", &*s)
    };
    if let Err(e) = sent {
        eprintln!("notify-jobs: could not notify systemd: {e}");
    }
}

/// A plain proxy that never reads the service's properties.
pub(crate) fn proxy(conn: &Connection, dest: &'static str, path: &'static str, iface: &'static str) -> zbus::Result<Proxy<'static>> {
    zbus::blocking::proxy::Builder::new(conn).destination(dest)?.path(path)?.interface(iface)?.cache_properties(zbus::proxy::CacheProperties::No).build()
}

/// Forward one signal from `proxy` into the worker, on its own thread.
fn listen<T, F>(proxy: Proxy<'static>, signal: &'static str, tx: Arc<Shared>, f: F)
where
    T: serde::de::DeserializeOwned + zbus::zvariant::Type,
    F: Fn(T) -> Option<Ev> + Send + 'static,
{
    std::thread::spawn(move || {
        let Ok(it) = proxy.receive_signal(signal) else {
            eprintln!("notify-jobs: cannot listen for {signal}");
            return;
        };
        for m in it {
            if let Some(ev) = m.body().deserialize::<T>().ok().and_then(&f) {
                tx.send(ev);
            }
        }
    });
}

/// Run the bridge until the session ends.
pub fn run() -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let sh = Arc::new(Shared::new(tx));
    let server = zbus::blocking::connection::Builder::session()?
        .serve_at("/JobViewServer", ServerV2(sh.clone()))?
        .serve_at("/JobViewServer", ServerV1(sh.clone()))?
        .serve_at("/JobViewServer", Kuiserver)?
        .build()
        .context("connecting to the session bus")?;
    for name in [JOB_SERVER, KUISERVER] {
        // queue (no DoNotQueue) and refuse replacement (no AllowReplacement): we keep the names once we have them
        match server.request_name_with_flags(name, RequestNameFlags::ReplaceExisting.into()).with_context(|| format!("requesting {name}"))? {
            RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => eprintln!("notify-jobs: owns {name}"),
            _ => eprintln!("notify-jobs: {name} is held by another program (Plasma); waiting for it, file operations show here from the next login"),
        }
    }
    sd_ready();

    let client = Connection::session().context("connecting to the session bus")?;
    let notif = proxy(&client, "org.freedesktop.Notifications", "/org/freedesktop/Notifications", "org.freedesktop.Notifications")?;
    listen::<(u32, String), _>(notif.clone(), "ActionInvoked", sh.clone(), |(nid, key)| Some(Ev::Action(nid, key)));
    listen::<(u32, u32), _>(notif, "NotificationClosed", sh.clone(), |(nid, reason)| Some(Ev::Closed(nid, reason)));
    let bus = proxy(&server, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus")?;
    listen::<(String, String, String), _>(bus, "NameOwnerChanged", sh.clone(), |(name, _old, new)| (name.starts_with(':') && new.is_empty()).then_some(Ev::OwnerGone(name)));

    worker::Worker::new(sh, server, &client)?.run(rx);
    Ok(())
}

#[cfg(test)]
mod tests;
