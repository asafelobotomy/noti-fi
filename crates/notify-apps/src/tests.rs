use super::*;

fn tmp(label: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("notify-apps-{label}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&d).unwrap();
    d
}

fn fixture() -> Roots {
    let root = tmp("fx");
    let apps = root.join("share/applications");
    fs::create_dir_all(&apps).unwrap();
    fs::create_dir_all(root.join("share/knotifications6")).unwrap();
    fs::create_dir_all(root.join("config")).unwrap();
    let w = |n: &str, t: &str| fs::write(apps.join(n), t).unwrap();
    w("org.telegram.desktop.desktop", "[Desktop Entry]\nType=Application\nName=Telegram\nIcon=telegram\nCategories=Network;InstantMessaging;\nX-GNOME-UsesNotifications=true\n");
    w("calc.desktop", "[Desktop Entry]\nType=Application\nName=Calculator\nIcon=calc\nCategories=Utility;\n");
    w("hidden.desktop", "[Desktop Entry]\nType=Application\nName=Hidden\nNoDisplay=true\nX-GNOME-UsesNotifications=true\n");
    w("mailer.desktop", "[Desktop Entry]\nType=Application\nName=Mailer\nCategories=Office;Email;\n[Desktop Action x]\nName=Compose\n");
    fs::write(root.join("share/knotifications6/bluedevil.notifyrc"), "[Global]\nIconName=bluetooth\nName=Bluetooth\nDesktopEntry=org.kde.bluedevilwizard\n").unwrap();
    fs::write(root.join("config/plasmanotifyrc"), "[Applications][brave-origin]\nSeen=true\n\n[Applications][org.telegram.desktop]\nSeen=true\n").unwrap();
    Roots { data_dirs: vec![root.join("share")], config_dir: root.join("config"), flatpak: false }
}

fn seen(app: &str, entry: &str, count: u64, last: i64) -> SeenApp {
    SeenApp { key: if entry.is_empty() { app.to_lowercase() } else { entry.to_lowercase() }, app: app.into(), desktop_entry: entry.into(), count, last, types: Vec::new() }
}

#[test]
fn finds_capable_apps_and_skips_hidden_and_plain_ones() {
    let apps = merge(&[], &discover(&fixture()));
    let ids: Vec<&str> = apps.iter().map(|a| a.id.as_str()).collect();
    assert!(ids.contains(&"org.telegram.desktop"), "{ids:?}");
    assert!(ids.contains(&"mailer"), "an email app is capable by category: {ids:?}");
    assert!(ids.contains(&"org.kde.bluedevilwizard"), "a KDE app with notifyrc: {ids:?}");
    assert!(ids.contains(&"brave-origin"), "Plasma has seen it: {ids:?}");
    assert!(!ids.contains(&"calc") && !ids.contains(&"hidden"), "{ids:?}");
    let tg = apps.iter().find(|a| a.id == "org.telegram.desktop").unwrap();
    assert!(tg.reasons.contains(&Reason::Declared) && tg.reasons.contains(&Reason::Plasma) && !tg.sent, "{tg:?}");
}

#[test]
fn sent_apps_come_first_and_match_by_entry_or_name() {
    let d = discover(&fixture());
    let apps = merge(&[seen("Telegram Desktop", "org.telegram.desktop", 5, 2000), seen("Mailer", "", 2, 3000), seen("Mystery", "", 1, 1000)], &d);
    let sent: Vec<&str> = apps.iter().take_while(|a| a.sent).map(|a| a.id.as_str()).collect();
    assert_eq!(sent, vec!["mailer", "org.telegram.desktop", "mystery"], "newest first, then the unknown app: {apps:?}");
    assert!(apps[3..].iter().all(|a| !a.sent));
    assert_eq!(apps.iter().filter(|a| a.id == "org.telegram.desktop").count(), 1, "no duplicates");
    let tg = apps.iter().find(|a| a.id == "org.telegram.desktop").unwrap();
    assert_eq!(tg.count, 5);
    assert_eq!(tg.name, "Telegram", "the launcher's name wins");
    assert_eq!((tg.match_field.as_str(), tg.match_value.as_str()), ("desktopEntry", "org.telegram.desktop"));
    let ml = apps.iter().find(|a| a.id == "mailer").unwrap();
    assert_eq!((ml.match_field.as_str(), ml.match_value.as_str()), ("appName", "Mailer"), "no desktop entry was sent, so match the name");
    let m = apps.iter().find(|a| a.id == "mystery").unwrap();
    assert!(m.capable, "an app that sent a notification is capable");
}

#[test]
fn notification_types_are_carried_to_the_app() {
    let d = discover(&fixture());
    let mut a = seen("Mailer", "", 3, 3000);
    a.types = vec![
        SeenType { key: "general|app".into(), category: String::new(), channel: "app".into(), caps: vec!["icon".into()], count: 1, last: 10 },
        SeenType { key: "email.arrived|app".into(), category: "email.arrived".into(), channel: "app".into(), caps: vec!["actions".into()], count: 2, last: 20 },
    ];
    let apps = merge(&[a], &d);
    let m = apps.iter().find(|x| x.id == "mailer").unwrap();
    assert_eq!(m.types.iter().map(|t| t.key.as_str()).collect::<Vec<_>>(), vec!["email.arrived|app", "general|app"], "most used first");
}

#[test]
fn flatpak_permissions_are_attached() {
    let mut d = discover(&fixture());
    d.permissions.insert("org.flatpak.game".into(), false);
    d.flatpaks.push(("org.flatpak.game".into(), "Game".into()));
    d.flatpaks.push(("org.flatpak.quiet".into(), "Quiet".into()));
    let apps = merge(&[], &d);
    let g = apps.iter().find(|a| a.id == "org.flatpak.game").unwrap();
    assert!(g.flatpak && g.flatpak_permission == Some(false) && g.capable);
    let q = apps.iter().find(|a| a.id == "org.flatpak.quiet");
    assert!(q.is_none(), "a Flatpak with no notification permission and no evidence is not listed as capable");
}

#[test]
fn flatpak_ids_are_validated() {
    assert!(set_flatpak_permission("", Some(true)).is_err());
    assert!(set_flatpak_permission("a b", Some(true)).is_err());
    assert!(set_flatpak_permission("--help", None).is_err(), "an id must not look like an option");
    assert!(set_flatpak_permission("../x; rm", None).is_err());
}
