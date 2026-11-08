//! Blocking client for the notifd control socket.
//!
//! ```no_run
//! use notify_client::Client;
//! let mut c = Client::connect_default()?;
//! println!("daemon {} (protocol {})", c.hello().daemon, c.hello().protocol);
//! let status = c.status()?;
//! println!("Do Not Disturb: {}", status.dnd);
//! # Ok::<(), notify_client::Error>(())
//! ```
//!
//! The connection checks the protocol version in the daemon's `hello` and refuses an incompatible daemon with
//! [`Error::VersionMismatch`]. Events received while waiting for an answer are queued and returned by
//! [`Client::next_event`], so nothing is lost.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use notify_protocol::*;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// How long a request may take before [`Error::Timeout`].
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
/// Largest message the client will buffer (the daemon refuses requests above 1 MiB).
const MAX_LINE: usize = 8 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot reach the daemon at {path}: {source}")]
    Connect { path: PathBuf, source: std::io::Error },
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("the daemon closed the connection")]
    Closed,
    #[error("timed out waiting for the daemon")]
    Timeout,
    #[error("the daemon sent something unexpected: {0}")]
    Protocol(String),
    #[error("daemon speaks protocol {daemon}, this client speaks {client}; run `notify update`")]
    VersionMismatch { daemon: u32, client: u32 },
    #[error("the daemon refused this connection: {0}")]
    Refused(String),
    /// The daemon understood the request and said no.
    #[error("{message} ({code})")]
    Daemon { code: String, message: String },
    #[error("bad json: {0}")]
    Json(#[from] serde_json::Error),
}

/// A peer that hung up shows up as a few different OS errors; to callers they all mean "closed".
fn io_error(e: std::io::Error) -> Error {
    match e.kind() {
        ErrorKind::BrokenPipe | ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted | ErrorKind::UnexpectedEof => Error::Closed,
        _ => Error::Io(e),
    }
}

