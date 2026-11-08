//! `org.freedesktop.Notifications` on the session bus. Notifications go straight through to Quickshell on the private bus;
//! inhibitions (`Inhibit`, `UnInhibit`, `Inhibited`: Plasma's extension, used by apps that present or share the screen) are
//! kept here and handed to notifd, which holds popups while any app inhibits them, as for Do Not Disturb.

use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use zbus::blocking::Connection;
use zbus::message::Header;
use zbus::zvariant::OwnedValue;
use zbus::{fdo, interface};

pub const NAME: &str = "org.freedesktop.Notifications";
pub const PATH: &str = "/org/freedesktop/Notifications";

/// One app asking for notifications to be held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inhibition {
    pub sender: String,
    pub app: String,
    pub reason: String,
}

/// Every inhibition by cookie. Shared with the thread that tells notifd, and with the one that sees apps go away.
#[derive(Default)]
pub struct Inhibitions {
    pub next: u32,
    pub by_cookie: BTreeMap<u32, Inhibition>,
}

impl Inhibitions {
    pub fn add(&mut self, sender: &str, app: &str, reason: &str) -> u32 {
        self.next = self.next.wrapping_add(1).max(1);
        while self.by_cookie.contains_key(&self.next) {
            self.next = self.next.wrapping_add(1).max(1);
        }
        self.by_cookie.insert(self.next, Inhibition { sender: sender.into(), app: app.into(), reason: reason.into() });
        self.next
    }
    /// What notifd is told: who inhibits, and why.
    pub fn list(&self) -> serde_json::Value {
        serde_json::Value::Array(self.by_cookie.values().map(|i| serde_json::json!({"app": i.app, "reason": i.reason})).collect())
    }
    /// An app left the bus: its inhibitions end with it. True if any did.
    pub fn drop_sender(&mut self, sender: &str) -> bool {
        let before = self.by_cookie.len();
        self.by_cookie.retain(|_, i| i.sender != sender);
        before != self.by_cookie.len()
    }
}

pub struct Front {
    /// The connection to the private bus, where Quickshell owns the name.
    pub inner: Connection,
    pub inhibitions: Arc<Mutex<Inhibitions>>,
    /// Wakes the thread that tells notifd (and announces `Inhibited`).
    pub changed: Sender<()>,
}

impl Front {
    fn forward<B, R>(&self, method: &str, body: &B) -> fdo::Result<R>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
        R: for<'d> serde::Deserialize<'d> + zbus::zvariant::Type,
    {
        let reply = self.inner.call_method(Some(NAME), PATH, Some(NAME), method, body).map_err(|e| fdo::Error::Failed(format!("notifd: {e}")))?;
        reply.body().deserialize::<R>().map_err(|e| fdo::Error::Failed(format!("notifd: {e}")))
    }
}

/// The app name to show for an inhibition: its desktop entry, else what it gave as its reason's owner, else its bus name.
fn app_label(desktop_entry: &str, hints: &HashMap<String, OwnedValue>, sender: &str) -> String {
    if !desktop_entry.is_empty() {
        return desktop_entry.to_string();
    }
    hints.get("x-kde-appname").and_then(|v| String::try_from(v.clone()).ok()).filter(|s| !s.is_empty()).unwrap_or_else(|| sender.to_string())
}

#[interface(name = "org.freedesktop.Notifications")]
impl Front {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> fdo::Result<u32> {
        self.forward("Notify", &(app_name, replaces_id, app_icon, summary, body, actions, hints, expire_timeout))
    }

    fn close_notification(&self, id: u32) -> fdo::Result<()> {
        self.forward::<_, ()>("CloseNotification", &(id,))
    }

    fn get_capabilities(&self) -> fdo::Result<Vec<String>> {
        let mut caps: Vec<String> = self.forward("GetCapabilities", &())?;
        if !caps.iter().any(|c| c == "inhibitions") {
            caps.push("inhibitions".into());
        }
        Ok(caps)
    }

    #[zbus(out_args("name", "vendor", "version", "spec_version"))]
    fn get_server_information(&self) -> fdo::Result<(String, String, String, String)> {
        self.forward("GetServerInformation", &())
    }

    /// Hold notifications until `UnInhibit(cookie)`, or until the caller leaves the bus.
    fn inhibit(&self, #[zbus(header)] hdr: Header<'_>, desktop_entry: String, reason: String, hints: HashMap<String, OwnedValue>) -> fdo::Result<u32> {
        let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
        let app = app_label(&desktop_entry, &hints, &sender);
        let cookie = self.inhibitions.lock().unwrap().add(&sender, &app, &reason);
        eprintln!("notify-bus: {app} holds notifications ({reason})");
        let _ = self.changed.send(());
        Ok(cookie)
    }

    fn un_inhibit(&self, #[zbus(header)] hdr: Header<'_>, cookie: u32) -> fdo::Result<()> {
        let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
        let mut inh = self.inhibitions.lock().unwrap();
        match inh.by_cookie.get(&cookie) {
            None => return Ok(()), // already gone (its app left, or a second UnInhibit): nothing to do
            Some(i) if i.sender != sender => return Err(fdo::Error::AccessDenied("that inhibition belongs to another program".into())),
            Some(_) => {}
        }
        inh.by_cookie.remove(&cookie);
        drop(inh);
        let _ = self.changed.send(());
        Ok(())
    }

    #[zbus(property)]
    fn inhibited(&self) -> bool {
        !self.inhibitions.lock().unwrap().by_cookie.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookies_are_unique_and_end_with_their_app() {
        let mut i = Inhibitions::default();
        let a = i.add(":1.5", "org.kde.okular", "presenting");
        let b = i.add(":1.6", "obs", "recording");
        assert_ne!(a, b);
        assert_eq!(i.list(), serde_json::json!([{"app": "org.kde.okular", "reason": "presenting"}, {"app": "obs", "reason": "recording"}]));
        assert!(i.drop_sender(":1.5"));
        assert!(!i.drop_sender(":1.5"));
        assert_eq!(i.by_cookie.len(), 1);
        i.next = u32::MAX;
        assert_eq!(i.add(":1.7", "x", ""), 1, "cookies wrap past zero, skipping 0 and ones in use (1 is free again)");
    }

    #[test]
    fn an_inhibition_is_named_after_its_app() {
        let hints = HashMap::new();
        assert_eq!(app_label("org.kde.okular", &hints, ":1.9"), "org.kde.okular");
        assert_eq!(app_label("", &hints, ":1.9"), ":1.9");
    }
}
