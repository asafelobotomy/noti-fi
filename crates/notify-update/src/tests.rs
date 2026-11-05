use super::*;
use std::collections::HashMap;
use std::process::Command;

struct Fake(HashMap<String, Vec<u8>>);
impl Fetch for Fake {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        self.0.get(url).cloned().ok_or_else(|| format!("no such address in the test: {url}"))
    }
}

fn tmp(label: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("notify-update-{label}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&d).unwrap();
    d
}

/// A release archive the way `packaging/build-tarball.sh` lays it out, with `marker` as the content of both programs.
fn archive(version: &str, marker: &str, with_panel: bool) -> Vec<u8> {
    archive_for_qt(version, marker, with_panel, Some("6.11.2"))
}

fn archive_for_qt(version: &str, marker: &str, with_panel: bool, qt: Option<&str>) -> Vec<u8> {
    archive_full(version, marker, with_panel, qt, false)
}

fn archive_full(version: &str, marker: &str, with_panel: bool, qt: Option<&str>, portable: bool) -> Vec<u8> {
    let w = tmp("arch");
    let root = w.join(format!("noti-fi-{version}-x86_64-linux"));
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("share/applications")).unwrap();
    fs::write(root.join("bin/notify"), marker).unwrap();
    if with_panel {
        fs::write(root.join("bin/notify-panel"), marker).unwrap();
    }
    fs::write(root.join("share/applications/org.notify.Panel.desktop"), "[Desktop Entry]\n").unwrap();
    if let Some(q) = qt {
        fs::write(root.join("qt-version"), format!("{q}\n")).unwrap();
    }
    if portable {
        fs::write(root.join("portable-qml"), "portable\n").unwrap();
    }
    let out = w.join("a.tar.gz");
    assert!(Command::new("tar").arg("czf").arg(&out).arg("-C").arg(&w).arg(root.file_name().unwrap()).status().unwrap().success());
    fs::read(out).unwrap()
}

fn release_json(version: &str, host: &str) -> String {
    let name = format!("noti-fi-{version}-x86_64-linux.tar.gz");
    serde_json::json!({
        "tag_name": format!("v{version}"), "html_url": format!("https://github.com/{REPO}/releases/tag/v{version}"), "body": "what changed",
        "assets": [
            {"name": name, "browser_download_url": format!("{host}{REPO}/releases/download/v{version}/{name}")},
            {"name": format!("{name}.sha256"), "browser_download_url": format!("{host}{REPO}/releases/download/v{version}/{name}.sha256")},
            {"name": "other.txt", "browser_download_url": "https://example.com/other.txt"}
        ]
    })
    .to_string()
}

#[test]
fn versions_compare_as_numbers() {
    assert!(is_newer("0.7.0", "0.7.1"));
    assert!(is_newer("0.9.2", "0.10.0"), "0.10.0 is newer than 0.9.2, not older");
    assert!(is_newer("0.7.0", "v1.0.0"));
    assert!(!is_newer("0.7.0", "0.7.0"));
    assert!(!is_newer("0.8.0", "0.7.9"));
    assert!(!is_newer("1.0.0", "1.0"), "a missing part counts as zero");
    assert!(!is_newer("0.7.0", "0.7.0-rc1"), "a pre-release suffix is ignored");
}

#[test]
fn a_release_answer_is_read_for_this_architecture() {
    let r = parse_release(&release_json("0.8.0", "https://github.com/"), "x86_64").unwrap();
    assert_eq!((r.tag.as_str(), r.version.as_str()), ("v0.8.0", "0.8.0"));
    assert!(r.archive_url.ends_with("noti-fi-0.8.0-x86_64-linux.tar.gz") && r.checksum_url.ends_with(".sha256"));
    assert_eq!(r.notes, "what changed");
    let e = parse_release(&release_json("0.8.0", "https://github.com/"), "aarch64").unwrap_err();
    assert!(e.contains("no build for aarch64"), "{e}");
    assert!(parse_release("{not json", "x86_64").is_err());
    assert!(parse_release("{}", "x86_64").unwrap_err().contains("no release tag"));
}

#[test]
fn downloads_from_anywhere_but_this_projects_releases_are_refused() {
    let e = parse_release(&release_json("0.8.0", "https://evil.example/"), "x86_64").unwrap_err();
    assert!(e.contains("outside this project's releases"), "{e}");
    let e = parse_release(&release_json("0.8.0", "http://github.com/"), "x86_64").unwrap_err();
    assert!(e.contains("outside"), "plain http is refused too: {e}");
    assert!(Curl.get("http://github.com/x").unwrap_err().contains("non-HTTPS"));
}

