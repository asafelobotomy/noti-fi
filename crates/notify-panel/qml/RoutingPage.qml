import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Where a notification goes and how it looks, in one place: Apps (simple per-app controls), Zones (the places on screen) and
// Rules (the power tool). All three edit the same draft, so one Save covers them and a tab switch never loses an edit.
Kirigami.Page {
    id: page
    title: "Routing"
    padding: 0
    property var model
    property var ui
    property string tab: "apps"           // apps | zones | rules
    signal tabSelected(string name)

    readonly property var tabs: ["apps", "zones", "rules"]
    readonly property var current: stack.children[Math.max(0, page.tabs.indexOf(page.tab))]
    // what the user sees is what the open tab reports (read by the headless tests)
    readonly property var uiState: page.current && page.current.uiState ? page.current.uiState : ({ page: page.tab })
    // Save, Revert and the tab's own actions are the open tab's, so there is one toolbar
    actions: page.current ? page.current.actions : []

    function open(name) {
        if (page.tabs.indexOf(name) < 0) return
        page.tab = name
        page.tabSelected(name)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        QQC2.TabBar {
            id: bar
            Layout.fillWidth: true
            currentIndex: Math.max(0, page.tabs.indexOf(page.tab))
            onCurrentIndexChanged: if (page.tabs[currentIndex] !== page.tab) page.open(page.tabs[currentIndex])
            QQC2.TabButton { text: "Apps"; icon.name: "preferences-desktop-notification-bell"; Accessible.name: "Apps tab" }
            QQC2.TabButton { text: "Zones"; icon.name: "view-grid"; Accessible.name: "Zones tab" }
            QQC2.TabButton { text: "Rules"; icon.name: "view-filter"; Accessible.name: "Rules tab" }
        }
        StackLayout {
            id: stack
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: Math.max(0, page.tabs.indexOf(page.tab))
            AppsPage { model: page.model; ui: page.ui; onOpenTab: name => page.open(name) }
            ZonesPage { model: page.model; ui: page.ui; onOpenTab: name => page.open(name) }
            RulesPage { model: page.model; ui: page.ui; onOpenTab: name => page.open(name) }
        }
    }
}
