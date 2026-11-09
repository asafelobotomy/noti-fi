//! The version is written in two places that must agree: the workspace (`Cargo.toml`, what `notify --version` and the release tag use)
//! and the daemon (`daemonVersion` in `notifd/shell.qml`, what notifd says in its hello). A release must not ship them out of step.

use std::path::Path;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

#[test]
fn the_daemon_reports_the_version_of_the_release() {
    let shell = std::fs::read_to_string(root().join("notifd/shell.qml")).unwrap();
    let daemon = shell.lines().find_map(|l| l.split("daemonVersion: \"").nth(1)).and_then(|r| r.split('"').next()).expect("daemonVersion in shell.qml");
    assert_eq!(daemon, env!("CARGO_PKG_VERSION"), "bump `daemonVersion` in notifd/shell.qml together with the workspace version");
}

#[test]
fn the_binary_reports_the_same_version() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_notify")).arg("--version").output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains(env!("CARGO_PKG_VERSION")));
}

/// The packaging and the release notes carry the version too; a release bumps them all (AGENTS.md, "Release").
#[test]
fn the_package_metainfo_and_changelog_name_this_version() {
    let v = env!("CARGO_PKG_VERSION");
    let pkgbuild = std::fs::read_to_string(root().join("packaging/PKGBUILD")).unwrap();
    assert!(pkgbuild.lines().any(|l| l == format!("pkgver={v}")), "bump pkgver in packaging/PKGBUILD to {v}");
    let meta = std::fs::read_to_string(root().join("packaging/org.notify.Panel.metainfo.xml")).unwrap();
    assert!(meta.contains(&format!("<release version=\"{v}\"")), "add <release version=\"{v}\" ...> to packaging/org.notify.Panel.metainfo.xml");
    let changes = std::fs::read_to_string(root().join("CHANGELOG.md")).unwrap();
    assert!(changes.lines().any(|l| l == format!("## {v}")), "CHANGELOG.md needs a `## {v}` section");
}
