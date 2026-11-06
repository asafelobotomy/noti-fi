import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Where a notification goes and how it looks, in one place: Apps (simple per-app controls), Zones (the places on screen) and
// Rules (the power tool), each chosen from the main sidebar. All three edit the same draft, so one Save covers them and a tab switch never loses an edit.
Kirigami.Page {
    id: page
    title: ({ apps: "Apps", zones: "Groups", rules: "Rules" })[page.tab] || "Apps"
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

    // Apps, Zones and Rules are chosen in the main sidebar; this page keeps the three together so they share one draft and toolbar
    StackLayout {
        id: stack
        anchors.fill: parent
        currentIndex: Math.max(0, page.tabs.indexOf(page.tab))
        AppsPage { model: page.model; ui: page.ui; onOpenTab: name => page.open(name) }
        ZonesPage { model: page.model; ui: page.ui; onOpenTab: name => page.open(name) }
        RulesPage { model: page.model; ui: page.ui; onOpenTab: name => page.open(name) }
    }
}
