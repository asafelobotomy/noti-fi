//! The system tray icon and its menu. The menu is built from a plain `TrayState` (so it can be tested), and a click becomes a `TrayAction`
//! handed to the caller; nothing here knows about Qt. It speaks StatusNotifierItem (what Plasma's tray uses) through `ksni`.

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{CheckmarkItem, MenuItem, StandardItem, SubMenu};

/// Everything the menu shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrayState {
    pub online: bool,
    pub dnd: bool,
    pub active: u32,
    /// The focus mode that is on ("" = none) and the modes there are: (name, label).
    pub focus: String,
    pub focus_modes: Vec<(String, String)>,
    /// (name, label, muted)
    pub zones: Vec<(String, String, bool)>,
    /// One line per recent notification, already worded for display.
    pub recent: Vec<String>,
    /// Unread notifications per group: (label, accent colour, count), in the config's order.
    pub unread: Vec<(String, String, u32)>,
    /// Why popups are held, as the daemon says ("" = they show; "dnd", "focus", "fullscreen", "screen sharing", "inhibited").
    pub holding: String,
}

/// What a click asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAction {
    /// Left click: open or close the notification center (or the window, when notifd is not running).
    Toggle,
    MarkAllRead,
    ClearAll,
    Show,
    OpenPage(String),
    SetDnd(bool),
    SetFocus(String),
    MuteZone(String, bool),
    Quit,
}

pub struct Tray {
    pub state: TrayState,
    on_action: Box<dyn Fn(TrayAction) + Send + Sync>,
}

impl Tray {
    pub fn new(state: TrayState, on_action: impl Fn(TrayAction) + Send + Sync + 'static) -> Tray {
        Tray { state, on_action: Box::new(on_action) }
    }

    pub fn status_line(&self) -> String {
        let s = &self.state;
        if !s.online {
            return "noti-fi: notifd is not running".into();
        }
        let mut parts = vec![if s.dnd { "Do Not Disturb on".to_string() } else { "Do Not Disturb off".to_string() }];
        if !s.focus.is_empty() {
            let label = s.focus_modes.iter().find(|(n, _)| *n == s.focus).map(|(_, l)| l.clone()).unwrap_or_else(|| s.focus.clone());
            parts.push(format!("Focus: {label}"));
        }
        match s.holding.as_str() {
            "fullscreen" => parts.push("popups held: a window is full screen".into()),
            "screen sharing" => parts.push("popups held: the screen is being shared".into()),
            "inhibited" => parts.push("popups held: an app asked for quiet".into()),
            _ => {}
        }
        parts.push(format!("{} on screen", s.active));
        let unread: u32 = s.unread.iter().map(|u| u.2).sum();
        if unread > 0 {
            let per: Vec<String> = s.unread.iter().filter(|u| u.2 > 0).map(|(l, _, n)| format!("{l} {n}")).collect();
            parts.push(format!("{unread} unread ({})", per.join(", ")));
        }
        format!("noti-fi: {}", parts.join(", "))
    }

    /// While anything is unread (and neither Do Not Disturb nor a focus mode has its own icon up), the icon is the unread counter,
    /// as Plasma's own notification icon does: a small overlay would be too small to read.
    pub fn counter_is_icon(&self) -> bool {
        let s = &self.state;
        s.online && !s.dnd && s.focus.is_empty() && s.unread.iter().any(|u| u.2 > 0)
    }

    /// The unread badge at the sizes a tray asks for (none when everything is read).
    pub fn badge(&self) -> Vec<ksni::Icon> {
        let slices: Vec<crate::badge::Slice> =
            self.state.unread.iter().map(|(_, c, n)| crate::badge::Slice { color: crate::badge::parse_color(c), count: *n }).collect();
        [16, 22, 32, 48, 64]
            .into_iter()
            .filter_map(|s| crate::badge::render(&slices, s).map(|data| ksni::Icon { width: s as i32, height: s as i32, data }))
            .collect()
    }
}

fn item(label: &str, icon: &str, act: TrayAction) -> MenuItem<Tray> {
    StandardItem { label: label.into(), icon_name: icon.into(), activate: Box::new(move |t: &mut Tray| (t.on_action)(act.clone())), ..Default::default() }
        .into()
}

