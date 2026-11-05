//! Output: human text by default, JSON with --json.
//!
//! - Read commands print one JSON document.
//! - Commands that change things stream one JSON object per line ("progress events"): a `plan`, then a
//!   `step` event as each step starts and finishes, then a final `result`.

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub struct Out {
    pub json: bool,
}

pub const OK: &str = "ok";
pub const WARN: &str = "warn";
pub const BAD: &str = "FAIL";
pub const INFO: &str = "info";

impl Out {
    /// `  [ ok] message` plus an optional `-> hint` line.
    pub fn line(&self, level: &str, msg: &str, hint: Option<&str>) {
        if self.json {
            return;
        }
        println!("  [{level:>4}] {msg}");
        if let Some(h) = hint {
            println!("         -> {h}");
        }
    }

    pub fn heading(&self, text: &str) {
        if !self.json {
            println!("{text}");
        }
    }

    pub fn text(&self, msg: &str) {
        if !self.json {
            println!("{msg}");
        }
    }

    pub fn blank(&self) {
        if !self.json {
            println!();
        }
    }

    /// One JSON document (pretty).
    pub fn document<T: Serialize>(&self, v: &T) {
        println!("{}", serde_json::to_string_pretty(v).expect("serializable"));
    }

    /// One event line (compact), only in JSON mode.
    pub fn event(&self, v: Value) {
        if self.json {
            println!("{v}");
        }
    }

    /// The closing event of a changing command.
    pub fn result(&self, ok: bool, message: &str) {
        if self.json {
            println!("{}", json!({"type": "result", "ok": ok, "message": message}));
        } else if !message.is_empty() {
            println!("{message}");
        }
    }
}

type Action = Box<dyn FnOnce() -> Result<()>>;

pub struct Step {
    pub desc: String,
    pub action: Option<Action>,
}

impl Step {
    pub fn new(desc: impl Into<String>, action: impl FnOnce() -> Result<()> + 'static) -> Step {
        Step { desc: desc.into(), action: Some(Box::new(action)) }
    }
    /// A step that only describes something (nothing to execute).
    pub fn note(desc: impl Into<String>) -> Step {
        Step { desc: desc.into(), action: None }
    }
}

/// Print the plan; unless `dry`, run each step in order, reporting progress. Stops at the first failure.
pub fn run_plan(out: &Out, name: &str, steps: Vec<Step>, dry: bool) -> Result<()> {
    let total = steps.len();
    if out.json {
        let list: Vec<Value> = steps.iter().enumerate().map(|(i, s)| json!({"n": i + 1, "desc": s.desc})).collect();
        out.event(json!({"type": "plan", "name": name, "dry_run": dry, "total": total, "steps": list}));
    }
    for (i, s) in steps.into_iter().enumerate() {
        let n = i + 1;
        if !out.json {
            println!("  {n}. {}", s.desc);
        }
        if dry {
            continue;
        }
        out.event(json!({"type": "step", "n": n, "total": total, "desc": s.desc, "status": "running"}));
        if let Some(a) = s.action {
            if let Err(e) = a() {
                out.event(json!({"type": "step", "n": n, "total": total, "desc": s.desc, "status": "failed", "error": format!("{e:#}")}));
                return Err(e);
            }
        }
        out.event(json!({"type": "step", "n": n, "total": total, "desc": s.desc, "status": "done"}));
    }
    Ok(())
}

/// Shared slot for passing a value between plan steps.
pub type Slot<T> = Rc<RefCell<T>>;
pub fn slot<T>(v: T) -> Slot<T> {
    Rc::new(RefCell::new(v))
}
