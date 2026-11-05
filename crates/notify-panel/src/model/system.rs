//! The System page: the health checks (`notify doctor`) and the install actions (take over, restore, update the daemon, make Brave deliver).
//! Each action first shows what it would do (the command's own preview), and only runs for real when the page asks for `apply`.
//! The work is done by the `notify` program beside this one, so the panel and the command line can never disagree.

use std::pin::Pin;

use cxx_qt::Threading;
use cxx_qt_lib::QString;
use serde_json::{Value, json};

use super::qobject;
use super::update::notify_binary;

/// The command line for an action. `None` for an unknown action, or for a preview the command does not have.
fn args_for(action: &str, apply: bool) -> Option<Vec<&'static str>> {
    Some(match (action, apply) {
        ("takeover", false) => vec!["takeover", "--dry-run"],
        ("takeover", true) => vec!["takeover", "--now"],
        ("restore", false) => vec!["restore", "--dry-run"],
        ("restore", true) => vec!["restore"],
        ("adapt", false) => vec!["adapt"],
        ("adapt", true) => vec!["adapt", "--apply"],
        ("update", true) => vec!["update"],
        _ => return None,
    })
}

fn run(args: &[&str]) -> (bool, String) {
    match std::process::Command::new(notify_binary()).args(args).output() {
        Ok(o) => {
            let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            (o.status.success(), text.trim().to_string())
        }
        Err(e) => (false, format!("cannot run notify: {e}")),
    }
}

impl qobject::PanelModel {
    fn system_state(mut self: Pin<&mut Self>, edit: impl FnOnce(&mut Value)) {
        let mut v: Value = serde_json::from_str(&self.as_ref().system_json().to_string()).unwrap_or_else(|_| json!({}));
        edit(&mut v);
        self.as_mut().set_system_json(QString::from(&v.to_string()));
    }

    /// Run the health checks in the background.
    pub fn system_doctor(mut self: Pin<&mut Self>) {
        self.as_mut().system_state(|v| v["busy"] = json!("doctor"));
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let (_, text) = run(&["--json", "doctor"]);
            let parsed: Option<Value> = serde_json::from_str(&text).ok();
            let _ = qt.queue(move |m| {
                m.system_state(|v| {
                    v["busy"] = json!("");
                    match parsed {
                        Some(d) => { v["checks"] = d["checks"].clone(); v["summary"] = d["summary"].clone(); v["error"] = json!(""); }
                        None => v["error"] = json!(format!("The health check could not run: {text}")),
                    }
                });
            });
        });
    }

    /// Preview (`apply` false) or perform an action. The result text lands in the page's output box; after a real run the checks are redone.
    pub fn system_run(mut self: Pin<&mut Self>, action: &QString, apply: bool) {
        let action = action.to_string();
        if self.as_ref().system_json().to_string().contains("\"busy\":\"") && !self.as_ref().system_json().to_string().contains("\"busy\":\"\"") {
            return; // one thing at a time
        }
        let Some(args) = args_for(&action, apply) else {
            self.system_state(|v| v["output"] = json!("This action has no preview; it only runs for real."));
            return;
        };
        let name = action.clone();
        self.as_mut().system_state(|v| { v["busy"] = json!(name); v["output"] = json!(""); });
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let (ok, text) = run(&args);
            let _ = qt.queue(move |mut m| {
                m.as_mut().system_state(|v| { v["busy"] = json!(""); v["ok"] = json!(ok); v["output"] = json!(text); v["applied"] = json!(apply); v["action"] = json!(action); });
                if apply { m.system_doctor(); }
            });
        });
    }
}
