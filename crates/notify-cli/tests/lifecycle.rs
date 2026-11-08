//! takeover, update, portal and restore in a fake home, with `--no-systemctl` and no session bus, so nothing on this machine
//! is touched. Checks the files each step leaves behind, and the JSON progress events the panel reads.

use std::path::{Path, PathBuf};
use std::process::Command;

use notify_testkit::Checks;
use serde_json::Value;

struct Home {
    root: PathBuf,
    desktop: String,
}

impl Home {
    fn new(label: &str) -> Home {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("notify-lifecycle-{label}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(root.join("run")).unwrap();
        Home { root, desktop: "KDE".into() }
    }
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let o = Command::new(env!("CARGO_BIN_EXE_notify"))
            .arg("--no-systemctl")
            .args(args)
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("XDG_DATA_HOME", self.root.join("data"))
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_RUNTIME_DIR", self.root.join("run")) // nothing listens here: no daemon
            .env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}", self.root.join("run/no-bus").display()))
            .env_remove("NOTIFD_SOCKET")
            .env("XDG_CURRENT_DESKTOP", &self.desktop)
            .output()
            .expect("run notify");
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
    }
    fn json_lines(&self, args: &[&str]) -> (i32, Vec<Value>) {
        let mut a = vec!["--json"];
        a.extend_from_slice(args);
        let (c, out, err) = self.run(&a);
        (c, out.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("not JSON ({e}): {l}\n{err}"))).collect())
    }
    fn p(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
    fn files(&self) -> Vec<String> {
        fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, root, out);
                } else {
                    out.push(p.strip_prefix(root).unwrap().display().to_string());
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &self.root, &mut out);
        out.retain(|f| !f.starts_with("run/"));
        out.sort();
        out
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn rules(h: &Home) -> Value {
    serde_json::from_str(&std::fs::read_to_string(h.p("config/notifd/rules.json")).unwrap()).unwrap()
}