#[test]
fn checksums_must_match() {
    let data = b"hello";
    let line = format!("{}  a.tar.gz\n", sha256_hex(data));
    assert!(verify_checksum(data, &line, "a.tar.gz").is_ok());
    assert!(verify_checksum(b"hellx", &line, "a.tar.gz").unwrap_err().contains("mismatch"));
    assert!(verify_checksum(data, &line, "b.tar.gz").unwrap_err().contains("does not mention"));
    assert!(verify_checksum(data, &format!("{}  *a.tar.gz\n", sha256_hex(data).to_uppercase()), "a.tar.gz").is_ok(), "binary-mode marker and upper case are fine");
    assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn a_newer_release_is_downloaded_verified_and_installed_over_the_old_one() {
    let arc = archive("0.8.0", "NEW", true);
    let name = "noti-fi-0.8.0-x86_64-linux.tar.gz";
    let base = format!("https://github.com/{REPO}/releases/download/v0.8.0/");
    let mut m = HashMap::new();
    m.insert("https://api.github.com/repos/asafelobotomy/noti-fi/releases/latest".to_string(), release_json("0.8.0", "https://github.com/").into_bytes());
    m.insert(format!("{base}{name}"), arc.clone());
    m.insert(format!("{base}{name}.sha256"), format!("{}  {name}\n", sha256_hex(&arc)).into_bytes());
    let fake = Fake(m);

    let rel = latest(&fake).unwrap();
    assert!(is_newer("0.7.0", &rel.version));
    let bytes = download_verified(&fake, &rel).unwrap();

    let prefix = tmp("prefix");
    fs::create_dir_all(prefix.join("bin")).unwrap();
    fs::write(prefix.join("bin/notify"), "OLD").unwrap();
    let done = install_archive(&bytes, &rel.version, &prefix, &tmp("work"), Some("6.11.2")).unwrap();
    assert_eq!(fs::read_to_string(prefix.join("bin/notify")).unwrap(), "NEW", "the old program is replaced");
    assert_eq!(fs::read_to_string(prefix.join("bin/notify-panel")).unwrap(), "NEW");
    assert_eq!(fs::metadata(prefix.join("bin/notify")).unwrap().permissions().mode() & 0o777, 0o755);
    assert!(prefix.join("share/applications/org.notify.Panel.desktop").exists());
    assert_eq!(done.files.len(), 3);
    assert!(done.panel_skipped.is_none());
    assert!(fs::read_dir(prefix.join("bin")).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().ends_with(".new")), "no temporary files are left");
}

#[test]
fn a_tampered_download_is_refused_and_nothing_is_installed() {
    let arc = archive("0.8.0", "NEW", true);
    let name = "noti-fi-0.8.0-x86_64-linux.tar.gz";
    let base = format!("https://github.com/{REPO}/releases/download/v0.8.0/");
    let mut m = HashMap::new();
    m.insert(format!("{base}{name}"), b"not the archive".to_vec());
    m.insert(format!("{base}{name}.sha256"), format!("{}  {name}\n", sha256_hex(&arc)).into_bytes());
    let rel = parse_release(&release_json("0.8.0", "https://github.com/"), "x86_64").unwrap();
    assert!(download_verified(&Fake(m), &rel).unwrap_err().contains("mismatch"));
}

#[test]
fn an_archive_without_the_programs_is_refused_before_anything_is_replaced() {
    let prefix = tmp("prefix2");
    fs::create_dir_all(prefix.join("bin")).unwrap();
    fs::write(prefix.join("bin/notify"), "OLD").unwrap();
    let e = install_archive(&archive("0.8.0", "NEW", false), "0.8.0", &prefix, &tmp("work2"), Some("6.11.2")).unwrap_err();
    assert!(e.contains("missing bin/notify-panel"), "{e}");
    assert_eq!(fs::read_to_string(prefix.join("bin/notify")).unwrap(), "OLD", "the check happens before any file is replaced");
    let e = install_archive(&archive("0.8.0", "NEW", true), "0.9.0", &prefix, &tmp("work3"), Some("6.11.2")).unwrap_err();
    assert!(e.contains("does not contain"), "an archive of another version is refused: {e}");
    assert!(install_archive(b"garbage", "0.8.0", &prefix, &tmp("work4"), Some("6.11.2")).is_err());
}

