//! Turns job changes into notifications: waits before showing short jobs (like Plasma: 500 ms, or 50 ms for
//! "immediate" ones), limits updates to four a second, and answers the popup's buttons.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::Value;

use crate::model::{ERR_USER_CANCELED, Job, Notice};
use crate::server::{Entry, Ev, Shared, ViewV2, ViewV3};

const DELAY: Duration = Duration::from_millis(500);
const DELAY_IMMEDIATE: Duration = Duration::from_millis(50);
const MIN_GAP: Duration = Duration::from_millis(250);
const IDLE: Duration = Duration::from_secs(3600);
/// How many finished jobs keep their "Open folder" button working.
const KEEP_FINISHED: usize = 64;

pub(crate) struct Worker<'a> {
    sh: Arc<Shared>,
    server: Connection,
    notifications: Proxy<'a>,
    finished: VecDeque<(u32, PathBuf)>,
}

impl Worker<'_> {
    pub fn new(sh: Arc<Shared>, server: Connection, client: &Connection) -> zbus::Result<Worker<'static>> {
        let notifications = crate::proxy(client, "org.freedesktop.Notifications", "/org/freedesktop/Notifications", "org.freedesktop.Notifications")?;
        Ok(Worker { sh, server, notifications, finished: VecDeque::new() })
    }

    pub fn run(mut self, rx: Receiver<Ev>) {
        loop {
            let now = Instant::now();
            let next = self.lock().values().filter_map(|e| e.due).min();
            let wait = next.map(|d| d.saturating_duration_since(now)).unwrap_or(IDLE);
            match rx.recv_timeout(wait) {
                Ok(ev) => self.handle(ev),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            self.flush();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Entry>> {
        self.sh.jobs.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn handle(&mut self, ev: Ev) {
        match ev {
            Ev::Changed(id) => self.schedule(id),
            Ev::Action(nid, key) => self.action(nid, &key),
            Ev::Closed(nid, reason) => {
                self.finished.retain(|(n, _)| *n != nid);
                // 1 = expired, 2 = closed by the user: a running job stops updating its popup until it ends
                if reason == 1 || reason == 2 {
                    for e in self.lock().values_mut().filter(|e| e.nid == nid && !e.job.is_done()) {
                        e.hidden = true;
                        e.nid = 0;
                    }
                }
            }
            Ev::OwnerGone(name) => {
                let ids: Vec<u32> = self.lock().iter().filter(|(_, e)| e.owner == name && !e.job.is_done()).map(|(id, _)| *id).collect();
                for id in ids {
                    // the app quit or crashed mid-job: end it quietly, as Plasma does
                    if let Some(e) = self.lock().get_mut(&id) {
                        e.job.terminate(ERR_USER_CANCELED, "");
                    }
                    self.schedule(id);
                }
            }
        }
    }

    /// When to (re)draw this job's notification.
    fn schedule(&mut self, id: u32) {
        let now = Instant::now();
        let mut drop_silently = None;
        {
            let mut jobs = self.lock();
            let Some(e) = jobs.get_mut(&id) else { return };
            let done = e.job.is_done();
            if !e.shown {
                if done && !e.job.immediate {
                    // ended before it was worth showing (a quick copy, a stat): no notification at all
                    drop_silently = Some((e.path.clone(), e.v3));
                } else {
                    let first = e.created + if e.job.immediate { DELAY_IMMEDIATE } else { DELAY };
                    // an immediate job shows as soon as it has a heading, or when it ends
                    e.due = Some(if done || (e.job.immediate && !e.job.title.is_empty()) { now } else { e.due.unwrap_or(first) });
                }
            } else if done {
                e.due = Some(now);
            } else if e.due.is_none() {
                e.due = Some(e.last.map(|l| (l + MIN_GAP).max(now)).unwrap_or(now));
            }
            if drop_silently.is_some() {
                jobs.remove(&id);
            }
        }
        if let Some((path, v3)) = drop_silently {
            self.unregister(&path, v3);
        }
    }

    fn flush(&mut self) {
        let now = Instant::now();
        let due: Vec<u32> = self.lock().iter().filter(|(_, e)| e.due.is_some_and(|d| d <= now)).map(|(id, _)| *id).collect();
        for id in due {
            let Some((job, nid, hidden)) = self.lock().get_mut(&id).map(|e| {
                e.due = None;
                (e.job.clone(), e.nid, e.hidden)
            }) else {
                continue;
            };
            let new_nid = self.show(&job, nid, hidden);
            let finished = {
                let mut jobs = self.lock();
                let Some(e) = jobs.get_mut(&id) else { continue };
                e.nid = new_nid;
                e.shown = true;
                e.last = Some(Instant::now());
                // (a change that arrived while drawing is already queued as an event and schedules the next draw)
                if job.is_done() { jobs.remove(&id).map(|e| (e.path, e.v3)) } else { None }
            };
            if let Some((path, v3)) = finished {
                self.unregister(&path, v3);
            }
        }
    }

    /// Draw (or close) the job's notification; returns the notification id now showing it (0 = none).
    fn show(&mut self, job: &Job, nid: u32, hidden: bool) -> u32 {
        let Some(n) = job.render() else {
            if nid != 0 {
                let _ = self.notifications.call::<_, _, ()>("CloseNotification", &(nid,));
            }
            return 0;
        };
        if hidden && !job.is_done() {
            return 0;
        }
        match self.notify(job, &n, if hidden { 0 } else { nid }) {
            Ok(id) => {
                if job.is_done() {
                    if let Some(p) = job.dest_path() {
                        self.finished.push_back((id, p));
                        while self.finished.len() > KEEP_FINISHED {
                            self.finished.pop_front();
                        }
                    }
                }
                id
            }
            Err(e) => {
                eprintln!("notify-jobs: could not show a job: {e}");
                nid
            }
        }
    }

    fn notify(&self, job: &Job, n: &Notice, replaces: u32) -> zbus::Result<u32> {
        let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
        if !job.desktop_entry.is_empty() {
            hints.insert("desktop-entry", Value::from(job.desktop_entry.as_str()));
        }
        hints.insert("category", Value::from(n.category));
        hints.insert("resident", Value::from(n.resident));
        hints.insert("urgency", Value::from(1u8));
        if let Some(p) = n.percent {
            hints.insert("value", Value::from(p));
        }
        let actions: Vec<&str> = n.actions.iter().flat_map(|(k, label)| [*k, label.as_str()]).collect();
        let app = if job.app_name.is_empty() { "File operation" } else { job.app_name.as_str() };
        let icon = if job.app_icon.is_empty() { "system-file-manager" } else { job.app_icon.as_str() };
        self.notifications.call("Notify", &(app, replaces, icon, n.summary.as_str(), n.body.as_str(), actions, hints, n.expire_ms))
    }

    fn action(&mut self, nid: u32, key: &str) {
        let running = self.lock().values().find(|e| e.nid == nid && !e.job.is_done()).map(|e| (e.path.clone(), e.v3));
        if let Some((path, v3)) = running {
            let signal = match key {
                "cancel" => "cancelRequested",
                "suspend" => "suspendRequested",
                "resume" => "resumeRequested",
                _ => return,
            };
            let iface = if v3 { "org.kde.JobViewV3" } else { "org.kde.JobViewV2" };
            if let Err(e) = self.server.emit_signal(None::<&str>, path.as_str(), iface, signal, &()) {
                eprintln!("notify-jobs: could not pass on {signal}: {e}");
            }
            return;
        }
        if key != "default" && key != "open" {
            return;
        }
        if let Some((_, p)) = self.finished.iter().find(|(n, _)| *n == nid) {
            let dir = if p.is_dir() { p.clone() } else { p.parent().map(PathBuf::from).unwrap_or_else(|| p.clone()) };
            std::thread::spawn(move || {
                if let Err(e) = std::process::Command::new("xdg-open").arg(&dir).stdin(std::process::Stdio::null()).status() {
                    eprintln!("notify-jobs: could not open {}: {e}", dir.display());
                }
            });
        }
    }

    fn unregister(&self, path: &str, v3: bool) {
        let os = self.server.object_server();
        let _ = if v3 { os.remove::<ViewV3, _>(path) } else { os.remove::<ViewV2, _>(path) };
    }
}
