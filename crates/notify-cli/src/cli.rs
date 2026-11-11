//! The command line: every subcommand and option `notify` accepts (clap derives the parser and --help from these).

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "notify", version, about = "noti-fi (notification fidelity): install, run and control the notifd notification daemon", long_about = None, propagate_version = true)]
pub(crate) struct Cli {
    /// Machine-readable output: one JSON document for read commands, one JSON object per line (progress events) for commands that change things
    #[arg(long, global = true)]
    pub(crate) json: bool,
    /// Talk to the `notify dev` daemon instead of the real one
    #[arg(long, global = true)]
    pub(crate) dev: bool,
    /// Use daemon files from this folder instead of the ones embedded in this binary (development)
    #[arg(long, global = true, value_name = "DIR")]
    pub(crate) payload: Option<PathBuf>,
    #[arg(long, global = true, hide = true)]
    pub(crate) no_systemctl: bool,
    #[command(subcommand)]
    pub(crate) cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Mode {
    On,
    Off,
    Toggle,
    Status,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Urg {
    Low,
    Normal,
    Critical,
}
impl Urg {
    pub(crate) fn s(self) -> &'static str {
        match self {
            Urg::Low => "low",
            Urg::Normal => "normal",
            Urg::Critical => "critical",
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Toggle {
    Status,
    Install,
    Remove,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum TrayAction {
    Status,
    Hide,
    Show,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum CenterAction {
    Toggle,
    Open,
    Close,
}

/// The settings `notify set` can change (a fixed list: the ones a desktop's settings page offers).
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum SettingKey {
    #[value(name = "history.limit")]
    HistoryLimit,
    #[value(name = "history.persist")]
    HistoryPersist,
    #[value(name = "dnd.allowCritical")]
    DndAllowCritical,
    #[value(name = "autoDnd.fullscreen")]
    AutoDndFullscreen,
    #[value(name = "autoDnd.screenShare")]
    AutoDndScreenShare,
}

#[derive(Subcommand)]
pub(crate) enum Cmd {
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
        /// The notification's title
        summary: Option<String>,
        /// The sending app's name
        #[arg(long, default_value = "")]
        app: String,
        /// Desktop entry
        #[arg(long, default_value = "")]
        entry: String,
        /// Site name, like a browser sends
        #[arg(long, default_value = "")]
        origin: String,
        /// The notification's text
        #[arg(long)]
        body: Option<String>,
        /// How urgent the app says it is
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
        /// Show the popups as this draft rules.json would place them
        #[arg(long)]
        config: Option<PathBuf>,
        /// How long the sample popups stay
        #[arg(long, default_value_t = 6)]
        seconds: u32,
        /// Stop a running preview
        #[arg(long)]
        end: bool,
    },
    /// Make notifd the notification owner (reversible)
    Takeover {
        /// Show what would change without changing it
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
        /// Install as a plain user service, without taking over from Plasma (automatic outside KDE, e.g. on Hyprland)
        #[arg(long)]
        plain: bool,
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
        /// Show what would change without changing it
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
        /// Stop routing portal notifications through notifd
        #[arg(long)]
        remove: bool,
        /// Show what would change without changing it
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
    /// Global shortcut that toggles Do Not Disturb (or, with --center, opens the notification list)
    Hotkey {
        /// The key combination, in KDE's notation [default: Meta+Shift+N, or Meta+N with --center]
        keys: Option<String>,
        /// The shortcut for the notification list instead of Do Not Disturb
        #[arg(long)]
        center: bool,
        /// Remove the shortcut
        #[arg(long)]
        remove: bool,
        /// Show what would change without changing it
        #[arg(long)]
        dry_run: bool,
    },
    /// Automatic Do Not Disturb for full-screen windows (KWin script)
    Autodnd {
        /// Show whether it is installed, install it, or remove it
        #[arg(value_enum, default_value = "status")]
        action: Toggle,
    },
    /// Show or hide Plasma's (now inert) notification icon in the system tray
    Tray {
        /// Show whether the icon is hidden, hide it, or show it again
        #[arg(value_enum, default_value = "status")]
        action: TrayAction,
    },
    /// The settings `notify set` changes, as the daemon runs them now, plus the names of your rules and zones
    Settings,
    /// Change one setting in rules.json (the daemon checks it first; nothing is written if it refuses): history.limit 50|100|200|500|1000, history.persist, dnd.allowCritical, autoDnd.fullscreen, autoDnd.screenShare on|off
    Set {
        /// Which setting
        #[arg(value_enum)]
        key: SettingKey,
        /// 50, 100, 200, 500 or 1000 for history.limit; on or off for the others
        value: String,
    },
    /// Do Not Disturb: on, off, toggle or status
    Dnd {
        /// Turn it on, off, flip it, or show whether it is on
        #[arg(value_enum, default_value = "status")]
        mode: Mode,
    },
    /// Focus mode (like Apple's): only important notifications get through. No argument = status; `list`; `off`; or a mode name
    Focus {
        /// A mode name, `off` or `list`
        mode: Option<String>,
    },
    /// Silence one zone (still kept in history)
    Mute {
        /// The zone's id (see `notify status`)
        zone: String,
    },
    /// Restore a muted zone
    Unmute {
        /// The zone's id (see `notify status`)
        zone: String,
    },
    /// Recent notifications
    History {
        /// How many to show, newest first
        #[arg(short, default_value_t = 20)]
        n: usize,
        /// Only this zone
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
    /// Open or close the notification list (what clicking the tray icon does)
    Center {
        /// Open it, close it, or flip it
        #[arg(value_enum, default_value = "toggle")]
        action: CenterAction,
    },
    /// Close every popup on screen
    Dismiss,
    /// Answer a notification that has a reply box (id from `notify history --json`)
    Reply {
        /// The notification's id
        id: i64,
        /// What to answer
        text: String,
    },
    /// Send a notification (for testing rules)
    Send {
        /// The title
        summary: String,
        /// The text
        body: Option<String>,
        /// The app name to send it as
        #[arg(short, long, default_value = "notify")]
        app: String,
        /// How urgent it is
        #[arg(short, long, value_enum, default_value = "normal")]
        urgency: Urg,
        /// ms (0 = never expires)
        #[arg(short, long, default_value_t = 6000)]
        timeout: i64,
        /// Site name, like a browser sends
        #[arg(long)]
        origin: Option<String>,
        /// The desktop entry to send it as (for rules that match on it)
        #[arg(long)]
        desktop_entry: Option<String>,
    },
    /// Send sample notifications to every zone
    Test {
        /// How long each sample stays, in ms (0 = until dismissed)
        #[arg(short, long, default_value_t = 6000)]
        timeout: i64,
    },
    /// Run a throwaway daemon on a private bus
    Dev,
    /// Show Dolphin's (and every KDE app's) copy, move and delete progress through notifd (notifd-jobs.service runs this)
    Jobs,
    /// Run the regression suite (needs the project checkout)
    Selftest,
    /// Run notifd behind the notification name, adding Inhibit (notifd.service runs this; --payload picks the daemon files)
    Serve,
}
