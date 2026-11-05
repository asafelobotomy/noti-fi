//! Checking for and installing a newer release from the General page. Nothing is contacted unless the user (or the opt-in daily check) asks.

use std::pin::Pin;

use cxx_qt::Threading;
use cxx_qt_lib::QString;
use notify_update::{CURRENT, Curl, Release, is_newer};

use super::qobject;

/// The `notify` command-line program that sits beside this one (or on the PATH).
pub(super) fn notify_binary() -> std::path::PathBuf {
    let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("notify")));
    beside.filter(|p| p.exists()).unwrap_or_else(|| "notify".into())
}

/// The running program's path, without the " (deleted)" Linux adds once an update has replaced the file.
fn own_path() -> std::path::PathBuf {
    let p = std::env::current_exe().unwrap_or_else(|_| "notify-panel".into());
    p.to_string_lossy().strip_suffix(" (deleted)").map(std::path::PathBuf::from).unwrap_or(p)
}

impl qobject::PanelModel {
    pub fn check_for_updates(mut self: Pin<&mut Self>) {
        if matches!(self.as_ref().update_state().to_string().as_str(), "checking" | "installing") {
            return;
        }
        self.as_mut().set_update_state(QString::from("checking"));
        self.as_mut().set_update_message(QString::from(""));
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result: Result<Release, String> = notify_update::latest(&Curl);
            let _ = qt.queue(move |m| m.on_update_checked(result));
        });
    }

    fn on_update_checked(mut self: Pin<&mut Self>, result: Result<Release, String>) {
        match result {
            Ok(r) if is_newer(CURRENT, &r.version) => {
                self.as_mut().set_update_latest(QString::from(&r.version));
                self.as_mut().set_update_page(QString::from(&r.page));
                self.as_mut().set_update_message(QString::from(&format!("noti-fi {} is available (you have {CURRENT}).", r.version)));
                self.as_mut().set_update_state(QString::from("available"));
            }
            Ok(r) => {
                self.as_mut().set_update_latest(QString::from(&r.version));
                self.as_mut().set_update_message(QString::from(&format!("noti-fi {CURRENT} is up to date.")));
                self.as_mut().set_update_state(QString::from("current"));
            }
            Err(e) => {
                self.as_mut().set_update_message(QString::from(&format!("Could not check for updates: {e}")));
                self.as_mut().set_update_state(QString::from("error"));
            }
        }
    }

    /// Runs `notify upgrade --apply`, which does the download, the checksum check, the replacement and the daemon refresh.
    pub fn install_update(mut self: Pin<&mut Self>) {
        if self.as_ref().update_state().to_string() != "available" {
            return;
        }
        self.as_mut().set_update_state(QString::from("installing"));
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let out = std::process::Command::new(notify_binary()).args(["upgrade", "--apply"]).output();
            let (ok, text) = match out {
                Ok(o) if o.status.success() => (true, String::from_utf8_lossy(&o.stdout).lines().last().unwrap_or("Updated.").trim().to_string()),
                Ok(o) => (false, String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("the update failed").trim().to_string()),
                Err(e) => (false, format!("cannot run notify: {e}")),
            };
            let _ = qt.queue(move |mut m| {
                m.as_mut().set_update_message(QString::from(&text));
                m.as_mut().set_update_state(QString::from(if ok { "installed" } else { "error" }));
            });
        });
    }

    /// Start a fresh copy of this program a moment from now and quit this one (the new copy waits for this one to let go of its socket).
    pub fn restart_app(mut self: Pin<&mut Self>) {
        let _ = std::process::Command::new("sh").args(["-c", "sleep 1; exec \"$0\"", &own_path().to_string_lossy()]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
        self.as_mut().quit_requested();
    }
}
