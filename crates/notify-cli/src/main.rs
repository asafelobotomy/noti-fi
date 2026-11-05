//! notify: install, run and control the notifd notification daemon.

mod apps;
mod controls;
mod ctx;
mod daemon;
mod dev;
mod install;
mod integrations;
mod lint;
mod out;
mod paths;
mod payload;
mod status;
mod sys;
mod units;
mod upgrade;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use notify_protocol::Urgency;

use crate::ctx::Ctx;
use crate::out::Out;

#[derive(Parser)]
#[command(name = "notify", version, about = "noti-fi (notification fidelity): install, run and control the notifd notification daemon", long_about = None, propagate_version = true)]
struct Cli {
    /// Machine-readable output: one JSON document for read commands, one JSON object per line (progress events) for commands that change things
    #[arg(long, global = true)]
    json: bool,
    /// Talk to the `notify dev` daemon instead of the real one
    #[arg(long, global = true)]
    dev: bool,
    /// Use daemon files from this folder instead of the ones embedded in this binary (development)
    #[arg(long, global = true, value_name = "DIR")]
    payload: Option<PathBuf>,
    #[arg(long, global = true, hide = true)]
    no_systemctl: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    On,
    Off,
    Toggle,
    Status,
}

#[derive(Clone, Copy, ValueEnum)]
enum Urg {
    Low,
    Normal,
    Critical,
}
impl Urg {
    fn s(self) -> &'static str {
        match self {
            Urg::Low => "low",
            Urg::Normal => "normal",
            Urg::Critical => "critical",
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Toggle {
    Status,
    Install,
    Remove,
}

#[derive(Clone, Copy, ValueEnum)]
enum TrayAction {
    Status,
    Hide,
    Show,
}

#[derive(Subcommand)]
enum Cmd {
    /// Who owns notifications, daemon state
    Status,
    /// Run every check and say what needs attention
    Doctor,
    /// Browsers and apps, and how ready each is for notifications
    Apps,
    /// Validate rules.json (static lint + the running daemon's verdict, without applying a --file)
    Rules {
        /// Check this file instead of the installed one
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// What would the daemon do with this notification? (the daemon decides, with its own rule engine)
    Simulate {
        summary: Option<String>,
        #[arg(long, default_value = "")]
        app: String,
        /// Desktop entry
        #[arg(long, default_value = "")]
        entry: String,
        /// Site name, like a browser sends
        #[arg(long, default_value = "")]
        origin: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(short, long, value_enum, default_value = "normal")]
        urgency: Urg,
        /// The app's requested timeout in ms (-1 none, 0 never)
        #[arg(long, default_value_t = -1, allow_hyphen_values = true)]
        timeout: i64,
        /// Evaluate against a draft rules.json instead of the live one
        #[arg(long)]
        config: Option<PathBuf>,
        /// Ignore Do Not Disturb / mutes / full screen right now
        #[arg(long)]
        ignore_state: bool,
    },
    /// Show real sample popups on screen (optionally with a draft config)
    Preview {
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long, default_value_t = 6)]
        seconds: u32,
        /// Stop a running preview
        #[arg(long)]
        end: bool,
    },
    /// Make notifd the notification owner (reversible)
    Takeover {
        #[arg(long)]
        dry_run: bool,
        /// Also switch the running session (restarts plasmashell briefly)
        #[arg(long)]
        now: bool,
        /// Do not touch portal routing
        #[arg(long)]
        no_portal: bool,
        /// Do not install the full-screen KWin script
        #[arg(long)]
        no_autodnd: bool,
    },
    /// Look for a newer noti-fi release; with --apply, download, verify and install it (nothing is contacted unless you run this)
    Upgrade {
        /// Actually install it (without this, only report what is available)
        #[arg(long)]
        apply: bool,
    },
    /// Install changed daemon files; smoke-tests first and rolls back if notifd will not start
    Update,
    /// Hand notifications back to Plasma
    Restore {
        #[arg(long)]
        dry_run: bool,
        /// Also delete the installed daemon files
        #[arg(long)]
        purge: bool,
        #[arg(long, hide = true)]
        auto: bool,
    },
    /// Route Flatpak/portal notifications through notifd
    Portal {
        #[arg(long)]
        remove: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Make Brave deliver notifications (preview unless --apply)
    Adapt {
        /// Make the changes (Brave must be closed; originals are backed up)
        #[arg(long)]
        apply: bool,
        /// Keep Brave running after its last window closes
        #[arg(long)]
        background: bool,
        /// Start Brave hidden at login
        #[arg(long)]
        autostart: bool,
    },
    /// Global shortcut that toggles Do Not Disturb
    Hotkey {
        #[arg(default_value = "Meta+Shift+N")]
        keys: String,
        #[arg(long)]
        remove: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Automatic Do Not Disturb for full-screen windows (KWin script)
    Autodnd {
        #[arg(value_enum, default_value = "status")]
        action: Toggle,
    },
    /// Show or hide Plasma's (now inert) notification icon in the system tray
    Tray {
        #[arg(value_enum, default_value = "status")]
        action: TrayAction,
    },
    /// Do Not Disturb: on, off, toggle or status
    Dnd {
        #[arg(value_enum, default_value = "status")]
        mode: Mode,
    },
    /// Focus mode (like Apple's): only important notifications get through. No argument = status; `list`; `off`; or a mode name
    Focus {
        /// A mode name, `off` or `list`
        mode: Option<String>,
    },
    /// Silence one zone (still kept in history)
    Mute { zone: String },
    /// Restore a muted zone
    Unmute { zone: String },
    /// Recent notifications
    History {
        #[arg(short, default_value_t = 20)]
        n: usize,
        #[arg(long)]
        zone: Option<String>,
        /// Regex on the app name
        #[arg(long)]
        app: Option<String>,
        /// Only hidden ones (DND, mute, silent rule)
        #[arg(long)]
        missed: bool,
        /// Include message text
        #[arg(long)]
        full: bool,
        /// Erase history
        #[arg(long)]
        clear: bool,
    },
    /// Close every popup on screen
    Dismiss,
    /// Answer a notification that has a reply box (id from `notify history --json`)
    Reply { id: i64, text: String },
    /// Send a notification (for testing rules)
    Send {
        summary: String,
        body: Option<String>,
        #[arg(short, long, default_value = "notify")]
        app: String,
        #[arg(short, long, value_enum, default_value = "normal")]
        urgency: Urg,
        /// ms (0 = never expires)
        #[arg(short, long, default_value_t = 6000)]
        timeout: i64,
        /// Site name, like a browser sends
        #[arg(long)]
        origin: Option<String>,
        #[arg(long)]
        desktop_entry: Option<String>,
    },
    /// Send sample notifications to every zone
    Test {
        #[arg(short, long, default_value_t = 6000)]
        timeout: i64,
    },
    /// Run a throwaway daemon on a private bus
    Dev,
    /// Run the regression suite (needs the project checkout)
    Selftest,
}

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
        Cmd::Takeover { dry_run, now, no_portal, no_autodnd } => install::takeover(&ctx, now, dry_run, no_portal, no_autodnd)?,
        Cmd::Update => install::update(&ctx)?,
        Cmd::Upgrade { apply } => upgrade::upgrade(&ctx, apply)?,
        Cmd::Restore { dry_run, purge, auto } => install::restore(&ctx, dry_run, purge, auto)?,
        Cmd::Portal { remove, dry_run } => install::portal(&ctx, remove, dry_run)?,
        Cmd::Adapt { apply, background, autostart } => integrations::adapt(&ctx, integrations::AdaptArgs { apply, background, autostart })?,
        Cmd::Hotkey { keys, remove, dry_run } => integrations::hotkey(&ctx, &keys, remove, dry_run)?,
        Cmd::Autodnd { action } => integrations::autodnd(&ctx, match action { Toggle::Status => "status", Toggle::Install => "install", Toggle::Remove => "remove" })?,
        Cmd::Tray { action } => integrations::tray(&ctx, match action { TrayAction::Status => "status", TrayAction::Hide => "hide", TrayAction::Show => "show" })?,
        Cmd::Focus { mode } => controls::focus(&ctx, mode.as_deref())?,
        Cmd::Dnd { mode } => controls::dnd(&ctx, match mode { Mode::On => "on", Mode::Off => "off", Mode::Toggle => "toggle", Mode::Status => "status" })?,
        Cmd::Mute { zone } => controls::mute(&ctx, &zone, true)?,
        Cmd::Unmute { zone } => controls::mute(&ctx, &zone, false)?,
        Cmd::History { n, zone, app, missed, full, clear } => controls::history(&ctx, controls::HistoryArgs { n, zone, app, missed, full, clear })?,
        Cmd::Dismiss => controls::dismiss(&ctx)?,
        Cmd::Reply { id, text } => controls::reply(&ctx, id, &text)?,
        Cmd::Send { summary, body, app, urgency, timeout, origin, desktop_entry } => {
            controls::send(&ctx, controls::SendArgs { summary, body, app, urgency: urgency.s().into(), timeout, origin, desktop_entry })?
        }
        Cmd::Test { timeout } => controls::test(&ctx, timeout)?,
        Cmd::Dev => dev::dev(&ctx)?,
        Cmd::Selftest => dev::selftest()?,
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
