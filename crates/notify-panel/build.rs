use cxx_qt_build::{CxxQtBuilder, QmlModule};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;

/// By default Qt compiles the QML ahead of time into C++ that calls Qt's *private* interfaces, which exist only in the Qt version that
/// built it: such a panel fails to start (`undefined symbol ...AOTCompiledContext...`) on any other Qt minor version. `cxx-qt-build` always runs
/// `qmlcachegen` and has no switch for this, so this build script points it at a stand-in tool that writes empty output. The QML is then loaded from
/// the binary's resources and compiled when the panel starts (a little slower to open, but the binary runs on any Qt 6 that is not older than the
/// one it was built with). Set `NOTIFI_QML_AOT=1` to keep Qt's ahead-of-time compilation.
fn portable_qml(out: &Path) {
    let qmake = std::env::var("QMAKE").unwrap_or_else(|_| ["qmake6", "qmake-qt6", "qmake"].iter().find(|q| Command::new(q).arg("-v").output().is_ok_and(|o| o.status.success())).expect("qmake6 not found").to_string());
    let query = |var: &str| -> String { String::from_utf8_lossy(&Command::new(&qmake).args(["-query", var]).output().expect("run qmake").stdout).trim().to_string() };
    let real_libexec = PathBuf::from(query("QT_HOST_LIBEXECS"));
    let shim = out.join("qt-shim");
    let libexec = shim.join("libexec");
    let _ = std::fs::remove_dir_all(&shim);
    std::fs::create_dir_all(&libexec).unwrap();
    // every real tool, except the cache generator
    for e in std::fs::read_dir(&real_libexec).expect("read Qt libexec").flatten() {
        if e.file_name() != "qmlcachegen" {
            let _ = symlink(e.path(), libexec.join(e.file_name()));
        }
    }
    let write_script = |path: &Path, text: &str| {
        std::fs::write(path, text).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    };
    write_script(
        &libexec.join("qmlcachegen"),
        "#!/bin/sh
# stand-in for qmlcachegen: writes the output file it was asked for. The loader unit is asked for a resource name, and the module's
# plugin code refers to qInitResources_<name>, so that one function is defined (it has nothing to load).
out=; name=
while [ $# -gt 0 ]; do
    case \"$1\" in -o) out=$2 ;; --resource-name) name=$2 ;; esac
    shift
done
[ -n \"$out\" ] || exit 0
if [ -n \"$name\" ]; then
    printf 'int qInitResources_%s() { return 1; }\\n' \"$name\" > \"$out\"
else
    echo '// no ahead-of-time compiled QML (see build.rs)' > \"$out\"
fi
exit 0
",
    );
    let wrapper = shim.join("qmake");
    write_script(
        &wrapper,
        &format!(
            "#!/bin/sh\ncase \"$*\" in\n  \"-query QT_HOST_LIBEXECS\"*|\"-query QT_INSTALL_LIBEXECS\"*) echo '{}'; exit 0 ;;\nesac\nexec '{}' \"$@\"\n",
            libexec.display(),
            qmake
        ),
    );
    // build scripts run single-threaded, before anything reads the environment
    unsafe { std::env::set_var("QMAKE", &wrapper) };
}

fn main() {
    println!("cargo:rerun-if-env-changed=NOTIFI_QML_AOT");
    println!("cargo:rerun-if-env-changed=QMAKE");
    if std::env::var_os("NOTIFI_QML_AOT").is_none() {
        portable_qml(Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")));
    }
    CxxQtBuilder::new_qml_module(
        QmlModule::new("org.notify.panel").qml_files(["qml/Main.qml", "qml/OverviewPage.qml", "qml/HistoryPage.qml", "qml/RoutingPage.qml", "qml/MatchTags.qml", "qml/RuleData.qml", "qml/RuleList.qml", "qml/AppDetails.qml", "qml/AppHeader.qml", "qml/AppTile.qml", "qml/PopupPreview.qml", "qml/Segmented.qml", "qml/DurationChips.qml", "qml/SwatchPicker.qml", "qml/FontPicker.qml", "qml/SoundPicker.qml", "qml/LookEditor.qml", "qml/LookControls.qml", "qml/ZoneMap.qml", "qml/RulePreview.qml", "qml/HistoryTweak.qml", "qml/GroupMembers.qml", "qml/RuleThen.qml", "qml/MiniScreen.qml", "qml/RulesPage.qml", "qml/RulesSimulator.qml", "qml/ZonesPage.qml", "qml/AppearancePage.qml", "qml/DraftBanners.qml", "qml/ColorField.qml", "qml/AppsPage.qml", "qml/FocusPage.qml", "qml/GeneralPage.qml", "qml/SystemPage.qml", "qml/SelfTest.qml"]),
    )
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .files(["src/model.rs"])
    .build();
}
