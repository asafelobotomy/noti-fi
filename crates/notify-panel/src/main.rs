//! noti-fi (notification fidelity): a native Plasma control panel for notifd.
//!
//! Command line (for tests and quick looks):
//!   --page=history   --quit-after=N   --report   --selftest

mod model;

use notify_panel_core::shell::{self, Claim};

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};

fn main() {
    // only one panel: a second launch asks the first to come forward (from the tray, the launcher, or login) and exits
    match shell::instance_socket().map(|s| shell::claim_instance(&s)) {
        Some(Ok(Claim::Other)) => return,
        Some(Ok(Claim::First(l))) => *model::INSTANCE_LISTENER.lock().unwrap() = Some(l),
        Some(Err(e)) => eprintln!("noti-fi: could not check for a running panel ({e}); starting anyway"),
        None => {}
    }
    let mut app = QGuiApplication::new();
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("notify"));
        app.as_mut().set_organization_name(&QString::from("notify"));
    }
    QGuiApplication::set_desktop_file_name(&QString::from("org.notify.Panel"));
    // follow the Plasma theme (Breeze, your accent colour and fonts)
    QQuickStyle::set_style(&QString::from("org.kde.desktop"));

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        engine.as_mut().load(&QUrl::from("qrc:/qt/qml/org/notify/panel/qml/Main.qml"));
    }
    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
