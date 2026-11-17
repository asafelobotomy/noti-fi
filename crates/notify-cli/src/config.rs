//! `notify config show|set|reset`: every scalar setting in rules.json (the ones the panel edits), and the plan documents
//! (`--dry-run --json`) that uQ's Settings page shows before a change.
//!
//! `show` reads the file itself, so it works with the daemon down and a page can watch the file. `set` and `reset` go through
//! the same daemon-checked, conflict-checked save as `notify set` (see settings.rs), which is the older, narrower spelling.
//!
//! A plan is one JSON document: `{summary, changes: [{group, label, detail, bytes?, reversible}], would_refuse, reason}`.

use anyhow::{Result, anyhow};
use clap::{CommandFactory, error::ErrorKind};
use notify_panel_core::draft::{Draft, rules_path, setting_specs};
use serde_json::{Value, json};

use crate::cli::{Cli, ConfigCmd};
use crate::ctx::Ctx;
use crate::settings::{self, Proposal};

pub fn run(ctx: &Ctx, action: Option<ConfigCmd>) -> Result<()> {
    match action.unwrap_or(ConfigCmd::Show) {
        ConfigCmd::Show => show(ctx),
        ConfigCmd::Set { key, value, dry_run } => set(ctx, &key, &value, dry_run),
        ConfigCmd::Reset { key, dry_run } => set(ctx, &key, "", dry_run),
    }
}

fn usage_error(message: &str) -> ! {
    Cli::command().error(ErrorKind::InvalidValue, message).exit()
}

fn word(v: &Value) -> String {
    match v {
        Value::Bool(b) => if *b { "on" } else { "off" }.into(),
        Value::String(s) if s.is_empty() => "(default)".into(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn show(ctx: &Ctx) -> Result<()> {
    let path = rules_path();
    let raw = std::fs::read_to_string(&path).unwrap_or_default();
    let draft = if raw.trim().is_empty() { Draft::empty() } else { Draft::from_raw(&raw).map_err(|e| anyhow!("{} is not usable: {e}", path.display()))? };
    let rows: Vec<Value> = setting_specs()
        .into_iter()
        .map(|s| {
            let value = draft.setting_value(s.key).unwrap_or(Value::Null);
            let mut row = json!({ "key": s.key, "type": s.kind, "default": s.default, "value": value, "set": draft.setting_is_set(s.key) });
            if !s.choices.is_empty() {
                row["choices"] = json!(s.choices);
            }
            row
        })
        .collect();
    if ctx.out.json {
        ctx.out.document(&json!({ "file": path, "settings": rows }));
        return Ok(());
    }
    println!("Settings in {} (change one with: notify config set <key> <value>; undo with: notify config reset <key>)", path.display());
    for r in &rows {
        let mark = if r["set"] == true { "" } else { "  (default)" };
        println!("  {}: {}{mark}", r["key"].as_str().unwrap_or(""), word(&r["value"]));
    }
    Ok(())
}

/// The words the draft takes for this value of a setting, or why the value can't be used (a usage error, like any bad argument).
fn draft_text(key: &str, value: &str) -> String {
    let Some(spec) = setting_specs().into_iter().find(|s| s.key == key) else { usage_error(&format!("'{key}' is not a setting; see: notify config show")) };
    let bad = |why: &str| -> ! { usage_error(&format!("'{value}' is not a value for {key}: {why}")) };
    if value.is_empty() {
        return String::new();
    }
    match spec.kind {
        "bool" => match value {
            "on" | "true" | "yes" => "true".into(),
            "off" | "false" | "no" => "false".into(),
            _ => bad("use on or off"),
        },
        "color" if !(value.starts_with('#') && (4..=9).contains(&value.len()) && value[1..].chars().all(|c| c.is_ascii_hexdigit())) => bad("use a colour like #1d1f27"),
        "number" if value.trim().parse::<f64>().is_err() => bad("use a number"),
        _ => {
            let parts = if key == "theme.timerStyle" { value.split(',').collect() } else { vec![value] };
            if !spec.choices.is_empty() && !parts.iter().all(|p| spec.choices.contains(p)) {
                bad(&format!("use one of {}", spec.choices.join(", ")));
            }
            value.to_string()
        }
    }
}

fn set(ctx: &Ctx, key: &str, value: &str, dry_run: bool) -> Result<()> {
    let text = draft_text(key, value);
    let shown = if value.is_empty() { "its default" } else { value };
    settings::change(ctx, key, &text, shown, dry_run)
}

fn plan(summary: &str, changes: Vec<Value>, refusal: &str) -> Value {
    json!({ "summary": summary, "changes": changes, "would_refuse": !refusal.is_empty(), "reason": refusal })
}

/// `--dry-run` of `config set`: what would change, and whether the daemon would take it.
pub fn print_plan(ctx: &Ctx, key: &str, shown: &str, p: &Proposal) {
    let before = Draft::from_raw(&p.raw).ok().and_then(|d| d.setting_value(key)).unwrap_or(Value::Null);
    let after = Draft::from_raw(&p.new_text).ok().and_then(|d| d.setting_value(key)).unwrap_or(Value::Null);
    let (summary, changes) = if p.unchanged {
        (format!("{key} is already {shown}"), vec![])
    } else {
        (format!("Set {key} to {}", word(&after)), vec![json!({ "group": "rules.json", "label": key, "detail": format!("{} to {}", word(&before), word(&after)), "reversible": true })])
    };
    let doc = plan(&summary, changes, &p.refusal);
    if ctx.out.json {
        ctx.out.document(&doc);
    } else if p.refusal.is_empty() {
        println!("{summary}");
    } else {
        println!("{summary}, but notifd would refuse: {}", p.refusal);
    }
}

/// `history --clear --dry-run`: how much would go, grouped by app. Nothing is changed.
pub fn print_history_plan(ctx: &Ctx, c: &mut notify_client::Client) -> Result<()> {
    let count = c.status()?.history;
    let rows = c.history(100_000)?;
    let mut apps: Vec<(String, usize)> = Vec::new();
    for r in &rows {
        match apps.iter_mut().find(|(a, _)| *a == r.app) {
            Some(e) => e.1 += 1,
            None => apps.push((r.app.clone(), 1)),
        }
    }
    apps.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let when = |t: i64| jiff::Timestamp::from_millisecond(t).ok().map(|d| d.to_zoned(jiff::tz::TimeZone::system()).strftime("%Y-%m-%d %H:%M").to_string()).unwrap_or_default();
    let (newest, oldest) = (rows.iter().map(|r| r.t).max(), rows.iter().map(|r| r.t).min());
    let noun = |n: usize| format!("{n} notification{}", if n == 1 { "" } else { "s" });
    let refusal = if count == 0 { "The history is already empty." } else { "" };
    let summary = if count == 0 { "Nothing to clear".to_string() } else { format!("Clear {}", noun(count as usize)) };
    let changes: Vec<Value> = apps.iter().map(|(a, n)| json!({ "group": "History", "label": if a.is_empty() { "(unnamed app)" } else { a }, "detail": noun(*n), "reversible": false })).collect();
    let mut doc = plan(&summary, changes, refusal);
    doc["would_remove"] = json!(count);
    doc["oldest"] = json!(oldest);
    doc["newest"] = json!(newest);
    if ctx.out.json {
        ctx.out.document(&doc);
    } else if count == 0 {
        println!("The history is already empty.");
    } else {
        println!("{summary}, from {} to {}", oldest.map(when).unwrap_or_default(), newest.map(when).unwrap_or_default());
        for (a, n) in &apps {
            println!("  {a}: {}", noun(*n));
        }
    }
    Ok(())
}