#[test]
fn the_install_prefix_comes_from_where_the_program_lives() {
    assert_eq!(install_prefix(Path::new("/home/me/.local/bin/notify")), Some(PathBuf::from("/home/me/.local")));
    assert_eq!(install_prefix(Path::new("/usr/bin/notify-panel")), Some(PathBuf::from("/usr")));
    assert_eq!(install_prefix(Path::new("/work/target/release/notify")), None, "a build folder is not an installation");
}

#[test]
fn qt_versions_must_match_for_the_panel() {
    assert!(qt_compatible("6.11.2", "6.11.2"));
    assert!(qt_compatible("6.11.2", "6.11.5"), "a newer patch of the same minor is fine");
    assert!(!qt_compatible("6.11.2", "6.11.1"), "an older patch may lack what the panel needs");
    assert!(!qt_compatible("6.12.0", "6.11.2"), "built for a newer minor: this is the case that broke the first releases");
    assert!(!qt_compatible("6.11.2", "6.12.0"), "a newer minor can have dropped private interfaces");
    assert!(!qt_compatible("6.11.2", "5.15.0"));
    assert!(!qt_compatible("", "6.11.2"));
}

#[test]
fn a_panel_built_for_another_qt_is_left_alone_but_the_cli_is_updated() {
    let prefix = tmp("prefix3");
    fs::create_dir_all(prefix.join("bin")).unwrap();
    fs::write(prefix.join("bin/notify"), "OLD").unwrap();
    fs::write(prefix.join("bin/notify-panel"), "OLD-PANEL").unwrap();
    let r = install_archive(&archive_for_qt("0.9.0", "NEW", true, Some("6.12.0")), "0.9.0", &prefix, &tmp("work5"), Some("6.11.2")).unwrap();
    assert_eq!(fs::read_to_string(prefix.join("bin/notify")).unwrap(), "NEW", "the command line tool does not depend on Qt and is updated");
    assert_eq!(fs::read_to_string(prefix.join("bin/notify-panel")).unwrap(), "OLD-PANEL", "a panel that would not start is not installed over one that does");
    assert!(r.panel_skipped.as_deref().unwrap().contains("built for Qt 6.12.0 but this machine has Qt 6.11.2"), "{r:?}");
    // no Qt version in the archive, or none detectable here: also left alone
    let r = install_archive(&archive_for_qt("0.9.0", "NEWER", true, None), "0.9.0", &prefix, &tmp("work6"), Some("6.11.2")).unwrap();
    assert!(r.panel_skipped.is_some() && fs::read_to_string(prefix.join("bin/notify-panel")).unwrap() == "OLD-PANEL");
    let r = install_archive(&archive_for_qt("0.9.0", "NEWEST", true, Some("6.11.2")), "0.9.0", &prefix, &tmp("work7"), None).unwrap();
    assert!(r.panel_skipped.as_deref().unwrap().contains("could not be determined"));
}

#[test]
fn a_portable_panel_runs_on_any_qt_6() {
    assert!(portable_compatible("6.12.0", "6.11.2"));
    assert!(portable_compatible("6.11.2", "6.12.0"));
    assert!(!portable_compatible("6.11.2", "5.15.0"));
    let prefix = tmp("prefix4");
    fs::create_dir_all(prefix.join("bin")).unwrap();
    fs::write(prefix.join("bin/notify-panel"), "OLD-PANEL").unwrap();
    let r = install_archive(&archive_full("0.9.0", "NEW", true, Some("6.12.0"), true), "0.9.0", &prefix, &tmp("work8"), Some("6.11.2")).unwrap();
    assert!(r.panel_skipped.is_none(), "{r:?}");
    assert_eq!(fs::read_to_string(prefix.join("bin/notify-panel")).unwrap(), "NEW", "a portable panel built for Qt 6.12 is installed on Qt 6.11");
    let r = install_archive(&archive_full("0.9.0", "NEWER", true, Some("6.12.0"), false), "0.9.0", &tmp("prefix5"), &tmp("work9"), Some("6.11.2")).unwrap();
    assert!(r.panel_skipped.is_some(), "without the portable marker the strict rule still applies");
}
