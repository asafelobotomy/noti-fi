//! Daily controls: everything that talks to the running daemon.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{Local, TimeZone};
use notify_protocol::{SimResult, Status, Urgency};
use serde::Serialize;
use serde_json::{Value, json};

use crate::ctx::Ctx;
use crate::daemon;
use crate::lint;
use crate::out::{BAD, INFO, OK, WARN};
use crate::paths::paths;
use crate::payload;

fn state_line(s: &Status) -> String {
    let mut line = format!(
        "Do Not Disturb: {}   muted zones: {}   on screen: {}   history: {}",
        if s.dnd { "ON" } else { "off" },
        if s.muted.is_empty() { "none".to_string() } else { s.muted.join(", ") },
        s.active,
        s.history
    );
    if !s.focus.is_empty() {
        line.push_str(&format!("   focus: {}", s.focus));
    }
    line
}

/// Focus mode: no argument shows it, `list` shows the modes, `off` turns it off, any other word turns that mode on.
pub fn focus(ctx: &Ctx, mode: Option<&str>) -> Result<()> {
    let mut c = daemon::connect(ctx.dev)?;
    match mode {
        None => show_status(ctx, &c.status()?),
        Some("list") => {
            let g = c.config_get()?;
            let active = c.status()?.focus;
            if ctx.out.json {
                ctx.out.document(&json!({ "active": active, "modes": g.effective.focus.modes }));
            } else {
                for (name, m) in &g.effective.focus.modes {
                    let allow = if m.allow.is_empty() { String::new() } else { format!(", plus {} allowed", m.allow.len()) };
                    println!(
                        "{} {name}: {} (lets through {} and above{allow})",
                        if *name == active { "*" } else { " " },
                        m.label.as_deref().unwrap_or(name),
                        m.min_priority.as_deref().unwrap_or("urgent")
                    );
                }
            }
        }
        Some(m) => {
            let s = c.focus(m).map_err(|e| anyhow!("{e}"))?;
            show_status(ctx, &s);
        }
    }
    Ok(())
}

fn show_status(ctx: &Ctx, s: &Status) {
    if ctx.out.json {
        ctx.out.document(s);
    } else {
        println!("{}", state_line(s));
    }
}

pub fn dnd(ctx: &Ctx, mode: &str) -> Result<()> {
    let mut c = daemon::connect(ctx.dev)?;
    let s = if mode == "status" { c.status()? } else { c.dnd(mode)? };
    show_status(ctx, &s);
    Ok(())
}

pub fn mute(ctx: &Ctx, zone: &str, on: bool) -> Result<()> {
    let mut c = daemon::connect(ctx.dev)?;
    let s = c.mute(zone, on).map_err(|e| anyhow!("{e}"))?;
    show_status(ctx, &s);
    Ok(())
}

pub fn dismiss(ctx: &Ctx) -> Result<()> {
    let mut c = daemon::connect(ctx.dev)?;
    c.dismiss_all()?;
    ctx.out.result(true, "dismissed everything on screen");
    Ok(())
}

pub fn reply(ctx: &Ctx, id: i64, text: &str) -> Result<()> {
    let mut c = daemon::connect(ctx.dev)?;
    c.reply(id, text).map_err(|e| anyhow!("{e}"))?;
    ctx.out.result(true, "reply sent");
    Ok(())
}

pub struct HistoryArgs {
    pub n: usize,
    pub zone: Option<String>,
    pub app: Option<String>,
    pub missed: bool,
    pub full: bool,
    pub clear: bool,
}

pub fn history(ctx: &Ctx, a: HistoryArgs) -> Result<()> {
    let mut c = daemon::connect(ctx.dev)?;
    if a.clear {
        c.clear_history()?;
        ctx.out.result(true, "history cleared");
        return Ok(());
    }
    let mut items = c.history(1000)?;
    if let Some(z) = &a.zone {
        items.retain(|i| &i.zone == z);
    }
    if let Some(pat) = &a.app {
        let re = regress::Regex::with_flags(pat, "i").map_err(|e| anyhow!("bad --app pattern: {e}"))?;
        items.retain(|i| re.find(&i.app).is_some());
    }
    if a.missed {
        items.retain(|i| !i.shown);
    }
    items.truncate(a.n);
    if ctx.out.json {
        ctx.out.document(&items);
        return Ok(());
    }
    if items.is_empty() {
        println!("no notifications in history");
        return Ok(());
    }
    for i in &items {
        let t = Local.timestamp_millis_opt(i.t).single().map(|d| d.format("%H:%M:%S").to_string()).unwrap_or_else(|| "??:??:??".into());
        let flag = if i.shown { String::new() } else { format!("  [{}]", i.why) };
        let site = if i.origin.is_empty() { String::new() } else { format!(" ({})", i.origin) };
        let upd = match i.updates {
            Some(n) if n > 0 => format!(" (updated x{n})"),
            _ => String::new(),
        };
        println!("{t}  {:<9} {}{site}: {}{upd}{flag}", i.zone, i.app, i.summary);
        if a.full && !i.body.is_empty() {
            println!("          {}", i.body);
        }
    }
    Ok(())
}