fn check(label: &str, checked: bool, enabled: bool, act: TrayAction) -> MenuItem<Tray> {
    CheckmarkItem { label: label.into(), checked, enabled, activate: Box::new(move |t: &mut Tray| (t.on_action)(act.clone())), ..Default::default() }.into()
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        crate::shell::APP_ID.into()
    }
    fn title(&self) -> String {
        "noti-fi".into()
    }
    fn category(&self) -> ksni::Category {
        ksni::Category::SystemServices
    }
    fn icon_name(&self) -> String {
        if self.counter_is_icon() {
            "" // the counter is drawn (icon_pixmap)
        } else if !self.state.online {
            "dialog-warning"
        } else if self.state.dnd {
            "notifications-disabled"
        } else if !self.state.focus.is_empty() {
            "face-glasses"
        } else {
            "preferences-desktop-notification"
        }
        .into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        if self.counter_is_icon() { self.badge() } else { Vec::new() }
    }
    // Do Not Disturb and focus mode keep their icon; unread ones then show as a small badge on it
    fn overlay_icon_pixmap(&self) -> Vec<ksni::Icon> {
        if self.state.online && !self.counter_is_icon() { self.badge() } else { Vec::new() }
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip { title: "noti-fi".into(), description: self.status_line(), icon_name: self.icon_name(), icon_pixmap: self.icon_pixmap() }
    }
    fn activate(&mut self, _x: i32, _y: i32) {
        (self.on_action)(TrayAction::Toggle);
    }
    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        // middle click: Do Not Disturb
        if self.state.online {
            (self.on_action)(TrayAction::SetDnd(!self.state.dnd));
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let s = &self.state;
        let mut m: Vec<MenuItem<Tray>> = vec![
            StandardItem { label: self.status_line(), enabled: false, ..Default::default() }.into(),
            MenuItem::Separator,
            item("Open noti-fi", "window", TrayAction::Show),
            MenuItem::Separator,
            StandardItem {
                label: "Mark all as read".into(),
                icon_name: "mail-mark-read".into(),
                enabled: s.online && s.unread.iter().any(|u| u.2 > 0),
                activate: Box::new(|t: &mut Tray| (t.on_action)(TrayAction::MarkAllRead)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Clear all".into(),
                icon_name: "edit-clear-history".into(),
                enabled: s.online && !s.recent.is_empty(),
                activate: Box::new(|t: &mut Tray| (t.on_action)(TrayAction::ClearAll)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            check("Do Not Disturb", s.dnd, s.online, TrayAction::SetDnd(!s.dnd)),
        ];
        // Focus: Off, then each mode
        let mut focus = vec![check("Off", s.focus.is_empty(), s.online, TrayAction::SetFocus("off".into()))];
        for (name, label) in &s.focus_modes {
            focus.push(check(label, s.focus == *name, s.online, TrayAction::SetFocus(name.clone())));
        }
        m.push(SubMenu { label: "Focus".into(), icon_name: "face-glasses".into(), enabled: s.online, submenu: focus, ..Default::default() }.into());
        if !s.zones.is_empty() {
            let zones = s.zones.iter().map(|(name, label, muted)| check(label, *muted, s.online, TrayAction::MuteZone(name.clone(), !muted))).collect();
            m.push(
                SubMenu { label: "Mute a group".into(), icon_name: "notifications-disabled".into(), enabled: s.online, submenu: zones, ..Default::default() }
                    .into(),
            );
        }
        m.push(MenuItem::Separator);
        let mut recent: Vec<MenuItem<Tray>> = if s.recent.is_empty() {
            vec![StandardItem { label: "Nothing yet".into(), enabled: false, ..Default::default() }.into()]
        } else {
            s.recent.iter().map(|line| item(line, "", TrayAction::OpenPage("history".into()))).collect()
        };
        recent.push(MenuItem::Separator);
        recent.push(item("Show all history…", "view-history", TrayAction::OpenPage("history".into())));
        m.push(SubMenu { label: "Recent notifications".into(), icon_name: "view-history".into(), submenu: recent, ..Default::default() }.into());
        m.push(item("Settings…", "configure", TrayAction::OpenPage("general".into())));
        m.push(MenuItem::Separator);
        m.push(item("Quit noti-fi", "application-exit", TrayAction::Quit));
        m
    }
}

/// A handle to the running tray icon.
pub struct TrayHandle(Handle<Tray>);

impl TrayHandle {
    pub fn update(&self, state: TrayState) {
        self.0.update(|t| {
            if t.state != state {
                t.state = state;
            }
        });
    }
}

/// Put the icon in the system tray. `None` when the desktop has no tray (or no session bus), in which case the panel behaves as a normal window.
pub fn spawn(state: TrayState, on_action: impl Fn(TrayAction) + Send + Sync + 'static) -> Option<TrayHandle> {
    Tray::new(state, on_action).spawn().ok().map(TrayHandle)
}

/// "now", "5 min", "3 h", "2 d": how long ago, for the recent list.
pub fn age_text(now_ms: i64, then_ms: i64) -> String {
    let s = ((now_ms - then_ms).max(0)) / 1000;
    match s {
        0..=89 => "now".into(),
        90..=5399 => format!("{} min", (s + 30) / 60),
        5400..=129_599 => format!("{} h", (s + 1800) / 3600),
        _ => format!("{} d", (s + 43_200) / 86_400),
    }
}

/// One line of the recent-notifications menu, e.g. `5 min  Steam: New message` (the text only when `show_text`).
pub fn recent_line(age: &str, app: &str, summary: &str, shown: bool, show_text: bool) -> String {
    let mut s = format!("{age}  {app}");
    if show_text && !summary.is_empty() {
        s.push_str(": ");
        s.push_str(&summary.chars().take(60).collect::<String>());
    }
    if !shown {
        s.push_str("  (held)");
    }
    s
}

#[cfg(test)]
mod tests;
