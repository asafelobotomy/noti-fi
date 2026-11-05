import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Where a notification goes and how it looks, in one place: Apps (simple per-app controls), Zones (the places on screen) and
// Rules (the power tool), chosen from a side panel. All three edit the same draft, so one Save covers them and a tab switch never loses an edit.
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

    // a side panel to switch between the three sections; icons only when the window is narrow
    RowLayout {
        anchors.fill: parent
        spacing: 0
        Rectangle {
            id: side
            Layout.fillHeight: true
            readonly property bool wide: page.width > Kirigami.Units.gridUnit * 34
            Layout.preferredWidth: wide ? Kirigami.Units.gridUnit * 9 : Kirigami.Units.gridUnit * 3
            color: Kirigami.Theme.alternateBackgroundColor
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.smallSpacing
                spacing: Kirigami.Units.smallSpacing
                Repeater {
                    model: [
                        { id: "apps", label: "Apps", icon: "preferences-desktop-notification-bell", tip: "Where each app's popups go, and muting" },
                        { id: "zones", label: "Zones", icon: "view-grid", tip: "The places on screen popups appear" },
                        { id: "rules", label: "Rules", icon: "view-filter", tip: "Match notifications and decide what happens" }
                    ]
                    delegate: QQC2.ItemDelegate {
                        required property var modelData
                        Layout.fillWidth: true
                        highlighted: page.tab === modelData.id
                        icon.name: modelData.icon
                        text: side.wide ? modelData.label : ""
                        display: side.wide ? QQC2.AbstractButton.TextBesideIcon : QQC2.AbstractButton.IconOnly
                        onClicked: page.open(modelData.id)
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.text: modelData.tip
                        Accessible.name: modelData.label + " section"
                    }
                }
                Item { Layout.fillHeight: true }
            }
        }
        Kirigami.Separator { Layout.fillHeight: true }
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