#[test]
fn takeover_update_and_restore_in_a_fake_home() {
    let h = Home::new("main");
    let mut k = Checks::default();

    let (c, out, err) = h.run(&["takeover", "--dry-run", "--now"]);
    k.check("takeover --dry-run prints a plan", c == 0 && out.contains("DRY RUN"), || format!("{out}{err}"));
    k.check("...and creates no files", h.files().is_empty(), || format!("{:?}", h.files()));

    let (c, out, err) = h.run(&["takeover"]);
    k.check("takeover succeeds", c == 0 && out.contains("Undo any time with: notify restore"), || format!("{out}{err}"));
    for f in ["config/systemd/user/notifd.service", "config/systemd/user/notifd-fallback.service", "config/systemd/user/notifd-jobs.service",
              "data/dbus-1/services/org.freedesktop.Notifications.service", "config/notifd/rules.json", "state/notifd/takeover.json",
              "data/notifd/shell.qml", "data/notifd/Center.qml"] {
        k.check(&format!("takeover writes {f}"), h.p(f).exists(), || format!("{:?}", h.files()));
    }
    let unit = std::fs::read_to_string(h.p("config/systemd/user/notifd-jobs.service")).unwrap_or_default();
    k.check("the job bridge unit starts before plasmashell and runs `notify jobs`", unit.contains("Before=plasma-plasmashell.service") && unit.contains(" jobs"), || unit.clone());
    let unit = std::fs::read_to_string(h.p("config/systemd/user/notifd.service")).unwrap_or_default();
    k.check("notifd.service runs `notify serve` on the installed daemon files", unit.contains(" serve --payload ") && unit.contains("BusName=org.freedesktop.Notifications"), || unit.clone());
    let (c, _, _) = h.run(&["takeover"]);
    k.check("a second takeover is refused", c != 0, String::new);

    // update on drifted state: an old config missing newer settings, plus one user edit
    let mut d = rules(&h);
    let obj = d.as_object_mut().unwrap();
    obj.remove("sound");
    obj.remove("center");
    obj.insert("defaultTimeout".into(), 9999.into());
    std::fs::write(h.p("config/notifd/rules.json"), d.to_string()).unwrap();
    std::fs::remove_file(h.p("data/notifd/Center.qml")).unwrap();
    let (c, out, err) = h.run(&["update"]);
    k.check("update succeeds", c == 0, || format!("{out}{err}"));
    let d = rules(&h);
    k.check("update merges new defaults and keeps user values", d.get("sound").is_some() && d["center"]["anchor"] == "bottom-right" && d["defaultTimeout"] == 9999, || d.to_string());
    k.check("update restores the daemon files and keeps the previous version", h.p("data/notifd/Center.qml").exists() && h.p("data/notifd.prev/shell.qml").exists(), || format!("{:?}", h.files()));

    // portal: refuses to overwrite a file notify did not create
    let (c, out, err) = h.run(&["portal", "--remove"]);
    k.check("portal --remove removes what notify made", c == 0, || format!("{out}{err}"));
    std::fs::create_dir_all(h.p("config/xdg-desktop-portal")).unwrap();
    let portal = std::fs::read_dir(h.p("config/xdg-desktop-portal")).unwrap().count();
    std::fs::write(h.p("config/xdg-desktop-portal/kde-portals.conf"), "[preferred]\n").unwrap();
    let (c, _, _) = h.run(&["portal"]);
    k.check("portal refuses to overwrite a foreign file", c != 0 && std::fs::read_to_string(h.p("config/xdg-desktop-portal/kde-portals.conf")).unwrap() == "[preferred]\n", || portal.to_string());
    std::fs::remove_file(h.p("config/xdg-desktop-portal/kde-portals.conf")).unwrap();

    // restore: the shortcuts' launchers go too
    for f in ["notify-dnd-toggle.desktop", "notify-center-toggle.desktop"] {
        std::fs::create_dir_all(h.p("data/applications")).unwrap();
        std::fs::write(h.p(&format!("data/applications/{f}")), "[Desktop Entry]\n").unwrap();
    }
    let (c, out, err) = h.run(&["restore"]);
    k.check("restore succeeds", c == 0, || format!("{out}{err}"));
    let left = h.files();
    let units_gone = !left.iter().any(|f| f.contains("systemd/user/notifd") || f.contains("dbus-1/services") || f.contains("takeover.json"));
    k.check("restore removes the units, the D-Bus override and the state record", units_gone, || format!("{left:?}"));
    k.check("...and the shortcuts' launchers", !left.iter().any(|f| f.starts_with("data/applications/")), || format!("{left:?}"));
    k.check("...and keeps the config and the installed daemon", h.p("config/notifd/rules.json").exists() && h.p("data/notifd/shell.qml").exists(), || format!("{left:?}"));
    let (c, _, _) = h.run(&["restore", "--purge"]);
    k.check("restore --purge removes the daemon files but keeps the config", c == 0 && !h.p("data/notifd").exists() && !h.p("data/notifd.prev").exists() && h.p("config/notifd/rules.json").exists(), || format!("{:?}", h.files()));
    k.finish();
}

#[test]
fn bad_configs_are_reported_line_by_line() {
    let h = Home::new("lint");
    let mut k = Checks::default();
    let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../notifd/rules.json");
    let (c, out, err) = h.run(&["rules", "--file", shipped.to_str().unwrap()]);
    k.check("the shipped config passes", c == 0, || format!("{out}{err}"));
    let mut bad: Value = serde_json::from_str(&std::fs::read_to_string(&shipped).unwrap()).unwrap();
    bad["zones"]["system"]["anchor"] = "middle".into();
    bad["defualtZone"] = "x".into();
    for r in [r#"{"name": "named group", "match": {"appName": "(?P<x>steam)"}}"#, r#"{"name": "inline flag", "match": {"appName": "(?i)steam"}}"#,
              r#"{"name": "ghost zone", "match": {}, "zone": "ghost"}"#, r#"{"name": "bad field", "match": {"nope": "a"}}"#] {
        bad["rules"].as_array_mut().unwrap().push(serde_json::from_str(r).unwrap());
    }
    let f = h.p("bad.json");
    std::fs::write(&f, bad.to_string()).unwrap();
    let (c, out, _) = h.run(&["rules", "--file", f.to_str().unwrap()]);
    k.check("a bad config is rejected (exit 1)", c == 1, || out.clone());
    for needle in ["anchor must be", "unknown zone 'ghost'", "unknown match field 'nope'", "unknown setting 'defualtZone'", "named groups", "inline flags"] {
        k.check(&format!("reports: {needle}"), out.contains(needle), || out.clone());
    }
    k.finish();
}

