//! How the panel fits into the desktop session: start at login, and only one panel at a time.
//! No Qt in here; every path comes from the caller so tests can use temporary folders.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The desktop-entry id of the panel (also its window's app id).
pub const APP_ID: &str = "org.notify.Panel";

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

pub fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| home().join(".config"))
}

pub fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"))
}

pub fn autostart_file(config_home: &Path) -> PathBuf {
    config_home.join("autostart").join(format!("{APP_ID}.desktop"))
}

pub fn launcher_file(data_home: &Path) -> PathBuf {
    data_home.join("applications").join(format!("{APP_ID}.desktop"))
}

/// Quote a path for a desktop entry's Exec line.
fn exec_quote(p: &Path) -> String {
    let s = p.to_string_lossy();
    if s.chars().any(|c| c.is_whitespace() || "\"'\\$`<>~|&;*?#()".contains(c)) {
        // inside quotes the desktop-entry rules escape these four with a backslash, and a literal backslash is doubled again by the key's own escaping
        let mut o = String::from("\"");
        for c in s.chars() {
            if "\"$`\\".contains(c) {
                o.push('\\');
                if c == '\\' {
                    o.push('\\');
                    o.push('\\');
                }
            }
            o.push(c);
        }
        o.push('"');
        o
    } else {
        s.into_owned()
    }
}

/// The text of a desktop entry that starts the panel. `background` starts it hidden in the system tray.
pub fn desktop_entry(exe: &Path, background: bool) -> String {
    let flag = if background { " --background" } else { "" };
    format!(
        "[Desktop Entry]\nType=Application\nName=noti-fi\nGenericName=Notification settings\nComment=noti-fi (notification fidelity): control your notifications\n\
         Exec={}{flag}\nIcon=preferences-desktop-notification\nTerminal=false\nCategories=Settings;DesktopSettings;Qt;KDE;\nStartupWMClass={APP_ID}\n{}",
        exec_quote(exe),
        if background { "X-GNOME-Autostart-enabled=true\nX-KDE-autostart-phase=2\n" } else { "" }
    )
}

fn write(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("desktop.tmp");
    fs::write(&tmp, text)?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o644))?;
    fs::rename(&tmp, path)
}

pub fn autostart_enabled(config_home: &Path) -> bool {
    fs::read_to_string(autostart_file(config_home)).is_ok_and(|t| t.contains("Exec=") && !t.lines().any(|l| l.trim() == "Hidden=true"))
}

/// Start the panel (hidden, in the tray) when you log in, or stop doing so. Also keeps a launcher entry so the panel can be found in the app menu.
pub fn set_autostart(config_home: &Path, data_home: &Path, exe: &Path, on: bool) -> std::io::Result<()> {
    if on {
        write(&launcher_file(data_home), &desktop_entry(exe, false))?;
        write(&autostart_file(config_home), &desktop_entry(exe, true))
    } else {
        match fs::remove_file(autostart_file(config_home)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

// ---------------------------------------------------------------- one panel at a time

/// Where a running panel listens for "show yourself" from a second launch. `None` when there is no per-user runtime folder (a shared `/tmp`
/// is not safe for this), in which case the panel simply does not check for another copy.
pub fn instance_socket() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("NOTIFY_PANEL_SOCKET").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(p));
    }
    std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()).map(|d| PathBuf::from(d).join("notify-panel.sock"))
}

#[derive(Debug)]
pub enum Claim {
    /// Nobody else is running: this process is the panel and should listen on the socket.
    First(UnixListener),
    /// Another panel answered and was asked to show its window.
    Other,
}

/// Become the one panel, or ask the running one to come forward.
pub fn claim_instance(sock: &Path) -> std::io::Result<Claim> {
    if let Ok(mut s) = UnixStream::connect(sock) {
        let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
        if s.write_all(b"show\n").is_ok() {
            return Ok(Claim::Other);
        }
    }
    // nothing answered: a stale socket file may be left behind by a crash
    let _ = fs::remove_file(sock);
    if let Some(d) = sock.parent() {
        let _ = fs::create_dir_all(d);
    }
    let l = UnixListener::bind(sock)?;
    let _ = fs::set_permissions(sock, fs::Permissions::from_mode(0o600));
    Ok(Claim::First(l))
}

/// Call `on_show` each time another launch asks this panel to come forward. Runs in its own thread until the process ends.
pub fn serve(listener: UnixListener, on_show: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let _ = conn.set_read_timeout(Some(Duration::from_secs(2)));
            let mut line = String::new();
            if BufReader::new(conn.take(64)).read_line(&mut line).is_ok() && line.trim() == "show" {
                on_show();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

    fn tmp(label: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("notify-shell-{label}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn autostart_is_written_and_removed() {
        let (c, d) = (tmp("c"), tmp("d"));
        let exe = Path::new("/opt/my apps/notify-panel");
        assert!(!autostart_enabled(&c));
        set_autostart(&c, &d, exe, true).unwrap();
        assert!(autostart_enabled(&c));
        let t = fs::read_to_string(autostart_file(&c)).unwrap();
        assert!(t.contains("Exec=\"/opt/my apps/notify-panel\" --background"), "a path with a space is quoted and the panel starts hidden:\n{t}");
        assert!(t.contains("X-GNOME-Autostart-enabled=true"));
        let l = fs::read_to_string(launcher_file(&d)).unwrap();
        assert!(l.contains("Exec=\"/opt/my apps/notify-panel\"\n") && !l.contains("--background"), "the app-menu launcher opens the window:\n{l}");
        set_autostart(&c, &d, exe, true).unwrap();            // idempotent
        set_autostart(&c, &d, exe, false).unwrap();
        assert!(!autostart_enabled(&c) && !autostart_file(&c).exists());
        assert!(launcher_file(&d).exists(), "the launcher stays");
        set_autostart(&c, &d, exe, false).unwrap();           // turning off twice is fine
    }

    #[test]
    fn a_plain_path_is_not_quoted() {
        assert!(desktop_entry(Path::new("/home/me/.local/bin/notify-panel"), true).contains("Exec=/home/me/.local/bin/notify-panel --background\n"));
    }

    #[test]
    fn a_second_launch_asks_the_first_to_show_itself() {
        let sock = tmp("sock").join("p.sock");
        let Claim::First(l) = claim_instance(&sock).unwrap() else { panic!("the first launch must win") };
        let shown = Arc::new(AtomicUsize::new(0));
        let s2 = shown.clone();
        serve(l, move || { s2.fetch_add(1, Ordering::SeqCst); });
        assert!(matches!(claim_instance(&sock).unwrap(), Claim::Other));
        assert!(matches!(claim_instance(&sock).unwrap(), Claim::Other));
        for _ in 0..50 {
            if shown.load(Ordering::SeqCst) == 2 { break; }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(shown.load(Ordering::SeqCst), 2, "each later launch asks once");
    }

    #[test]
    fn a_stale_socket_from_a_crash_is_replaced() {
        let sock = tmp("stale").join("p.sock");
        drop(UnixListener::bind(&sock).unwrap());             // the file stays, nobody listens
        assert!(sock.exists());
        assert!(matches!(claim_instance(&sock).unwrap(), Claim::First(_)), "a leftover socket must not block the panel from starting");
    }
}
