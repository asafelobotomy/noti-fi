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