#[test]
fn json_progress_events_for_the_panel() {
    let h = Home::new("json");
    let mut k = Checks::default();
    let (_, lines) = h.json_lines(&["takeover", "--dry-run"]);
    let plan = lines.iter().find(|l| l["type"] == "plan").cloned().unwrap_or_default();
    k.check("takeover --json --dry-run emits a plan listing every step", plan["dry_run"] == true && plan["total"].as_u64() == plan["steps"].as_array().map(|s| s.len() as u64) && plan["total"].as_u64() >= Some(9), || plan.to_string());
    k.check("...and a closing result", lines.last().is_some_and(|l| l["type"] == "result" && l["ok"] == true), || format!("{:?}", lines.last()));

    let (_, lines) = h.json_lines(&["takeover"]);
    let total = lines.first().and_then(|l| l["total"].as_u64()).unwrap_or(0);
    let steps: Vec<(u64, String)> = lines.iter().filter(|l| l["type"] == "step").map(|l| (l["n"].as_u64().unwrap_or(0), l["status"].as_str().unwrap_or("").to_string())).collect();
    let want: Vec<(u64, String)> = (1..=total).flat_map(|n| [(n, "running".to_string()), (n, "done".to_string())]).collect();
    k.check("a real run reports every step as running then done, in order", total > 0 && steps == want, || format!("{steps:?}"));
    k.check("...ending with a successful result", lines.last().is_some_and(|l| l["ok"] == true && l["message"].as_str().unwrap_or("").contains("notify restore")), || format!("{:?}", lines.last()));

    let (c, lines) = h.json_lines(&["takeover"]);
    k.check("a refused command reports a failed result and exits 1", c == 1 && lines.last().is_some_and(|l| l["type"] == "result" && l["ok"] == false), || format!("{c} {:?}", lines.last()));
    let (_, lines) = h.json_lines(&["restore"]);
    let done = lines.iter().filter(|l| l["status"] == "done").count() as u64;
    k.check("restore --json streams a plan, steps and a result", lines.first().is_some_and(|l| l["type"] == "plan" && l["total"].as_u64() == Some(done)) && lines.last().is_some_and(|l| l["ok"] == true), || format!("{lines:?}"));
    k.finish();
}

#[test]
fn on_hyprland_takeover_installs_a_plain_user_service() {
    let mut h = Home::new("plain");
    h.desktop = "Hyprland".into();
    let mut k = Checks::default();
    let (c, out, err) = h.run(&["takeover"]);
    k.check("takeover on Hyprland says it installs a plain user service", c == 0 && out.contains("plain user service"), || format!("{out}{err}"));
    let unit = std::fs::read_to_string(h.p("config/systemd/user/notifd.service")).unwrap_or_default();
    k.check("notifd starts with the graphical session, with no Plasma ordering or fallback", unit.contains("WantedBy=graphical-session.target") && !unit.contains("plasma") && !unit.contains("OnFailure"), || unit.clone());
    let jobs = std::fs::read_to_string(h.p("config/systemd/user/notifd-jobs.service")).unwrap_or_default();
    k.check("the job bridge too (KDE apps on other desktops report their copies there)", jobs.contains("WantedBy=graphical-session.target") && !jobs.contains("plasma"), || jobs.clone());
    k.check("no fallback unit (there is no Plasma to hand back to)", !h.p("config/systemd/user/notifd-fallback.service").exists(), || format!("{:?}", h.files()));
    let state: Value = serde_json::from_str(&std::fs::read_to_string(h.p("state/notifd/takeover.json")).unwrap()).unwrap();
    k.check("the install is recorded as plain", state["plain"] == true, || state.to_string());
    let (c, out, err) = h.run(&["update"]);
    let unit = std::fs::read_to_string(h.p("config/systemd/user/notifd.service")).unwrap_or_default();
    k.check("update keeps it plain", c == 0 && !unit.contains("plasma") && !h.p("config/systemd/user/notifd-fallback.service").exists(), || format!("{out}{err}{unit}"));
    let (c, out, err) = h.run(&["restore"]);
    k.check("restore removes it", c == 0 && !h.p("config/systemd/user/notifd.service").exists() && !h.p("state/notifd/takeover.json").exists(), || format!("{out}{err}"));
    h.desktop = "KDE".into();
    let (c, out, _) = h.run(&["takeover", "--dry-run", "--plain"]);
    k.check("--plain forces it on Plasma too", c == 0 && out.contains("plain user service") && !out.contains("fallback"), || out.clone());
    k.finish();
}
