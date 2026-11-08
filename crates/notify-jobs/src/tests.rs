use std::collections::HashMap;

use crate::apps::name_and_icon;
use crate::model::*;

fn props(p: &[(&str, Val)]) -> HashMap<String, Val> {
    p.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

fn copying() -> Job {
    let mut j = Job::new("org.kde.dolphin", CAP_KILLABLE | CAP_SUSPENDABLE, &props(&[("application-display-name", Val::S("Dolphin".into()))]));
    j.update(&props(&[
        ("title", Val::S("Copying".into())),
        ("totalFiles", Val::U(10)),
        ("processedFiles", Val::U(3)),
        ("totalBytes", Val::U(500 * 1024 * 1024)),
        ("processedBytes", Val::U(100 * 1024 * 1024)),
        ("speed", Val::U(20 * 1024 * 1024)),
        ("destUrl", Val::S("file:///data/My%20Stuff".into())),
    ]));
    j
}

#[test]
fn running_job_shows_progress_speed_and_buttons() {
    let n = copying().render().unwrap();
    assert_eq!(n.summary, "Copying");
    assert_eq!(n.percent, Some(20));
    assert_eq!(n.category, "transfer");
    assert!(n.resident && n.expire_ms == 0);
    let first = n.body.lines().next().unwrap();
    assert_eq!(first, "3 of 10 files · 100 MiB of 500 MiB · 20.0 MiB/s · 20 s left");
    assert!(n.body.contains("To /data/My Stuff"), "{}", n.body);
    let keys: Vec<_> = n.actions.iter().map(|a| a.0).collect();
    assert_eq!(keys, ["suspend", "cancel"]);
}

#[test]
fn the_speed_text_is_what_the_transfer_graph_parses() {
    // notifd's Transfers.qml reads "<number> <K|M|G|T>[i]B/s" from the text
    let body = copying().render().unwrap().body;
    assert!(body.contains("20.0 MiB/s"));
}

#[test]
fn explicit_percent_wins_and_paused_jobs_offer_resume() {
    let mut j = copying();
    j.update(&props(&[("percent", Val::U(42)), ("suspended", Val::B(true))]));
    let n = j.render().unwrap();
    assert_eq!(n.percent, Some(42));
    assert_eq!(n.summary, "Copying (paused)");
    assert!(n.body.contains("paused") && !n.body.contains("/s"));
    assert_eq!(n.actions[0].0, "resume");
}

#[test]
fn finished_job_says_what_was_done_and_can_open_the_folder() {
    let mut j = copying();
    j.terminate(0, "");
    let n = j.render().unwrap();
    assert_eq!(n.summary, "Copied");
    assert_eq!(n.category, "transfer.complete");
    assert_eq!(n.body.lines().next().unwrap(), "10 files, 500 MiB");
    assert!(!n.resident && n.expire_ms == -1 && n.percent.is_none());
    assert_eq!(n.actions.iter().map(|a| a.0).collect::<Vec<_>>(), ["default", "open"]);
    assert_eq!(j.dest_path().unwrap().to_str(), Some("/data/My Stuff"));
}

#[test]
fn cancelled_and_quiet_jobs_leave_no_notification() {
    let mut j = copying();
    j.terminate(ERR_USER_CANCELED, "Cancelled");
    assert!(j.render().is_none());
    let mut t = Job::new("org.kde.dolphin", 0, &props(&[("transient", Val::B(true))]));
    t.terminate(0, "");
    assert!(t.render().is_none());
    // a transient job that fails is still reported
    let mut f = Job::new("org.kde.dolphin", 0, &props(&[("transient", Val::B(true))]));
    f.terminate(13, "Access denied");
    assert!(f.render().is_some());
}

#[test]
fn failed_job_shows_the_error_escaped() {
    let mut j = copying();
    j.terminate(13, "Could not write to <b>/root</b> & co");
    let n = j.render().unwrap();
    assert_eq!(n.summary, "Copying failed");
    assert_eq!(n.category, "transfer.error");
    assert!(n.body.starts_with("Could not write to &lt;b&gt;/root&lt;/b&gt; &amp; co"));
    assert!(n.actions.is_empty());
}

#[test]
fn v2_clients_set_fields_one_by_one_and_the_error_before_terminate() {
    let mut j = Job::new("", CAP_KILLABLE, &HashMap::new());
    j.set_info_message("Moving");
    j.set_amount(false, 4, "files");
    j.set_amount(true, 1, "files");
    j.set_description(0, "Source", "/home/me/a.txt");
    j.set_description(1, "Destination", "sftp://me:secret@host/srv/a.txt");
    let n = j.render().unwrap();
    assert_eq!(n.summary, "Moving");
    assert!(n.body.contains("1 of 4 files"));
    assert!(n.body.contains("Destination: sftp://host/srv/a.txt"), "{}", n.body);
    assert!(!n.body.contains("secret"));
    j.pending_error = ERR_USER_CANCELED;
    j.terminate(j.pending_error, "");
    assert!(j.render().is_none());
}

#[test]
fn single_file_without_dest_url_opens_the_destination_folder() {
    let mut j = Job::new("", 0, &HashMap::new());
    j.set_amount(false, 1, "files");
    j.set_description(1, "Destination", "/data/in/report.pdf");
    assert_eq!(j.dest_path().unwrap().to_str(), Some("/data/in"));
}

#[test]
fn heading_falls_back_to_the_app_then_a_generic_name() {
    let j = Job::new("", 0, &props(&[("application-display-name", Val::S("Ark".into()))]));
    assert_eq!(j.render().unwrap().summary, "Ark");
    assert_eq!(Job::new("", 0, &HashMap::new()).render().unwrap().summary, "File operation");
}

#[test]
fn finished_headings() {
    assert_eq!(finished("Copying"), "Copied");
    assert_eq!(finished("Moving"), "Moved");
    assert_eq!(finished("Extracting archive"), "Extracted archive");
    assert_eq!(finished("Sharpening"), "Sharpening finished");
}

#[test]
fn sizes_durations_and_paths() {
    assert_eq!(bytes(512), "512 B");
    assert_eq!(bytes(1536), "1.5 KiB");
    assert_eq!(bytes(300 * 1024 * 1024), "300 MiB");
    assert_eq!(duration(5), "5 s");
    assert_eq!(duration(61), "2 min");
    assert_eq!(duration(3900), "1 h 5 min");
    assert_eq!(local_path("file:///a/b%C3%A9/").unwrap().to_str(), Some("/a/bé"));
    assert_eq!(local_path("file:///a/b/.").unwrap().to_str(), Some("/a/b"));
    assert!(local_path("smb://host/x").is_none());
    assert!(local_path("Hard Drive").is_none());
    assert_eq!(pretty_url("smb://user@nas/share/%20x"), "smb://nas/share/ x");
}

#[test]
fn unknown_and_mistyped_properties_are_ignored() {
    let mut j = copying();
    let before = j.render();
    j.update(&props(&[("percent", Val::S("lots".into())), ("whatever", Val::U(1)), ("totalWidgets", Val::U(9))]));
    assert_eq!(j.percent, None);
    assert_eq!(j.render(), before);
}

#[test]
fn desktop_entry_name_and_icon_come_from_the_main_group() {
    let text = "[Desktop Entry]\nName=Dolphin\nName[de]=Dolphin DE\nIcon=system-file-manager\n[Desktop Action new]\nName=New Window\nIcon=x\n";
    assert_eq!(name_and_icon(text), ("Dolphin".into(), "system-file-manager".into()));
}