impl Error {
    /// The daemon's error code, when this is a [`Error::Daemon`].
    pub fn code(&self) -> Option<&str> {
        match self {
            Error::Daemon { code, .. } => Some(code),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// `$NOTIFD_SOCKET`, or `$XDG_RUNTIME_DIR/notifd.sock`. Without `$XDG_RUNTIME_DIR` there is no safe per-user folder (a shared `/tmp` would let other users
/// listen in), so the path points nowhere and the connection error says which variable is missing.
pub fn default_socket_path() -> PathBuf {
    if let Some(p) = std::env::var_os("NOTIFD_SOCKET").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let dir = std::env::var_os("XDG_RUNTIME_DIR").filter(|p| !p.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("XDG_RUNTIME_DIR-is-not-set"));
    dir.join("notifd.sock")
}

pub struct Client {
    stream: UnixStream,
    buf: Vec<u8>,
    next_id: u64,
    events: VecDeque<EventEnvelope>,
    hello: Hello,
}

impl Client {
    pub fn connect_default() -> Result<Client> {
        Client::connect(default_socket_path())
    }

    /// Connect and read the daemon's greeting.
    pub fn connect(path: impl AsRef<Path>) -> Result<Client> {
        let path = path.as_ref();
        let stream = UnixStream::connect(path).map_err(|source| Error::Connect { path: path.to_path_buf(), source })?;
        let mut c = Client {
            stream,
            buf: Vec::new(),
            next_id: 1,
            events: VecDeque::new(),
            hello: Hello { protocol: 0, daemon: String::new(), pid: 0, seq: 0 },
        };
        let first = c.read_incoming(DEFAULT_TIMEOUT)?;
        match first {
            Incoming::Event(env) => match env.event {
                Event::Hello(h) => {
                    if h.protocol != PROTOCOL_VERSION {
                        return Err(Error::VersionMismatch { daemon: h.protocol, client: PROTOCOL_VERSION });
                    }
                    c.hello = Hello { protocol: h.protocol, daemon: h.daemon, pid: h.pid, seq: env.seq };
                }
                Event::Error(e) => return Err(Error::Refused(format!("{} ({})", e.message, e.code))),
                other => return Err(Error::Protocol(format!("expected hello, got {other:?}"))),
            },
            Incoming::Response(r) => return Err(Error::Protocol(format!("expected hello, got a response: {r:?}"))),
        }
        Ok(c)
    }

    /// The greeting: protocol, daemon version, daemon pid and the event sequence at connect time.
    pub fn hello(&self) -> &Hello {
        &self.hello
    }

    // ------------------------------------------------------------ plumbing

    fn read_line(&mut self, timeout: Duration) -> Result<String> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(pos) = self.buf.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = self.buf.drain(..=pos).collect();
                return String::from_utf8(line[..line.len() - 1].to_vec()).map_err(|e| Error::Protocol(e.to_string()));
            }
            if self.buf.len() > MAX_LINE {
                return Err(Error::Protocol("message too large".into()));
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(Error::Timeout);
            }
            self.stream.set_read_timeout(Some(left))?;
            let mut chunk = [0u8; 65536];
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err(Error::Closed),
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => return Err(Error::Timeout),
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(io_error(e)),
            }
        }
    }

    fn read_incoming(&mut self, timeout: Duration) -> Result<Incoming> {
        let line = self.read_line(timeout)?;
        Incoming::parse(&line).map_err(|e| Error::Protocol(format!("{e}: {line}")))
    }

    fn write_line(&mut self, text: &str) -> Result<()> {
        self.stream.write_all(text.as_bytes()).map_err(io_error)?;
        self.stream.write_all(b"\n").map_err(io_error)?;
        Ok(())
    }

    /// Send one request and wait for its answer. Events that arrive meanwhile are queued.
    pub fn call_raw(&mut self, method: &str, params: Value) -> Result<Value> {
        self.call_raw_timeout(method, params, DEFAULT_TIMEOUT)
    }

    pub fn call_raw_timeout(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let req = Request { id, method: method.to_string(), params };
        self.write_line(&serde_json::to_string(&req)?)?;
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.read_incoming(left)? {
                Incoming::Event(env) => self.events.push_back(*env),
                Incoming::Response(r) if r.id == Some(id) => {
                    return match (r.result, r.error) {
                        (_, Some(e)) => Err(Error::Daemon { code: e.code, message: e.message }),
                        (Some(v), None) => Ok(v),
                        (None, None) => Ok(Value::Null),
                    };
                }
                // an answer to a request we did not make (for example a bad_request about a line we never sent): keep waiting
                Incoming::Response(_) => {}
            }
        }
    }

    /// [`call_raw`](Self::call_raw) with typed parameters and result.
    pub fn call<P: Serialize, R: DeserializeOwned>(&mut self, method: &str, params: &P) -> Result<R> {
        let v = self.call_raw(method, serde_json::to_value(params)?)?;
        Ok(serde_json::from_value(v)?)
    }

    // ------------------------------------------------------------ events

    /// Start receiving pushed events. Returns the current status and the event sequence number to continue from.
    pub fn subscribe(&mut self) -> Result<(Status, u64)> {
        let v = self.call_raw("subscribe", json!({}))?;
        let status = serde_json::from_value(v.get("status").cloned().unwrap_or(Value::Null))?;
        let seq = v.get("seq").and_then(Value::as_u64).unwrap_or(0);
        Ok((status, seq))
    }

    pub fn unsubscribe(&mut self) -> Result<()> {
        self.call_raw("unsubscribe", json!({})).map(|_| ())
    }

    /// The next pushed event, waiting up to `timeout`. `Ok(None)` means none arrived in time.
    pub fn next_event(&mut self, timeout: Duration) -> Result<Option<EventEnvelope>> {
        if let Some(e) = self.events.pop_front() {
            return Ok(Some(e));
        }
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.read_incoming(left) {
                Ok(Incoming::Event(e)) => return Ok(Some(*e)),
                Ok(Incoming::Response(_)) => continue,
                Err(Error::Timeout) => return Ok(None),
                Err(e) => return Err(e),
            }
        }
    }

    /// Wait for an event matching `pred`; others are discarded.
    pub fn wait_event(&mut self, timeout: Duration, mut pred: impl FnMut(&Event) -> bool) -> Result<Option<EventEnvelope>> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(None);
            }
            match self.next_event(left)? {
                Some(e) if pred(&e.event) => return Ok(Some(e)),
                Some(_) => continue,
                None => return Ok(None),
            }
        }
    }

    // ------------------------------------------------------------ typed methods

    pub fn ping(&mut self) -> Result<()> {
        self.call_raw("ping", json!({})).map(|_| ())
    }

    pub fn status(&mut self) -> Result<Status> {
        self.call("status", &json!({}))
    }

    pub fn config_status(&mut self) -> Result<ConfigStatus> {
        self.call("configStatus", &json!({}))
    }

    /// The raw file text and the merged config the daemon is running.
    pub fn config_get(&mut self) -> Result<ConfigGet> {
        self.call("configGet", &json!({}))
    }

    /// `mode`: "on", "off" or "toggle".
    pub fn dnd(&mut self, mode: &str) -> Result<Status> {
        self.call("dnd", &json!({ "mode": mode }))
    }

    pub fn mute(&mut self, zone: &str, on: bool) -> Result<Status> {
        self.call("mute", &json!({ "zone": zone, "on": on }))
    }

    /// Turn a focus mode on by name, or off with "off". The daemon refuses an unknown name and lists the ones that exist.
    pub fn focus(&mut self, mode: &str) -> Result<Status> {
        self.call("focus", &json!({ "mode": mode }))
    }

    /// Expand, collapse (`toggle`), `clear` or `scroll` a stack of notifications.
    pub fn stack(&mut self, zone: &str, key: &str, action: &str, delta: i32) -> Result<Status> {
        self.call("stack", &json!({ "zone": zone, "key": key, "action": action, "delta": delta }))
    }

    /// `kind`: "fullscreen" or "share". Normally set by the watchers; exposed for tests and tools.
    pub fn set_auto(&mut self, kind: &str, on: bool) -> Result<Status> {
        self.call("setAuto", &json!({ "kind": kind, "on": on }))
    }

    pub fn history(&mut self, limit: u32) -> Result<Vec<HistoryEntry>> {
        self.call("history", &json!({ "limit": limit }))
    }

    /// Every app that ever sent a notification, most recent first.
    pub fn seen_apps(&mut self) -> Result<Vec<SeenApp>> {
        self.call("seenApps", &json!({}))
    }

    pub fn clear_history(&mut self) -> Result<Status> {
        self.call("clearHistory", &json!({}))
    }

    pub fn dismiss_all(&mut self) -> Result<Status> {
        self.call("dismissAll", &json!({}))
    }

    /// Open, close or toggle the notification center.
    pub fn center(&mut self, action: &str) -> Result<Status> {
        self.call("center", &json!({ "action": action }))
    }

    /// Mark these history entries read (by notification id), or all of them with `None`.
    pub fn mark_read(&mut self, ids: Option<&[i64]>) -> Result<Status> {
        self.call("markRead", &match ids {
            Some(ids) => json!({ "ids": ids }),
            None => json!({}),
        })
    }

    pub fn reply(&mut self, id: i64, text: &str) -> Result<()> {
        self.call_raw("reply", json!({ "id": id, "text": text })).map(|_| ())
    }

    /// Check a config (an object) without applying it.
    pub fn validate(&mut self, config: &Config) -> Result<ValidateResult> {
        self.call("validate", &json!({ "config": config }))
    }

    /// Check raw rules.json text without applying it (works for text that is not valid JSON, too).
    pub fn validate_text(&mut self, text: &str) -> Result<ValidateResult> {
        self.call("validate", &json!({ "text": text }))
    }

    pub fn simulate(&mut self, req: &SimRequest) -> Result<SimResult> {
        self.call("simulate", req)
    }

    pub fn preview(&mut self, req: &PreviewRequest) -> Result<PreviewResult> {
        self.call("preview", req)
    }

    pub fn end_preview(&mut self) -> Result<Status> {
        self.call("endPreview", &json!({}))
    }
}