pub struct SendArgs {
    pub summary: String,
    pub body: Option<String>,
    pub app: String,
    pub urgency: String,
    pub timeout: i64,
    pub origin: Option<String>,
    pub desktop_entry: Option<String>,
}

/// notify-send talks to the session bus; in dev mode that is the dev daemon's private bus.
fn notify_send(ctx: &Ctx, a: &SendArgs) -> Result<()> {
    let mut cmd = Command::new("notify-send");
    cmd.args(["-a", &a.app, "-u", &a.urgency, "-t", &a.timeout.to_string()]);
    if let Some(o) = &a.origin {
        cmd.args(["-h", &format!("string:x-kde-origin-name:{o}")]);
    }
    if let Some(e) = &a.desktop_entry {
        cmd.args(["-h", &format!("string:desktop-entry:{e}")]);
    }
    cmd.arg(&a.summary);
    if let Some(b) = &a.body {
        cmd.arg(b);
    }
    if ctx.dev {
        let bus = std::fs::read_to_string(paths().dev_bus).context("no dev daemon running; start one with: notify dev")?;
        cmd.env("DBUS_SESSION_BUS_ADDRESS", bus.trim());
    }
    let st = cmd.status().context("running notify-send (is libnotify installed?)")?;
    if !st.success() {
        bail!("notify-send failed");
    }
    Ok(())
}

pub fn send(ctx: &Ctx, a: SendArgs) -> Result<()> {
    notify_send(ctx, &a)
}

const SAMPLES: [(&str, &str, &str, &str, Option<&str>, Option<&str>); 6] = [
    ("Brave Origin", "normal", "New message", "A subscriber sent you a message", Some("onlyfans.com"), Some("brave-origin")),
    ("Brave Origin", "normal", "Review requested", "A pull request needs you", Some("github.com"), Some("brave-origin")),
    ("Steam", "normal", "Friend online", "someone started playing a game", None, None),
    ("Claude", "normal", "Task finished", "Your workspace review is ready", None, None),
    ("Software Update", "normal", "Updates available", "12 packages can be updated", None, None),
    ("kernel", "critical", "Disk almost full", "/home is at 97%", None, None),
];

pub fn test(ctx: &Ctx, timeout: i64) -> Result<()> {
    for (app, urgency, summary, body, origin, entry) in SAMPLES {
        notify_send(ctx, &SendArgs {
            summary: summary.into(),
            body: Some(body.into()),
            app: app.into(),
            urgency: urgency.into(),
            timeout,
            origin: origin.map(Into::into),
            desktop_entry: entry.map(Into::into),
        })?;
        crate::sys::sleep_ms(300);
    }
    ctx.out.result(true, &format!("sent {} sample notifications", SAMPLES.len()));
    Ok(())
}

// ---------------------------------------------------------------- simulate / preview / rules

fn read_draft(path: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("{} is not valid JSON", path.display()))
}

pub struct SimArgs {
    pub summary: String,
    pub app: String,
    pub entry: String,
    pub origin: String,
    pub body: Option<String>,
    pub urgency: Urgency,
    pub timeout: i64,
    pub config: Option<PathBuf>,
    pub ignore_state: bool,
}

pub fn simulate(ctx: &Ctx, a: SimArgs) -> Result<()> {
    let mut req = json!({
        "notification": {
            "appName": a.app, "desktopEntry": a.entry, "origin": a.origin, "summary": a.summary,
            "body": a.body.unwrap_or_default(), "urgency": a.urgency, "expireTimeout": a.timeout
        }
    });
    if a.ignore_state {
        req["ignoreState"] = json!(true);
    }
    if let Some(p) = &a.config {
        req["config"] = read_draft(p)?;
    }
    let mut c = daemon::connect(ctx.dev)?;
    let v = c.call_raw("simulate", req).map_err(|e| anyhow!("{e}"))?;
    let r: SimResult = serde_json::from_value(v)?;
    if ctx.out.json {
        ctx.out.document(&r);
        return Ok(());
    }
    let why = |d: &Option<String>| d.as_ref().map(|n| format!(" (rule '{n}')")).unwrap_or_else(|| " (default)".into());
    println!("zone:     {}{}", r.zone, why(&r.decided_by.zone));
    println!("timeout:  {}{}", if r.timeout == 0 { "stays until dismissed".to_string() } else { format!("{} ms", r.timeout) }, why(&r.decided_by.timeout));
    println!("outcome:  {}", if r.would_show { "shown".to_string() } else { format!("NOT shown: {}", r.hold) });
    println!("matched:  {}", if r.matched.is_empty() { "no rules (defaults apply)".to_string() } else { r.matched.join(", ") });
    Ok(())
}

