//! Repeats of one notification sent in a burst become one notification.
//!
//! Chromium-based browsers (Brave, Chromium) send every `showNotification` of a page as its own `Notify` call. When a page shows
//! the same notification several times before the first call is answered (a backlog of push messages, a tag shown again), each
//! call goes out with `replaces_id` 0: the server makes a new popup for each, the browser keeps only the last id, and it deletes
//! the earlier calls' icon files. The earlier popups then stay on screen with a missing icon, and the browser never closes them.
//!
//! So a `Notify` without `replaces_id` that repeats one the same program (bus connection) sent a moment ago, with the same text,
//! actions and site, is sent on as a replacement of that one, and the caller gets that one's id back. Different programs never
//! merge (each `notify-send` is a connection of its own), and neither do different texts. If the earlier one has already closed,
//! Quickshell makes a new notification for the replacement as it would for any unknown id.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

/// How long a notification counts as just sent.
pub const WINDOW: Duration = Duration::from_secs(2);

/// The parts of a `Notify` call that make two calls the same notification.
pub struct Call<'a> {
    pub sender: &'a str,
    pub app_name: &'a str,
    pub summary: &'a str,
    pub body: &'a str,
    pub actions: &'a [String],
    /// The site (`x-kde-origin-name`) and the desktop entry, if given.
    pub origin: &'a str,
    pub desktop_entry: &'a str,
}

impl Call<'_> {
    fn key(&self) -> u64 {
        let mut h = DefaultHasher::new();
        (self.sender, self.app_name, self.summary, self.body, self.actions, self.origin, self.desktop_entry).hash(&mut h);
        h.finish()
    }
}

/// Recent notifications by call key: their id, and when they were sent.
#[derive(Default)]
pub struct Recent {
    by_key: HashMap<u64, (u32, Instant)>,
}

impl Recent {
    /// The id a new call (`replaces_id` 0) should replace: the same notification sent within `WINDOW`, else 0.
    pub fn replaces(&mut self, call: &Call, now: Instant) -> u32 {
        self.by_key.retain(|_, (_, t)| now.duration_since(*t) < WINDOW);
        self.by_key.get(&call.key()).map_or(0, |(id, _)| *id)
    }
    /// The server answered `id` for this call.
    pub fn sent(&mut self, call: &Call, id: u32, now: Instant) {
        if id != 0 {
            self.by_key.insert(call.key(), (id, now));
        }
    }
    /// The app closed `id` itself: a repeat after that is a new notification.
    pub fn closed(&mut self, id: u32) {
        self.by_key.retain(|_, (i, _)| *i != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call<'a>(sender: &'a str, body: &'a str) -> Call<'a> {
        Call { sender, app_name: "Brave", summary: "FediNSFW", body, actions: &[], origin: "fedinsfw.example", desktop_entry: "brave" }
    }

    #[test]
    fn a_repeat_from_the_same_program_replaces_the_first() {
        let (mut r, t) = (Recent::default(), Instant::now());
        let c = call(":1.5", "3 notifications.");
        assert_eq!(r.replaces(&c, t), 0);
        r.sent(&c, 7, t);
        for _ in 0..3 {
            assert_eq!(r.replaces(&c, t + Duration::from_millis(5)), 7);
            r.sent(&c, 7, t + Duration::from_millis(5));
        }
    }

    #[test]
    fn other_programs_other_text_and_later_repeats_are_new() {
        let (mut r, t) = (Recent::default(), Instant::now());
        r.sent(&call(":1.5", "3 notifications."), 7, t);
        assert_eq!(r.replaces(&call(":1.6", "3 notifications."), t), 0, "another program");
        assert_eq!(r.replaces(&call(":1.5", "4 notifications."), t), 0, "other text");
        assert_eq!(r.replaces(&call(":1.5", "3 notifications."), t + WINDOW), 0, "too late");
    }

    #[test]
    fn a_closed_notification_is_not_replaced() {
        let (mut r, t) = (Recent::default(), Instant::now());
        let c = call(":1.5", "done");
        r.sent(&c, 9, t);
        r.closed(9);
        assert_eq!(r.replaces(&c, t), 0);
    }
}
