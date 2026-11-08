use super::*;
use std::os::unix::fs::symlink;

const SAMPLE: &str = r#"{
  "defaultZone": "personal",
  "futureSetting": { "keep": "me" },
  "zones": { "business": {"anchor": "top-left"}, "personal": {"anchor": "top-right"} },
  "rules": [
{ "name": "Business sites", "match": { "origin": "example-shop|example-chat" }, "zone": "business", "extra": 1 },
{ "name": "Critical", "match": { "urgency": "critical" }, "timeout": 0 },
{ "match": { "appName": "^kernel$" }, "silent": true }
  ]
}
"#;

fn tmpdir(label: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("notify-draft-{label}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn settings_show_defaults_and_set_by_path() {
    let mut d = Draft::from_raw(SAMPLE).unwrap();
    let v = d.settings_view();
    assert_eq!(v["values"]["theme.card"], "#1d1f27", "a missing key shows the daemon's default");
    assert_eq!(v["values"]["maxVisible"], 5);
    assert_eq!(v["values"]["center.anchor"], "bottom-right", "the notification list's corner defaults to the tray's");
    assert_eq!(v["zones"].as_array().unwrap().len(), 2);
    assert_eq!(v["zones"][0]["rules"], 1, "business is used by one rule");
    d.set_setting("theme.card", "#000000").unwrap();
    d.set_setting("maxVisible", "3").unwrap();
    d.set_setting("sound.enabled", "false").unwrap();
    assert_eq!(d.value()["theme"]["card"], "#000000");
    assert_eq!(d.value()["maxVisible"], 3);
    assert_eq!(d.value()["sound"]["enabled"], false);
    assert_eq!(d.value()["futureSetting"]["keep"], "me", "unknown keys survive");
    d.set_setting("theme.card", "").unwrap();
    assert!(d.value()["theme"].get("card").is_none(), "empty text removes the key");
    assert!(d.set_setting("maxVisible", "lots").is_err());
    assert!(d.set_setting("maxVisible", "-1").is_err());
    assert!(d.set_setting("sound.enabled", "maybe").is_err());
    assert!(d.set_setting("nonsense", "1").is_err());
}

#[test]
fn zones_can_be_added_edited_and_deleted_safely() {
    let mut d = Draft::from_raw(SAMPLE).unwrap();
    let id = d.add_zone();
    assert_eq!(id, "zone3");
    d.set_zone_field(&id, "anchor", "center").unwrap();
    assert!(d.set_zone_field(&id, "anchor", "somewhere").is_err());
    d.set_zone_position(&id, 0.25, 1.7).unwrap();
    assert_eq!((d.value()["zones"][&id]["anchor"].as_str(), d.value()["zones"][&id]["x"].as_f64(), d.value()["zones"][&id]["y"].as_f64()), (Some("free"), Some(0.25), Some(1.0)), "clamped to the screen");
    d.set_zone_field(&id, "anchor", "top-center").unwrap();
    assert!(d.value()["zones"][&id].get("x").is_none(), "leaving free drops the position");
    d.set_zone_field(&id, "anchor", "free").unwrap();
    assert_eq!(d.value()["zones"][&id]["x"], 0.5, "switching to free starts in the middle");
    d.set_zone_field(&id, "anchor", "bottom-left").unwrap();
    let anchor = d.value()["zones"][&id]["anchor"].as_str().unwrap().to_string();
    assert!(["bottom-right", "bottom-left"].contains(&anchor.as_str()), "a free corner is chosen: {anchor}");
    d.set_zone_field(&id, "label", "Games").unwrap();
    d.set_zone_field(&id, "anchor", "top-left").unwrap();
    assert_eq!(d.value()["zones"][&id]["label"], "Games");
    assert!(d.set_zone_field(&id, "colour", "x").is_err());
    assert!(d.set_zone_field("nope", "label", "x").is_err());
    d.set_zone_field(&id, "label", "").unwrap();
    assert!(d.value()["zones"][&id].get("label").is_none());
    assert!(d.delete_zone("personal").is_err(), "the default zone cannot be deleted");
    assert_eq!(d.delete_zone("business").unwrap(), 1, "one rule loses its zone");
    assert!(d.rules()[0].zone.is_empty());
    assert!(d.delete_zone(&id).is_ok());
    assert!(d.delete_zone("personal").is_err(), "the last zone cannot be deleted");
}

#[test]
fn per_app_rules_are_created_updated_and_tidied_away() {
    let mut d = Draft::from_raw(SAMPLE).unwrap();
    assert!(!d.app_routing("appName", "Steam").exists);
    d.set_app_routing("Steam", "appName", "Steam", "muted", "true").unwrap();
    assert_eq!(d.rule_count(), 4);
    assert_eq!(d.value()["rules"][0]["name"], "App: Steam", "the app's rule goes first so it wins");
    assert_eq!(d.value()["rules"][0]["match"]["appName"], "^Steam$");
    d.set_app_routing("Steam", "appName", "Steam", "zone", "business").unwrap();
    d.set_app_routing("Telegram", "desktopEntry", "org.telegram.desktop", "timeout", "3000").unwrap();
    assert_eq!(d.rule_count(), 5);
    assert_eq!(d.value()["rules"][1]["name"], "App: Telegram", "managed rules stay together, before the user's own");
    assert_eq!(d.value()["rules"][1]["match"]["desktopEntry"], "^org\\.telegram\\.desktop$", "regex characters are escaped");
    let r = d.app_routing("appName", "Steam");
    assert!(r.exists && r.muted && r.zone == "business", "{r:?}");
    d.set_app_routing("Steam", "appName", "Steam", "muted", "false").unwrap();
    d.set_app_routing("Steam", "appName", "Steam", "zone", "").unwrap();
    assert!(!d.app_routing("appName", "Steam").exists, "a rule that does nothing is removed");
    assert_eq!(d.rule_count(), 4);
    d.set_app_routing("Nobody", "appName", "Nobody", "muted", "false").unwrap();
    assert_eq!(d.rule_count(), 4, "turning off something that was never on adds nothing");
    assert!(d.set_app_routing("X", "nonsense", "x", "muted", "true").is_err());
    assert!(d.set_app_routing("X", "appName", "x", "muted", "maybe").is_err());
    assert!(d.set_app_routing("X", "appName", "x", "timeout", "soon").is_err());
    assert!(d.set_app_routing("X", "appName", "x", "colour", "red").is_err());
    assert_eq!(d.value()["rules"][1]["name"], "Business sites", "the user's own rules are untouched");
}

#[test]
fn style_keys_are_set_on_rules_zones_apps_and_kinds_of_notification() {
    let mut d = Draft::from_raw(SAMPLE).unwrap();
    d.set_rule_style(0, "opacity", "0.6").unwrap();
    d.set_rule_style(0, "clickThrough", "true").unwrap();
    d.set_rule_style(0, "noFocus", "false").unwrap();
    d.set_rule_style(0, "width", "480").unwrap();
    d.set_rule_style(0, "fontFamily", "Serif").unwrap();
    d.set_rule_style(0, "group", "Sites").unwrap();
    let r = &d.value()["rules"][0];
    assert_eq!((r["opacity"].as_f64(), r["clickThrough"].as_bool(), r["noFocus"].as_bool(), r["width"].as_i64()), (Some(0.6), Some(true), Some(false), Some(480)));
    assert_eq!(d.rules()[0].style["clickThrough"], "true");
    assert_eq!(d.rules()[0].style["noFocus"], "false", "an explicit false is kept: it overrides a later rule");
    d.set_rule_style(0, "clickThrough", "").unwrap();
    assert!(d.value()["rules"][0].get("clickThrough").is_none(), "empty text removes the key");
    assert!(d.set_rule_style(0, "opacity", "clear").is_err());
    assert!(d.set_rule_style(0, "flavour", "x").is_err());
    assert!(d.set_rule_style(0, "noFocus", "perhaps").is_err());

    d.set_zone_style("personal", "opacity", "0.5").unwrap();
    assert_eq!(d.value()["zones"]["personal"]["opacity"], 0.5);
    assert!(d.set_zone_style("personal", "group", "x").is_err(), "a zone has no group");
    assert!(d.set_zone_style("ghost", "opacity", "0.5").is_err());
    assert_eq!(d.settings_view()["zones"][1]["style"]["opacity"], "0.5");
    d.set_setting("theme.opacity", "0.9").unwrap();
    d.set_setting("theme.fontFamily", "Noto Sans").unwrap();
    d.set_setting("theme.fontSize", "14").unwrap();
    assert_eq!((d.value()["theme"]["opacity"].as_f64(), d.value()["theme"]["fontSize"].as_i64()), (Some(0.9), Some(14)));

    // per app, and per kind of notification of an app: the kind's rule goes first
    d.set_routing("Mailer", &[("appName", "Mailer")], "clickThrough", "true").unwrap();
    d.set_routing("Mailer mail", &[("appName", "Mailer"), ("category", "email.arrived")], "group", "Mail").unwrap();
    d.set_routing("Mailer mail", &[("appName", "Mailer"), ("category", "email.arrived")], "zone", "business").unwrap();
    let names: Vec<&str> = d.value()["rules"].as_array().unwrap().iter().filter_map(|r| r["name"].as_str()).collect();
    assert_eq!(&names[..2], &["App: Mailer mail", "App: Mailer"], "more specific first, before the user's rules: {names:?}");
    assert_eq!(d.routing_for(&[("appName", "Mailer"), ("category", "email.arrived")]).style["group"], "Mail");
    assert_eq!(d.routing_for(&[("appName", "Mailer")]).style["clickThrough"], "true");
    d.set_routing("Mailer mail", &[("appName", "Mailer"), ("category", "email.arrived")], "group", "").unwrap();
    d.set_routing("Mailer mail", &[("appName", "Mailer"), ("category", "email.arrived")], "zone", "").unwrap();
    assert!(!d.routing_for(&[("appName", "Mailer"), ("category", "email.arrived")]).exists, "an emptied rule is removed");
    assert!(d.routing_for(&[("appName", "Mailer")]).exists, "the app's own rule is untouched");
}

#[test]
fn focus_modes_are_edited_as_text() {
    let mut d = Draft::from_raw("{\"zones\": {\"a\": {\"anchor\": \"top-left\"}}}").unwrap();
    assert_eq!(d.focus_view()[0]["name"], "urgent", "the built-in mode is shown when the file has no focus section");
    let n = d.add_focus_mode();
    assert_eq!(n, "mode2", "the built-in mode counts, and is written out with the first edit");
    assert!(d.value()["focus"]["modes"].get("urgent").is_some());
    d.set_focus_field(&n, "label", "Work").unwrap();
    d.set_focus_field(&n, "minPriority", "high").unwrap();
    d.set_focus_field(&n, "allow", "^Signal$\norigin:calendar and category:^email\n\n").unwrap();
    let m = &d.value()["focus"]["modes"][&n];
    assert_eq!((m["label"].as_str(), m["minPriority"].as_str()), (Some("Work"), Some("high")));
    assert_eq!(m["allow"][0]["appName"], "^Signal$", "a bare pattern means the app name");
    assert_eq!((m["allow"][1]["origin"].as_str(), m["allow"][1]["category"].as_str()), (Some("calendar"), Some("^email")));
    let v = d.focus_view();
    assert_eq!(v[1]["allow"], "appName:^Signal$\norigin:calendar and category:^email", "round-trips as text");
    assert!(d.set_focus_field(&n, "minPriority", "mega").is_err());
    assert!(d.set_focus_field(&n, "allow", "appName:").is_err(), "an empty pattern is refused");
    assert!(d.set_focus_field("ghost", "label", "x").is_err());
    assert!(d.set_focus_field(&n, "colour", "x").is_err());
    d.set_focus_field(&n, "allow", "").unwrap();
    assert!(d.value()["focus"]["modes"][&n].get("allow").is_none(), "an empty list is removed");
    d.delete_focus_mode(&n).unwrap();
    assert!(d.delete_focus_mode(&n).is_err());
}

#[test]
fn rejects_text_that_is_not_an_object() {
    assert!(Draft::from_raw("{nope").is_err());
    assert!(Draft::from_raw("[1,2]").unwrap_err().contains("object"));
}

#[test]
fn views_summarise_each_rule() {
    let d = Draft::from_raw(SAMPLE).unwrap();
    let v = d.rules();
    assert_eq!(v.len(), 3);
    assert_eq!(v[0].title, "Business sites");
    assert_eq!(v[0].summary, "origin ~ example-shop|example-chat  →  business");
    assert_eq!(v[0].matches["origin"], "example-shop|example-chat");
    assert_eq!(v[0].matches["appName"], "");
    assert_eq!(v[1].timeout, "0");
    assert!(v[1].summary.contains("stays until dismissed"));
    assert_eq!(v[2].title, "(unnamed rule)");
    assert!(v[2].silent && v[2].summary.ends_with("silent"));
    assert_eq!(d.zone_names(), ["business", "personal"]);
}

#[test]
fn editing_changes_only_what_was_asked() {
    let mut d = Draft::from_raw(SAMPLE).unwrap();
    assert!(!d.is_dirty());
    d.set_match(0, "appName", "^steam$");
    d.set_match(0, "origin", "");
    d.set_zone(0, Some("personal"));
    d.set_timeout(0, Some(1500.0));
    d.set_name(0, "Steam");
    assert!(d.is_dirty());
    let r = &d.rules()[0];
    assert_eq!((r.name.as_str(), r.zone.as_str(), r.timeout.as_str()), ("Steam", "personal", "1500"));
    assert_eq!(r.matches["appName"], "^steam$");
    assert_eq!(r.matches["origin"], "");
    // unknown keys survive, in the rule and at the top level
    assert_eq!(d.value()["rules"][0]["extra"], 1);
    assert_eq!(d.value()["futureSetting"]["keep"], "me");
    // whole-number timeouts are written as integers
    assert!(d.to_text().contains("\"timeout\": 1500,") || d.to_text().contains("\"timeout\": 1500\n"), "{}", d.to_text());
    assert!(!d.to_text().contains("1500.0"));
    d.set_zone(0, None);
    d.set_timeout(0, None);
    d.set_silent(0, true);
    let r = &d.rules()[0];
    assert!(r.zone.is_empty() && r.timeout.is_empty() && r.silent);
    d.set_silent(0, false);
    assert!(!d.rules()[0].silent);
}

#[test]
fn add_delete_move_duplicate_keep_positions_straight() {
    let mut d = Draft::from_raw(SAMPLE).unwrap();
    let n = d.add_rule();
    assert_eq!((n, d.rule_count()), (3, 4));
    assert_eq!(d.rules()[3].summary, "everything  (no effect yet)");
    assert_eq!(d.move_rule(3, -1), 2);
    assert_eq!(d.rules()[2].title, "New rule");
    assert_eq!(d.move_rule(0, -1), 0, "moving the first rule up does nothing");
    assert_eq!(d.move_rule(3, 1), 3, "moving the last rule down does nothing");
    assert_eq!(d.duplicate_rule(0), Some(1));
    assert_eq!(d.rules()[1].title, "Business sites (copy)");
    d.delete_rule(1);
    assert_eq!(d.rule_count(), 4);
    d.delete_rule(99);
    assert_eq!(d.rule_count(), 4);
}

#[test]
fn revert_and_mark_saved() {
    let mut d = Draft::from_raw(SAMPLE).unwrap();
    d.add_rule();
    assert!(d.is_dirty());
    d.revert();
    assert!(!d.is_dirty() && d.rule_count() == 3);
    d.add_rule();
    let text = d.to_text();
    d.mark_saved(&text);
    assert!(!d.is_dirty() && d.base_raw() == text && d.rule_count() == 4);
}

#[test]
fn maps_daemon_errors_to_a_rule() {
    let d = Draft::from_raw(SAMPLE).unwrap();
    assert_eq!(d.error_index("rule 'Critical': unknown match field 'x'"), Some(1));
    assert_eq!(d.error_index("rule #3: bad regex for appName"), Some(2));
    assert_eq!(d.error_index("rule #9: whatever"), None);
    assert_eq!(d.error_index("defaultZone 'x' is not a defined zone"), None);
    assert_eq!(d.error_index("rule 'Nope': bad"), None);
}

#[test]
fn saving_detects_hand_edits_and_can_be_forced() {
    let dir = tmpdir("save");
    let p = dir.join("rules.json");
    fs::write(&p, SAMPLE).unwrap();
    let mut d = Draft::from_raw(SAMPLE).unwrap();
    d.add_rule();
    // nobody touched the file: saved, and the previous contents are kept
    assert_eq!(save(&p, &d.to_text(), d.base_raw(), false), SaveOutcome::Saved);
    assert_eq!(fs::read_to_string(&p).unwrap(), d.to_text());
    assert_eq!(fs::read_to_string(dir.join("rules.json.bak")).unwrap(), SAMPLE);
    // someone edits the file by hand; saving from the OLD base must not clobber it
    fs::write(&p, "{\"rules\": []}\n").unwrap();
    d.add_rule();
    match save(&p, &d.to_text(), SAMPLE, false) {
        SaveOutcome::Conflict { disk } => assert_eq!(disk, "{\"rules\": []}\n"),
        other => panic!("expected a conflict, got {other:?}"),
    }
    assert_eq!(fs::read_to_string(&p).unwrap(), "{\"rules\": []}\n", "a conflict writes nothing");
    // forcing overwrites, and keeps the hand-edited version as the backup
    assert_eq!(save(&p, &d.to_text(), SAMPLE, true), SaveOutcome::Saved);
    assert_eq!(fs::read_to_string(dir.join("rules.json.bak")).unwrap(), "{\"rules\": []}\n");
}

#[test]
fn saving_through_a_symlink_keeps_the_link() {
    let dir = tmpdir("link");
    let real = dir.join("config-rules.json");
    let link = dir.join("payload-rules.json");
    fs::write(&real, SAMPLE).unwrap();
    symlink(&real, &link).unwrap();
    let mut d = Draft::from_raw(SAMPLE).unwrap();
    d.add_rule();
    assert_eq!(save(&link, &d.to_text(), SAMPLE, false), SaveOutcome::Saved);
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "the symlink must survive");
    assert_eq!(fs::read_to_string(&real).unwrap(), d.to_text(), "the real file is what changed");
}

