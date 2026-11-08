//! A KDE job (a file copy, move, delete, extraction...) as apps report it over org.kde.JobViewV2/V3, and the
//! notification that shows it. No D-Bus here: the bridge converts the wire values into [`Val`] and calls in.

use std::collections::HashMap;
use std::path::PathBuf;

/// KIO's "the user cancelled" error (KJob::KilledJobError): the job ends without a notification.
pub const ERR_USER_CANCELED: u32 = 1;
/// Job capabilities (requestView's `capabilities`).
pub const CAP_KILLABLE: i32 = 0x1;
pub const CAP_SUSPENDABLE: i32 = 0x2;

/// A property value as it arrived on the bus.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    U(u64),
    I(i64),
    S(String),
    B(bool),
}

impl Val {
    fn u64(&self) -> Option<u64> {
        match self {
            Val::U(v) => Some(*v),
            Val::I(v) if *v >= 0 => Some(*v as u64),
            _ => None,
        }
    }
    fn str(&self) -> Option<&str> {
        if let Val::S(s) = self { Some(s) } else { None }
    }
    fn bool(&self) -> Option<bool> {
        match self {
            Val::B(b) => Some(*b),
            Val::U(v) => Some(*v != 0),
            Val::I(v) => Some(*v != 0),
            Val::S(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Amounts {
    pub bytes: u64,
    pub files: u64,
    pub dirs: u64,
    pub items: u64,
}

impl Amounts {
    fn slot(&mut self, unit: &str) -> Option<&mut u64> {
        match unit {
            "bytes" => Some(&mut self.bytes),
            "files" => Some(&mut self.files),
            "dirs" => Some(&mut self.dirs),
            "items" => Some(&mut self.items),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Running,
    Suspended,
    Done { error: u32, message: String },
}

#[derive(Clone, Debug)]
pub struct Job {
    pub desktop_entry: String,
    pub app_name: String,
    pub app_icon: String,
    pub killable: bool,
    pub suspendable: bool,
    /// Shown at once (after 50 ms) instead of after 500 ms, and shown even if it ends before then.
    pub immediate: bool,
    /// Disappears without a "finished" notification when it succeeds.
    pub transient: bool,
    pub title: String,
    pub info: String,
    pub percent: Option<u32>,
    pub speed: u64,
    pub processed: Amounts,
    pub total: Amounts,
    pub desc: [(String, String); 2],
    pub dest_url: String,
    /// The error a JobViewV2 client sets before it calls terminate.
    pub pending_error: u32,
    pub state: State,
}

/// What the notification shows. Action keys: "cancel", "suspend", "resume" while running; "default" and "open" when done.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub summary: String,
    pub body: String,
    pub percent: Option<i32>,
    pub actions: Vec<(&'static str, String)>,
    pub category: &'static str,
    /// Stays on screen when an action is used (pause/resume must not close it).
    pub resident: bool,
    /// freedesktop expire_timeout: 0 = until closed, -1 = the server's default.
    pub expire_ms: i32,
}

impl Job {
    pub fn new(desktop_entry: &str, capabilities: i32, hints: &HashMap<String, Val>) -> Job {
        let s = |k: &str| hints.get(k).and_then(Val::str).unwrap_or("").to_string();
        let b = |k: &str| hints.get(k).and_then(Val::bool).unwrap_or(false);
        Job {
            desktop_entry: desktop_entry.to_string(),
            app_name: s("application-display-name"),
            app_icon: s("application-icon-name"),
            killable: capabilities & CAP_KILLABLE != 0,
            suspendable: capabilities & CAP_SUSPENDABLE != 0,
            immediate: b("immediate"),
            transient: b("transient"),
            title: String::new(),
            info: String::new(),
            percent: None,
            speed: 0,
            processed: Amounts::default(),
            total: Amounts::default(),
            desc: Default::default(),
            dest_url: String::new(),
            pending_error: 0,
            state: State::Running,
        }
    }

    pub fn is_done(&self) -> bool {
        matches!(self.state, State::Done { .. })
    }

    /// JobViewV3.update: any subset of the known properties; unknown ones are ignored.
    pub fn update(&mut self, props: &HashMap<String, Val>) {
        for (k, v) in props {
            match k.as_str() {
                "title" => self.title = v.str().unwrap_or("").to_string(),
                "infoMessage" => self.info = v.str().unwrap_or("").to_string(),
                "percent" => self.percent = v.u64().map(|p| p.min(100) as u32),
                "speed" => self.speed = v.u64().unwrap_or(0),
                "destUrl" => self.dest_url = v.str().unwrap_or("").to_string(),
                "suspended" => self.set_suspended(v.bool().unwrap_or(false)),
                "descriptionLabel1" => self.desc[0].0 = v.str().unwrap_or("").to_string(),
                "descriptionValue1" => self.desc[0].1 = v.str().unwrap_or("").to_string(),
                "descriptionLabel2" => self.desc[1].0 = v.str().unwrap_or("").to_string(),
                "descriptionValue2" => self.desc[1].1 = v.str().unwrap_or("").to_string(),
                other => {
                    let (amounts, unit) = if let Some(u) = other.strip_prefix("processed") {
                        (&mut self.processed, u)
                    } else if let Some(u) = other.strip_prefix("total") {
                        (&mut self.total, u)
                    } else {
                        continue;
                    };
                    if let (Some(slot), Some(n)) = (amounts.slot(&unit.to_lowercase()), v.u64()) {
                        *slot = n;
                    }
                }
            }
        }
    }

    // JobViewV2: one setter per field. Its "infoMessage" is really the heading ("Copying"), as Plasma treats it.
    pub fn set_info_message(&mut self, m: &str) {
        self.title = m.to_string();
    }
    pub fn set_amount(&mut self, processed: bool, amount: u64, unit: &str) {
        let a = if processed { &mut self.processed } else { &mut self.total };
        if let Some(slot) = a.slot(unit) {
            *slot = amount;
        }
    }
    pub fn set_description(&mut self, n: u32, label: &str, value: &str) {
        if let Some(d) = self.desc.get_mut(n as usize) {
            *d = (label.to_string(), value.to_string());
        }
    }
    pub fn set_suspended(&mut self, on: bool) {
        if !self.is_done() {
            self.state = if on { State::Suspended } else { State::Running };
        }
    }
    pub fn terminate(&mut self, error: u32, message: &str) {
        self.state = State::Done { error, message: message.to_string() };
    }

    fn percent_now(&self) -> Option<u32> {
        self.percent.or_else(|| (self.total.bytes > 0).then(|| (self.processed.bytes.min(self.total.bytes) * 100 / self.total.bytes) as u32))
    }

    /// Where the job put things, as a local path (for "Open folder").
    pub fn dest_path(&self) -> Option<PathBuf> {
        let from_url = local_path(&self.dest_url);
        // a single file and no destination URL: the second description field is usually "Destination"
        from_url.or_else(|| if self.total.files == 1 { local_path(&self.desc[1].1).and_then(|p| p.parent().map(PathBuf::from)) } else { None })
    }

    fn where_line(&self) -> Option<String> {
        if let Some(p) = self.dest_path() {
            return Some(format!("To {}", esc(&tilde(&p.to_string_lossy()))));
        }
        if !self.dest_url.is_empty() {
            return Some(format!("To {}", esc(&pretty_url(&self.dest_url))));
        }
        let lines: Vec<String> = self.desc.iter().filter(|(_, v)| !v.is_empty()).map(|(l, v)| if l.is_empty() { esc(&pretty_url(v)) } else { format!("{}: {}", esc(l.trim_end_matches(':')), esc(&pretty_url(v))) }).collect();
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    fn count_text(&self, a: &Amounts, of: Option<&Amounts>) -> Option<String> {
        let pick = |x: &Amounts| [(x.files, "file", "files"), (x.dirs, "folder", "folders"), (x.items, "item", "items")];
        let totals = pick(of.unwrap_or(a));
        let done = pick(a);
        for i in 0..3 {
            let (t, one, many) = totals[i];
            if of.is_some() && t > 1 {
                return Some(format!("{} of {} {}", done[i].0.min(t), t, many));
            }
            if of.is_none() && t > 0 {
                return Some(format!("{} {}", t, if t == 1 { one } else { many }));
            }
        }
        None
    }

    /// The notification for the job's current state; None means "no notification" (a cancelled job, or a
    /// transient one that succeeded), so any popup it has should close.
    pub fn render(&self) -> Option<Notice> {
        let heading = if !self.title.is_empty() { self.title.clone() } else if !self.app_name.is_empty() { self.app_name.clone() } else { "File operation".to_string() };
        let mut lines: Vec<String> = Vec::new();
        match &self.state {
            State::Done { error, message } => {
                let error = if *error == 0 { self.pending_error } else { *error };
                if error == ERR_USER_CANCELED || (error == 0 && self.transient) {
                    return None;
                }
                if error != 0 {
                    lines.push(esc(if message.is_empty() { "Something went wrong" } else { message }));
                    lines.extend(self.where_line());
                    return Some(Notice { summary: format!("{heading} failed"), body: lines.join("\n"), percent: None, actions: vec![], category: "transfer.error", resident: false, expire_ms: -1 });
                }
                let mut what = Vec::new();
                what.extend(self.count_text(&self.total, None));
                if self.total.bytes > 0 {
                    what.push(bytes(self.total.bytes));
                }
                if !what.is_empty() {
                    lines.push(what.join(", "));
                }
                lines.extend(self.where_line());
                let actions = if self.dest_path().is_some() { vec![("default", "Open".to_string()), ("open", "Open folder".to_string())] } else { vec![] };
                Some(Notice { summary: finished(&heading), body: lines.join("\n"), percent: None, actions, category: "transfer.complete", resident: false, expire_ms: -1 })
            }
            state => {
                let mut parts = Vec::new();
                parts.extend(self.count_text(&self.processed, Some(&self.total)));
                if self.total.bytes > 0 {
                    parts.push(format!("{} of {}", bytes(self.processed.bytes.min(self.total.bytes)), bytes(self.total.bytes)));
                }
                let paused = *state == State::Suspended;
                if paused {
                    parts.push("paused".into());
                } else if self.speed > 0 {
                    parts.push(format!("{}/s", bytes(self.speed)));
                    if self.total.bytes > self.processed.bytes {
                        parts.push(format!("{} left", duration((self.total.bytes - self.processed.bytes) / self.speed)));
                    }
                }
                if !parts.is_empty() {
                    lines.push(parts.join(" · "));
                }
                lines.extend(self.where_line());
                if !self.info.is_empty() {
                    lines.push(esc(&self.info));
                }
                let mut actions = Vec::new();
                if self.suspendable {
                    actions.push(if paused { ("resume", "Resume".to_string()) } else { ("suspend", "Pause".to_string()) });
                }
                if self.killable {
                    actions.push(("cancel", "Cancel".to_string()));
                }
                let summary = if paused { format!("{heading} (paused)") } else { heading };
                Some(Notice { summary, body: lines.join("\n"), percent: self.percent_now().map(|p| p as i32), actions, category: "transfer", resident: true, expire_ms: 0 })
            }
        }
    }
}

/// "Copying" -> "Copied": the heading of a finished job, for the common KIO titles; anything else gets "... finished".
pub fn finished(title: &str) -> String {
    const PAST: [(&str, &str); 14] = [
        ("Copying", "Copied"),
        ("Moving", "Moved"),
        ("Deleting", "Deleted"),
        ("Trashing", "Moved to Trash"),
        ("Restoring", "Restored"),
        ("Extracting", "Extracted"),
        ("Compressing", "Compressed"),
        ("Creating", "Created"),
        ("Renaming", "Renamed"),
        ("Linking", "Linked"),
        ("Downloading", "Downloaded"),
        ("Uploading", "Uploaded"),
        ("Transferring", "Transferred"),
        ("Examining", "Examined"),
    ];
    let first = title.split_whitespace().next().unwrap_or("");
    match PAST.iter().find(|(ing, _)| *ing == first) {
        Some((ing, past)) => format!("{past}{}", &title[ing.len()..]),
        None => format!("{title} finished"),
    }
}

/// Body text is markup: escape what the app sent.
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Sizes the way KDE shows them (binary units).
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["KiB", "MiB", "GiB", "TiB", "PiB"];
    if n < 1024 {
        return format!("{n} B");
    }
    let mut v = n as f64 / 1024.0;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if v >= 100.0 { format!("{v:.0} {}", UNITS[u]) } else { format!("{v:.1} {}", UNITS[u]) }
}

pub fn duration(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs} s"),
        60..3600 => format!("{} min", secs.div_ceil(60)),
        _ => format!("{} h {} min", secs / 3600, (secs % 3600) / 60),
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A local path from "file:///..." or a plain absolute path; None for remote URLs and non-paths.
pub fn local_path(s: &str) -> Option<PathBuf> {
    let p = if let Some(rest) = s.strip_prefix("file://") { percent_decode(rest.strip_prefix("localhost").unwrap_or(rest)) } else if s.starts_with('/') { s.to_string() } else { return None };
    let p = p.trim_end_matches("/.").to_string();
    (p.starts_with('/')).then(|| PathBuf::from(if p.len() > 1 { p.trim_end_matches('/').to_string() } else { p }))
}

fn tilde(p: &str) -> String {
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && h != "/" && (p == h || p.starts_with(&format!("{h}/"))) => format!("~{}", &p[h.len()..]),
        _ => p.to_string(),
    }
}

/// A URL for display: local paths with ~, remote ones without the user name or password.
pub fn pretty_url(s: &str) -> String {
    if let Some(p) = local_path(s) {
        return tilde(&p.to_string_lossy());
    }
    match s.split_once("://") {
        Some((scheme, rest)) => {
            let (host, path) = rest.split_once('/').map(|(h, p)| (h, format!("/{p}"))).unwrap_or((rest, String::new()));
            let host = host.rsplit_once('@').map(|(_, h)| h).unwrap_or(host);
            format!("{scheme}://{host}{}", percent_decode(&path))
        }
        None => s.to_string(),
    }
}
