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

/// Capabilities notifd adds to Quickshell's own. `inhibitions` is the KDE inhibit extension. `x-kde-origin-name` tells a browser
/// to send a web page's site as the `x-kde-origin-name` hint (which the `origin` a rule matches on comes from); without it
/// Chromium and Brave fold the site into the notification text, and every site rule falls through to the default group.
/// `body-images` makes a browser send the large picture of an image notification (the web `image` option) as an `<img>` in the
/// body, which `take_body_image` turns into the `x-notifi-image` hint for the popup.
pub const EXTRA_CAPABILITIES: &[&str] = &["inhibitions", "x-kde-origin-name", "body-images"];

/// The hint a body's local picture is passed to notifd in (a file path).
pub const IMAGE_HINT: &str = "x-notifi-image";

/// Remove every `<img ...>` tag from a notification body and return the first one that names a local file (`file:///...`), as a
/// path. Remote pictures are dropped and never fetched; so are paths with `..` or a NUL.
pub fn take_body_image(body: &str) -> (String, Option<String>) {
    let lower = body.to_ascii_lowercase(); // same byte offsets as `body`
    let (mut out, mut at, mut found) = (String::with_capacity(body.len()), 0, None);
    while let Some(rel) = lower[at..].find("<img") {
        let start = at + rel;
        let after = lower.as_bytes().get(start + 4).copied().unwrap_or(b'>');
        let Some(end) = lower[start..].find('>').map(|e| start + e + 1).filter(|_| after.is_ascii_whitespace() || after == b'/' || after == b'>') else {
            out.push_str(&body[at..start + 4]); // "<imgur" and the like: not a picture tag
            at = start + 4;
            continue;
        };
        out.push_str(&body[at..start]);
        if found.is_none() {
            found = attr_src(&body[start..end]).and_then(|u| local_path(&u));
        }
        at = end;
    }
    out.push_str(&body[at..]);
    (out.trim_matches('\n').to_string(), found)
}

/// The value of `src="..."` (or `'...'`) in one tag.
fn attr_src(tag: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = lower[from..].find("src") {
        let i = from + rel;
        from = i + 3;
        let rest = tag[i + 3..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let rest = rest.trim_start();
        let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let inner = &rest[1..];
        return inner.find(quote).map(|e| inner[..e].to_string());
    }
    None
}

/// `file:///tmp/x%20y/image.png` -> `/tmp/x y/image.png`, when that is a plain absolute path.
fn local_path(url: &str) -> Option<String> {
    let enc = url.strip_prefix("file://")?;
    if !enc.starts_with('/') || enc.len() > 1024 {
        return None;
    }
    let (mut bytes, raw) = (Vec::with_capacity(enc.len()), enc.as_bytes());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            let hex = std::str::from_utf8(raw.get(i + 1..i + 3)?).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            bytes.push(raw[i]);
            i += 1;
        }
    }
    let path = String::from_utf8(bytes).ok()?;
    (!path.contains('\0') && !path.split('/').any(|c| c == "..")).then_some(path)
}

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
fn add_extra_capabilities(caps: &mut Vec<String>) {
    for extra in EXTRA_CAPABILITIES {
        if !caps.iter().any(|c| c == extra) {
            caps.push((*extra).into());
        }
    }
}

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
        mut hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> fdo::Result<u32> {
        // only this proxy sets the picture hint (from a body `<img>`), so one an app sends itself is dropped
        hints.remove(IMAGE_HINT);
        let (body, image) = take_body_image(&body);
        if let Some(path) = image {
            hints.insert(IMAGE_HINT.into(), zbus::zvariant::Value::from(path).try_into().expect("a string is a valid value"));
        }
        self.forward("Notify", &(app_name, replaces_id, app_icon, summary, body, actions, hints, expire_timeout))
    }

    fn close_notification(&self, id: u32) -> fdo::Result<()> {
        self.forward::<_, ()>("CloseNotification", &(id,))
    }

    fn get_capabilities(&self) -> fdo::Result<Vec<String>> {
        let mut caps: Vec<String> = self.forward("GetCapabilities", &())?;
        add_extra_capabilities(&mut caps);
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

    #[test]
    fn browsers_are_told_to_send_the_site_as_a_hint() {
        let mut caps = vec!["actions".to_string(), "inhibitions".to_string()];
        add_extra_capabilities(&mut caps);
        add_extra_capabilities(&mut caps);
        assert_eq!(caps, ["actions", "inhibitions", "x-kde-origin-name", "body-images"], "added once, existing ones kept");
    }

    #[test]
    fn a_browsers_picture_moves_from_the_body_to_a_hint() {
        let body = "hello <b>there</b>\n<img src=\"file:///tmp/org.chromium.Chromium.scoped_dir.abc/image%20one.png\" alt=\"\"/>";
        assert_eq!(take_body_image(body), ("hello <b>there</b>".into(), Some("/tmp/org.chromium.Chromium.scoped_dir.abc/image one.png".into())));
        // exactly what Chromium 153 sent for a web notification with an `image`, captured from a real run
        let real = "hey &lt;b&gt;there&lt;/b&gt; &amp; more\n<img src=\"file:///tmp/org.chromium.Chromium.scoped_dir.oenr8p/image.png\" alt=\"\"/>";
        assert_eq!(take_body_image(real), ("hey &lt;b&gt;there&lt;/b&gt; &amp; more".into(), Some("/tmp/org.chromium.Chromium.scoped_dir.oenr8p/image.png".into())));
        assert_eq!(take_body_image("no picture"), ("no picture".into(), None));
    }

    #[test]
    fn only_local_pictures_are_kept_and_every_tag_is_dropped() {
        let (body, img) = take_body_image("a <IMG SRC='https://evil.example/x.png'> b <img src=\"file:///tmp/ok.png\"> c");
        assert_eq!((body.as_str(), img.as_deref()), ("a  b  c", Some("/tmp/ok.png")), "the remote one is dropped, the later local one still found");
        for bad in ["file:///tmp/../etc/x.png", "file://host/x.png", "http://x/y.png", "file:///tmp/a%00b.png", "/tmp/x.png", "file:///tmp/%zz"] {
            assert_eq!(take_body_image(&format!("<img src=\"{bad}\">")), ("".into(), None), "{bad}");
        }
        assert_eq!(take_body_image("see <imgur> and <img"), ("see <imgur> and <img".into(), None), "not picture tags: left alone");
    }
}
