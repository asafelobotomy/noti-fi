//! `notify settings` and `notify set`: the few settings a desktop's settings page changes without the panel.
//!
//! `set` edits rules.json the way the panel does (the same draft and the same atomic, conflict-checked save from
//! `notify-panel-core`), after the running daemon has checked the result with `validate`. So a bad value is refused by the
//! daemon's own rules, and nothing is written when it refuses, when the current file already has an error, or when the file
//! changed on disk in the meantime.

use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use clap::{CommandFactory, error::ErrorKind};
use notify_panel_core::draft::{Draft, SaveOutcome, rules_path, save};
use notify_protocol::Config;
use serde_json::json;

use crate::cli::{Cli, SettingKey};
use crate::ctx::Ctx;
use crate::daemon;

const LIMITS: [&str; 5] = ["50", "100", "200", "500", "1000"];

impl SettingKey {
    /// The key as the user types it, and as it sits in rules.json.
    fn name(self) -> &'static str {
        match self {
            SettingKey::HistoryLimit => "history.limit",
            SettingKey::HistoryPersist => "history.persist",
            SettingKey::DndAllowCritical => "dnd.allowCritical",
            SettingKey::AutoDndFullscreen => "autoDnd.fullscreen",
            SettingKey::AutoDndScreenShare => "autoDnd.screenShare",
        }
    }

    /// What the draft's `set_setting` takes for this value ("true"/"false", or the number), or why the value is not allowed.
    fn draft_text(self, value: &str) -> Result<String, String> {
        match self {
            SettingKey::HistoryLimit if LIMITS.contains(&value) => Ok(value.to_string()),
            SettingKey::HistoryLimit => Err(format!("use one of {}", LIMITS.join(", "))),
            _ => match value {
                "on" => Ok("true".into()),
                "off" => Ok("false".into()),
                _ => Err("use on or off".into()),
            },
        }
    }

    /// The value the daemon is running, in the words `set` takes.
    fn running(self, c: &Config) -> String {
        let onoff = |b: bool| if b { "on" } else { "off" }.to_string();
        match self {
            SettingKey::HistoryLimit => c.history.limit.to_string(),
            SettingKey::HistoryPersist => onoff(c.history.persist),
            SettingKey::DndAllowCritical => onoff(c.dnd.allow_critical),
            SettingKey::AutoDndFullscreen => onoff(c.auto_dnd.fullscreen),
            SettingKey::AutoDndScreenShare => onoff(c.auto_dnd.screen_share),
        }
    }
}

const KEYS: [SettingKey; 5] =
    [SettingKey::DndAllowCritical, SettingKey::HistoryLimit, SettingKey::HistoryPersist, SettingKey::AutoDndFullscreen, SettingKey::AutoDndScreenShare];

/// A value `set` does not accept is a usage error, like any other bad argument (exit 2).
fn usage_error(message: &str) -> ! {
    Cli::command().error(ErrorKind::InvalidValue, message).exit()
}

/// `notify settings`: what the daemon runs now.
pub fn settings(ctx: &Ctx) -> Result<()> {
    let mut c = daemon::connect(ctx.dev)?;
    let cfg = c.config_get()?.effective;
    let zones: Vec<_> = cfg
        .zones
        .iter()
        .map(|(id, z)| json!({ "id": id, "label": z.label.as_deref().filter(|l| !l.is_empty()).unwrap_or(id) }))
        .collect();
    let rules: Vec<String> = cfg
        .rules
        .iter()
        .enumerate()
        .map(|(i, r)| r.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Rule {}", i + 1)))
        .collect();
    if ctx.out.json {
        ctx.out.document(&json!({
            "dnd": { "allowCritical": cfg.dnd.allow_critical },
            "history": { "limit": cfg.history.limit, "persist": cfg.history.persist },
            "autoDnd": { "fullscreen": cfg.auto_dnd.fullscreen, "screenShare": cfg.auto_dnd.screen_share },
            "zones": zones,
            "rules": rules.iter().map(|n| json!({ "name": n })).collect::<Vec<_>>(),
        }));
        return Ok(());
    }
    println!("Settings (change one with: notify set <setting> <value>)");
    for k in KEYS {
        println!("  {}: {}", k.name(), k.running(&cfg));
    }
    println!("Rules (edit them in the panel: notify-panel)");
    if rules.is_empty() {
        println!("  none");
    }
    for (i, n) in rules.iter().enumerate() {
        println!("  {}. {n}", i + 1);
    }
    Ok(())
}

/// `notify set <key> <value>`.
pub fn set(ctx: &Ctx, key: SettingKey, value: &str) -> Result<()> {
    let text = key.draft_text(value).unwrap_or_else(|why| usage_error(&format!("'{value}' is not a value for {}: {why}", key.name())));
    let mut c = daemon::connect(ctx.dev)?;
    let got = c.config_get()?;
    if !got.error.is_empty() {
        bail!("notifd rejected the rules.json it has now ({}), so nothing was changed; fix that first (see: notify rules)", got.error);
    }
    let mut draft = if got.raw.trim().is_empty() {
        Draft::empty()
    } else {
        Draft::from_raw(&got.raw).map_err(|e| anyhow!("rules.json is not usable ({e}), so nothing was changed"))?
    };
    draft.set_setting(key.name(), &text).map_err(|e| anyhow!("{e}"))?;
    if !draft.is_dirty() {
        ctx.out.result(true, &format!("{} is already {value}", key.name()));
        return Ok(());
    }
    let new_text = draft.to_text();
    let verdict = c.validate_text(&new_text)?;
    if !verdict.ok {
        bail!("notifd refused this change ({}); nothing was changed", verdict.error.unwrap_or_else(|| "no reason given".into()));
    }
    match save(&rules_path(), &new_text, &got.raw, false) {
        SaveOutcome::Saved => {}
        SaveOutcome::Conflict { .. } => bail!("rules.json changed on disk while this ran, so nothing was changed; try again"),
        SaveOutcome::Failed(why) => bail!("{why}"),
    }
    // notifd reloads the file by itself; wait a moment so the next read shows the new value
    let end = Instant::now() + Duration::from_secs(3);
    let applied = loop {
        if c.config_get().is_ok_and(|g| key.running(&g.effective) == value) {
            break true;
        }
        if Instant::now() >= end {
            break false;
        }
        crate::sys::sleep_ms(100);
    };
    let message = if applied { format!("{} set to {value}", key.name()) } else { format!("{} set to {value} (notifd is still loading it)", key.name()) };
    ctx.out.result(true, &message);
    Ok(())
}