#[test]
fn saving_a_file_that_does_not_exist_yet_works() {
    let dir = tmpdir("new");
    let p = dir.join("sub/rules.json");
    assert_eq!(save(&p, "{}\n", "", false), SaveOutcome::Saved);
    assert_eq!(fs::read_to_string(&p).unwrap(), "{}\n");
}

#[test]
fn the_simulator_input_builds_the_daemons_request() {
    let mut i = SimInput::default();
    assert!(i.set("appName", "Brave Origin") && i.set("origin", "example-shop.com") && i.set("urgency", "critical"));
    assert!(i.set("timeout", "never") && i.set("ignoreState", "false"));
    assert!(!i.set("urgency", "loud") && !i.set("nonsense", "x") && !i.set("timeout", "soon"), "bad fields and values are refused");
    let draft = json!({"rules": []});
    let r = i.to_request(&draft);
    assert_eq!(r["notification"]["appName"], "Brave Origin");
    assert_eq!(r["notification"]["urgency"], "critical");
    assert_eq!(r["notification"]["expireTimeout"], 0);
    assert_eq!(r["ignoreState"], false);
    assert_eq!(r["config"], draft, "the draft is what gets evaluated");
    assert_eq!(SimInput::default().to_request(&draft)["notification"]["expireTimeout"], -1);
}

#[test]
fn the_path_can_be_overridden_for_tests() {
    // not set in the normal environment of this test run; only check the default shape
    if std::env::var_os("NOTIFD_RULES").is_none() {
        assert!(rules_path().ends_with("notifd/rules.json"));
    }
}

#[test]
fn rule_summaries_read_as_words_when_the_patterns_are_plain_lists() {
    use super::rule_view::*;
    let r = serde_json::json!({ "match": { "origin": "\\b(shop|chat)\\b", "summary": "invoice" }, "zone": "business" });
    assert_eq!(rule_view(0, &r).summary, "website is shop or chat and title has “invoice”  →  business");
    let r = serde_json::json!({ "match": { "appName": "^(kernel|linux.*)$" }, "timeout": 0 });
    assert_eq!(rule_view(0, &r).summary, "appName ~ ^(kernel|linux.*)$  →  stays until dismissed");
    let r = serde_json::json!({ "match": { "urgency": "critical" } });
    assert_eq!(rule_view(0, &r).summary, "urgency is critical  (no effect yet)");
}
