//! Interaction tests for the popup card (tap, pin, stack, click-through, drag-resize), run with Qt 6's qmltestrunner on the offscreen platform.
//! Skipped (with a note) when the runner is not installed, so a machine without qt6-declarative can still run the rest.

use std::path::Path;
use std::process::Command;

#[test]
fn popup_interactions() {
    run_qml("tst_popup.qml");
}

#[test]
fn choice_controls_and_look_styles() {
    run_qml("tst_controls.qml");
}

#[test]
fn history_rule_from_a_notification() {
    run_qml("tst_historytweak.qml");
}

#[test]
fn rule_match_tags() {
    run_qml("tst_matchtags.qml");
}

fn run_qml(file: &str) {
    let runner = ["/usr/lib/qt6/bin/qmltestrunner", "/usr/lib64/qt6/bin/qmltestrunner"].into_iter().find(|p| Path::new(p).exists());
    let Some(runner) = runner else {
        eprintln!("skipped: Qt 6 qmltestrunner not found");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(runner)
        .env("QT_QPA_PLATFORM", "offscreen")
        .arg("-input")
        .arg(root.join("tests/qml").join(file))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success() && text.contains(" 0 failed"), "{file} failed:\n{text}\n{}", String::from_utf8_lossy(&out.stderr));
}
