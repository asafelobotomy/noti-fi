use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("org.notify.panel").qml_files(["qml/Main.qml", "qml/OverviewPage.qml", "qml/HistoryPage.qml", "qml/RulesPage.qml", "qml/ZonesPage.qml", "qml/AppearancePage.qml", "qml/DraftBanners.qml", "qml/ColorField.qml", "qml/AppsPage.qml", "qml/StyleFields.qml", "qml/FocusPage.qml", "qml/GeneralPage.qml", "qml/SelfTest.qml"]),
    )
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .files(["src/model.rs"])
    .build();
}