pub fn preview(ctx: &Ctx, config: Option<PathBuf>, seconds: u32, end: bool) -> Result<()> {
    let mut c = daemon::connect(ctx.dev)?;
    if end {
        c.end_preview().map_err(|e| anyhow!("{e}"))?;
        ctx.out.result(true, "preview ended");
        return Ok(());
    }
    let mut req = json!({ "seconds": seconds });
    if let Some(p) = &config {
        req["config"] = read_draft(p)?;
    }
    let v = c.call_raw("preview", req).map_err(|e| anyhow!("{e}"))?;
    let shown = v.get("shown").and_then(Value::as_u64).unwrap_or(0);
    let secs = v.get("seconds").and_then(Value::as_u64).unwrap_or(0);
    if ctx.out.json {
        ctx.out.document(&v);
    } else {
        println!("previewing {shown} sample popups for {secs}s{}", if config.is_some() { " using the draft config" } else { "" });
    }
    Ok(())
}

#[derive(Serialize)]
pub struct RulesReport {
    pub file: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub daemon: Option<Value>,
}

/// Build the rules report (and keep the file text for the summary line).
pub fn rules_report(ctx: &Ctx, file: Option<PathBuf>) -> (RulesReport, Option<String>, bool) {
    let is_file = file.is_some();
    let path = file.unwrap_or_else(|| paths().rules);
    let mut rep = RulesReport { file: path.display().to_string(), errors: vec![], warnings: vec![], daemon: None };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => Some(t),
        Err(_) if !is_file => Some(payload::DEFAULT_RULES.to_string()),
        Err(e) => {
            rep.errors.push(format!("cannot load: {e}"));
            None
        }
    };
    if let Some(t) = &text {
        match serde_json::from_str::<Value>(t) {
            Err(e) => rep.errors.push(format!("not valid JSON: {e}")),
            Ok(v) => {
                rep.errors.extend(lint::lint_config(&v));
                rep.warnings = lint::lint_warnings(&v);
                if let Some(mut c) = daemon::try_connect(ctx.dev) {
                    rep.daemon = if is_file {
                        c.validate_text(t).ok().and_then(|r| serde_json::to_value(r).ok())
                    } else {
                        c.config_status().ok().map(|s| json!({"ok": s.ok, "error": s.error, "rules": s.rules}))
                    };
                }
            }
        }
    }
    let daemon_bad = rep.daemon.as_ref().map(|d| d["ok"] == json!(false)).unwrap_or(false);
    let good = rep.errors.is_empty() && !daemon_bad;
    (rep, text, good)
}

pub fn print_rules(ctx: &Ctx, rep: &RulesReport, text: Option<&str>, is_file: bool) {
    println!("Rules: {}", rep.file);
    for e in &rep.errors {
        ctx.out.line(BAD, e, None);
    }
    for w in &rep.warnings {
        ctx.out.line(WARN, w, None);
    }
    if rep.errors.is_empty() {
        if let Some(Ok(v)) = text.map(serde_json::from_str::<Value>) {
            let z = v.get("zones").and_then(Value::as_object).map(|m| m.len()).unwrap_or(0);
            let r = v.get("rules").and_then(Value::as_array).map(|m| m.len()).unwrap_or(0);
            ctx.out.line(OK, &format!("{z} zones, {r} rules: valid"), None);
        }
    }
    match &rep.daemon {
        None => ctx.out.line(INFO, "daemon not running; only the static lint above applies", None),
        Some(d) if d["ok"] == json!(true) => ctx.out.line(OK, &format!("the running daemon accepts this config{}", if is_file { " (checked without applying it)" } else { "" }), None),
        Some(d) => ctx.out.line(
            BAD,
            &format!("the running daemon {} {}", if is_file { "REJECTS this config:" } else { "REJECTED the config and is using the previous one:" }, d["error"].as_str().unwrap_or("")),
            None,
        ),
    }
}

/// Returns true when everything is fine.
pub fn rules(ctx: &Ctx, file: Option<PathBuf>) -> Result<bool> {
    let is_file = file.is_some();
    let (rep, text, good) = rules_report(ctx, file);
    if ctx.out.json {
        ctx.out.document(&rep);
    } else {
        print_rules(ctx, &rep, text.as_deref(), is_file);
    }
    Ok(good)
}
