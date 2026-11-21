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
        unread: vec![("Personal".into(), "#60a5fa".into(), 2), ("System".into(), "#fbbf24".into(), 1)],
        holding: "dnd".into(),
    }
}

#[test]
fn the_tooltip_says_why_popups_are_held() {
    for (holding, says) in [("screen sharing", "the screen is being shared"), ("inhibited", "an app asked"), ("fullscreen", "full screen")] {
        let mut s = state();
        s.dnd = false;
        s.holding = holding.into();
        let t = Tray::new(s, |_| {});
        assert!(t.status_line().contains(says), "{holding}: {}", t.status_line());
    }
}

fn tray(actions: Arc<Mutex<Vec<TrayAction>>>) -> Tray {
    Tray::new(state(), move |a| actions.lock().unwrap().push(a))
}

#[test]
fn the_menu_shows_the_status_and_lets_it_be_changed() {
    let t = tray(Default::default());
    let l = labels(&t.menu());
    assert_eq!(l[0], "noti-fi: Do Not Disturb on, Focus: Urgent only, 2 on screen, 3 unread (Personal 2, System 1)");
    for want in [
        "Do Not Disturb [x]",
        "Focus >",
        "  Off [ ]",
        "  Urgent only [x]",
        "  Work [ ]",
        "Mute a group >",
        "  Business [x]",
        "  Personal [ ]",
        "Mark all as read",
        "Clear all",
        "Recent notifications >",
        "  2 min  Steam: New message",
        "  1 h  Brave  (held)",
        "  Show all history…",
        "Settings…",
        "Quit noti-fi",
    ] {
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
    run("Mark all as read");
    run("Clear all");
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
            TrayAction::MarkAllRead,
            TrayAction::ClearAll,
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
    let menu = t.menu();
    let dnd = menu.iter().find_map(|i| match i {
        MenuItem::Checkmark(c) if c.label == "Do Not Disturb" => Some(c),
        _ => None,
    });
    assert!(!dnd.expect("a Do Not Disturb checkbox").enabled, "the Do Not Disturb item is disabled when notifd is not running");
    let mark = menu.iter().find_map(|i| match i {
        MenuItem::Standard(s) if s.label == "Mark all as read" => Some(s),
        _ => None,
    });
    assert!(!mark.expect("a Mark all as read item").enabled);
    assert_eq!(t.icon_name(), "dialog-warning");
    assert!(t.overlay_icon_pixmap().is_empty(), "no badge without a daemon");
}

#[test]
fn unread_notifications_become_the_icon_or_a_badge_on_it() {
    let mut t = tray(Default::default());
    // Do Not Disturb is on in the test state: its icon stays, the unread count is a badge on it
    assert_eq!(t.icon_name(), "notifications-disabled");
    let sizes: Vec<i32> = t.overlay_icon_pixmap().iter().map(|i| i.width).collect();
    assert_eq!(sizes, [16, 22, 32, 48, 64]);
    assert!(t.overlay_icon_pixmap().iter().all(|i| i.data.len() == (i.width * i.height * 4) as usize));
    // otherwise the counter is the icon
    t.state.dnd = false;
    t.state.focus.clear();
    assert_eq!(t.icon_name(), "");
    assert_eq!(t.icon_pixmap().len(), 5);
    assert!(t.overlay_icon_pixmap().is_empty());
    t.state.unread.clear();
    assert_eq!(t.icon_name(), "preferences-desktop-notification", "the usual icon once everything is read");
    assert!(t.icon_pixmap().is_empty() && t.overlay_icon_pixmap().is_empty());
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
