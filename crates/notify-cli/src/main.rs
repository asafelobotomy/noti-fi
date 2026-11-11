//! notify: install, run and control the notifd notification daemon.

mod apps;
mod cli;
mod controls;
mod ctx;
mod daemon;
mod dev;
mod hotkey;
mod install;
mod integrations;
mod lint;
mod out;
mod paths;
mod payload;
mod settings;
mod status;
mod sys;
mod units;
mod upgrade;

use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use notify_protocol::Urgency;

use crate::cli::*;
use crate::ctx::Ctx;
use crate::out::Out;

fn run(cli: Cli) -> Result<bool> {
    let ctx = Ctx { out: Out { json: cli.json }, dev: cli.dev, no_systemctl: cli.no_systemctl, payload_dir: cli.payload };
    match cli.cmd {
        Cmd::Status => status::status(&ctx)?,
        Cmd::Doctor => return status::doctor(&ctx),
        Cmd::Apps => apps::run(&ctx)?,
        Cmd::Rules { file } => return controls::rules(&ctx, file),
        Cmd::Simulate { summary, app, entry, origin, body, urgency, timeout, config, ignore_state } => controls::simulate(
            &ctx,
            controls::SimArgs {
                summary: summary.unwrap_or_default(),
                app,
                entry,
                origin,
                body,
                urgency: match urgency {
                    Urg::Low => Urgency::Low,
                    Urg::Normal => Urgency::Normal,
                    Urg::Critical => Urgency::Critical,
                },
                timeout,
                config,
                ignore_state,
            },
        )?,
        Cmd::Preview { config, seconds, end } => controls::preview(&ctx, config, seconds, end)?,
        Cmd::Takeover { dry_run, now, no_portal, no_autodnd, plain } => install::takeover(&ctx, now, dry_run, no_portal, no_autodnd, plain)?,
        Cmd::Update => install::update(&ctx)?,
        Cmd::Upgrade { apply } => upgrade::upgrade(&ctx, apply)?,
        Cmd::Restore { dry_run, purge, auto } => install::restore(&ctx, dry_run, purge, auto)?,
        Cmd::Portal { remove, dry_run } => install::portal(&ctx, remove, dry_run)?,
        Cmd::Adapt { apply, background, autostart } => integrations::adapt(&ctx, integrations::AdaptArgs { apply, background, autostart })?,
        Cmd::Hotkey { keys, center, remove, dry_run } => {
            hotkey::hotkey(&ctx, if center { hotkey::Target::Center } else { hotkey::Target::Dnd }, keys.as_deref(), remove, dry_run)?
        }
        Cmd::Autodnd { action } => integrations::autodnd(&ctx, match action { Toggle::Status => "status", Toggle::Install => "install", Toggle::Remove => "remove" })?,
        Cmd::Tray { action } => integrations::tray(&ctx, match action { TrayAction::Status => "status", TrayAction::Hide => "hide", TrayAction::Show => "show" })?,
        Cmd::Settings => settings::settings(&ctx)?,
        Cmd::Set { key, value } => settings::set(&ctx, key, &value)?,
        Cmd::Focus { mode } => controls::focus(&ctx, mode.as_deref())?,
        Cmd::Dnd { mode } => controls::dnd(&ctx, match mode { Mode::On => "on", Mode::Off => "off", Mode::Toggle => "toggle", Mode::Status => "status" })?,
        Cmd::Mute { zone } => controls::mute(&ctx, &zone, true)?,
        Cmd::Unmute { zone } => controls::mute(&ctx, &zone, false)?,
        Cmd::History { n, zone, app, missed, full, clear } => controls::history(&ctx, controls::HistoryArgs { n, zone, app, missed, full, clear })?,
        Cmd::Center { action } => controls::center(&ctx, match action { CenterAction::Toggle => "toggle", CenterAction::Open => "open", CenterAction::Close => "close" })?,
        Cmd::Dismiss => controls::dismiss(&ctx)?,
        Cmd::Reply { id, text } => controls::reply(&ctx, id, &text)?,
        Cmd::Send { summary, body, app, urgency, timeout, origin, desktop_entry } => {
            controls::send(&ctx, controls::SendArgs { summary, body, app, urgency: urgency.s().into(), timeout, origin, desktop_entry })?
        }
        Cmd::Test { timeout } => controls::test(&ctx, timeout)?,
        Cmd::Dev => dev::dev(&ctx)?,
        Cmd::Jobs => notify_jobs::run()?,
        Cmd::Selftest => dev::selftest()?,
        Cmd::Serve => daemon::serve(&ctx)?,
    }
    Ok(true)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    match run(cli) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            if json {
                println!("{}", serde_json::json!({"type": "result", "ok": false, "message": format!("{e:#}")}));
            } else {
                eprintln!("error: {e:#}");
            }
            ExitCode::from(1)
        }
    }
}
