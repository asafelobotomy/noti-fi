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
}

/// What a click asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayAction {
    /// Left click: show the window, or hide it if it is in front.
    Toggle,
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
        parts.push(format!("{} on screen", s.active));
        format!("noti-fi: {}", parts.join(", "))
    }
}

fn item(label: &str, icon: &str, act: TrayAction) -> MenuItem<Tray> {
    StandardItem { label: label.into(), icon_name: icon.into(), activate: Box::new(move |t: &mut Tray| (t.on_action)(act.clone())), ..Default::default() }.into()
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
        if !self.state.online {
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
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip { title: "noti-fi".into(), description: self.status_line(), icon_name: self.icon_name(), icon_pixmap: Vec::new() }
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
            m.push(SubMenu { label: "Mute a zone".into(), icon_name: "notifications-disabled".into(), enabled: s.online, submenu: zones, ..Default::default() }.into());
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
mod tests {
    use super::*;
    use ksni::Tray as _;
    use std::sync::{Arc, Mutex};

    fn labels(items: &[MenuItem<Tray>]) -> Vec<String> {
        items
            .iter()
            .flat_map(|i| match i {
                MenuItem::Standard(s) => vec![s.label.clone()],
                MenuItem::Checkmark(c) => vec![format!("{} [{}]", c.label, if c.checked { "x" } else { " " })],
                MenuItem::SubMenu(m) => std::iter::once(format!("{} >", m.label)).chain(labels(&m.submenu).into_iter().map(|l| format!("  {l}"))).collect(),
                MenuItem::Separator => vec!["-".to_string()],
                _ => vec![],
            })
            .collect()
    }

    fn state() -> TrayState {
        TrayState {
            online: true,
            dnd: true,
            active: 2,
            focus: "urgent".into(),
            focus_modes: vec![("urgent".into(), "Urgent only".into()), ("work".into(), "Work".into())],
            zones: vec![("business".into(), "Business".into(), true), ("personal".into(), "Personal".into(), false)],
            recent: vec![recent_line("2 min", "Steam", "New message", true, true), recent_line("1 h", "Brave", "x", false, false)],
        }
    }

    fn tray(actions: Arc<Mutex<Vec<TrayAction>>>) -> Tray {
        Tray::new(state(), move |a| actions.lock().unwrap().push(a))
    }

    #[test]
    fn the_menu_shows_the_status_and_lets_it_be_changed() {
        let t = tray(Default::default());
        let l = labels(&t.menu());
        assert_eq!(l[0], "noti-fi: Do Not Disturb on, Focus: Urgent only, 2 on screen");
        for want in ["Do Not Disturb [x]", "Focus >", "  Off [ ]", "  Urgent only [x]", "  Work [ ]", "Mute a zone >", "  Business [x]", "  Personal [ ]", "Recent notifications >", "  2 min  Steam: New message", "  1 h  Brave  (held)", "  Show all history…", "Settings…", "Quit noti-fi"] {
            assert!(l.iter().any(|x| x == want), "missing `{want}` in {l:#?}");
        }
        assert_eq!(t.icon_name(), "notifications-disabled", "Do Not Disturb has its own icon");
    }

    #[test]
    fn clicking_menu_items_asks_for_the_right_things() {
        let seen: Arc<Mutex<Vec<TrayAction>>> = Default::default();
        let mut t = tray(seen.clone());
        let menu = t.menu();
        let mut run = |wanted: &str| {
            fn find<'a>(items: &'a [MenuItem<Tray>], wanted: &str) -> Option<&'a MenuItem<Tray>> {
                items.iter().find_map(|i| match i {
                    MenuItem::Standard(s) if s.label == wanted => Some(i),
                    MenuItem::Checkmark(c) if c.label == wanted => Some(i),
                    MenuItem::SubMenu(m) => find(&m.submenu, wanted),
                    _ => None,
                })
            }
            match find(&menu, wanted).unwrap_or_else(|| panic!("no item {wanted}")) {
                MenuItem::Standard(s) => (s.activate)(&mut t),
                MenuItem::Checkmark(c) => (c.activate)(&mut t),
                _ => unreachable!(),
            }
        };
        run("Do Not Disturb");
        run("Work");
        run("Off");
        run("Business");
        run("Personal");
        run("Open noti-fi");
        run("Show all history…");
        run("Settings…");
        run("Quit noti-fi");
        assert_eq!(
            *seen.lock().unwrap(),
            vec![
                TrayAction::SetDnd(false),
                TrayAction::SetFocus("work".into()),
                TrayAction::SetFocus("off".into()),
                TrayAction::MuteZone("business".into(), false),
                TrayAction::MuteZone("personal".into(), true),
                TrayAction::Show,
                TrayAction::OpenPage("history".into()),
                TrayAction::OpenPage("general".into()),
                TrayAction::Quit,
            ]
        );
        t.activate(0, 0);
        t.secondary_activate(0, 0);
        let last: Vec<_> = seen.lock().unwrap().iter().rev().take(2).cloned().collect();
        assert_eq!(last, vec![TrayAction::SetDnd(false), TrayAction::Toggle], "left click toggles the window; middle click flips Do Not Disturb");
    }

    #[test]
    fn without_a_daemon_the_menu_is_inert_and_says_so() {
        let mut t = tray(Default::default());
        t.state = TrayState::default();
        let l = labels(&t.menu());
        assert_eq!(l[0], "noti-fi: notifd is not running");
        assert!(l.contains(&"Do Not Disturb [ ]".to_string()));
        match &t.menu()[4] {
            MenuItem::Checkmark(c) => assert!(!c.enabled, "the Do Not Disturb item is disabled when notifd is not running"),
            _ => panic!("the fifth item should be the Do Not Disturb checkbox"),
        }
        assert_eq!(t.icon_name(), "dialog-warning");
    }

    #[test]
    fn ages_are_worded_briefly() {
        assert_eq!(age_text(10_000, 5_000), "now");
        assert_eq!(age_text(600_000, 0), "10 min");
        assert_eq!(age_text(3 * 3_600_000, 0), "3 h");
        assert_eq!(age_text(3 * 86_400_000, 0), "3 d");
        assert_eq!(age_text(0, 5_000), "now", "a clock that runs backwards must not panic");
    }

    #[test]
    fn message_text_appears_in_the_recent_list_only_when_allowed() {
        assert_eq!(recent_line("now", "Steam", "secret", true, false), "now  Steam");
        assert_eq!(recent_line("now", "Steam", "hi", true, true), "now  Steam: hi");
    }
}
