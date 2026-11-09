import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import "Json.js" as Json

Kirigami.ScrollablePage {
    id: page
    // in a wide window the content keeps a readable width, centred, instead of stretching labels and their controls apart
    leftPadding: Math.max(Kirigami.Units.largeSpacing, (width - Kirigami.Units.gridUnit * 64) / 2)
    rightPadding: leftPadding
    title: "History"
    property var model
    readonly property bool online: model !== undefined && model !== null && model.connected === true
    property var ui
    signal openPage(string name)
    // the entry opened for changing how notifications like it are shown (one at a time), and what rules are tried against
    // the open row, as "id:time": ids start again at 1 when notifd restarts, and history can be kept across restarts
    property string expandedKey: ""
    function keyOf(e) { return e.id + ":" + e.t }
    property var recent: []
    // test/screenshot hook: --open-details opens the newest entry
    onItemsChanged: if (page.expandedKey === "" && page.items.length && Qt.application.arguments.indexOf("--open-details") >= 0) page.expandedKey = page.keyOf(page.items[0])
    onExpandedKeyChanged: if (expandedKey !== "" && page.model) page.recent = Json.parse(page.model.recentNotifications(), [])
    readonly property bool showText: ui ? ui.showText : false
    readonly property bool missedOnly: ui ? ui.missedOnly : false

    // filtering happens in Rust; this page only displays the result
    // what the user actually sees (read by the headless tests)
    readonly property var uiState: ({ page: "history", rows: list.count, summary: countLabel.text })

    readonly property var items: Json.parse(page.model ? page.model.historyJson : "[]", [])
    function applyFilter() { if (page.model) page.model.setHistoryFilter(search.text, page.missedOnly) }
    onMissedOnlyChanged: applyFilter()
    Component.onCompleted: applyFilter()
    Shortcut { sequences: ["Ctrl+F"]; onActivated: search.forceActiveFocus() }

    actions: [
        Kirigami.Action {
            text: "Show message text"
            icon.name: "view-visible"
            checkable: true
            checked: page.showText
            onTriggered: page.ui.showText = checked
        },
        Kirigami.Action {
            text: "Clear history"
            icon.name: "edit-clear-history"
            enabled: page.online
            onTriggered: page.model.clearHistory()
        }
    ]

    header: ColumnLayout {
        spacing: 0
        RowLayout {
            Layout.margins: Kirigami.Units.smallSpacing
            Kirigami.SearchField {
                id: search
                Layout.fillWidth: true
                placeholderText: "Filter by app, site, title or group (Ctrl+F)"
                onTextChanged: page.applyFilter()
            }
            QQC2.CheckBox { text: "Missed only"; checked: page.missedOnly; onToggled: page.ui.missedOnly = checked }
        }
        QQC2.Label {
            id: countLabel
            Layout.margins: Kirigami.Units.smallSpacing
            opacity: 0.7
            visible: page.online
            text: page.model ? "Showing " + page.model.historyShown + " of " + page.model.historyTotal : ""
        }
        Kirigami.Separator { Layout.fillWidth: true }
    }

    ListView {
        id: list
        model: page.items
        clip: true
        delegate: QQC2.ItemDelegate {
            id: row
            required property var modelData
            readonly property bool expanded: page.expandedKey === page.keyOf(modelData)
            width: ListView.view.width
            Accessible.name: (expanded ? "Close " : "Open ") + modelData.app + ": " + modelData.summary
            contentItem: ColumnLayout {
                spacing: 2
                RowLayout {
                    Layout.fillWidth: true
                    // only the entry's own line opens and closes it, so clicks inside the opened editor never close it
                    TapHandler { onTapped: page.expandedKey = row.expanded ? "" : page.keyOf(row.modelData) }
                    Kirigami.Icon { source: row.expanded ? "arrow-down" : "arrow-right"; Layout.preferredWidth: Kirigami.Units.iconSizes.small; Layout.preferredHeight: Layout.preferredWidth; opacity: 0.6 }
                    QQC2.Label {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        font.bold: true
                        text: modelData.app + (modelData.origin ? "  (" + modelData.origin + ")" : "") + ": " + modelData.summary
                        textFormat: Text.PlainText
                    }
                    QQC2.Label {
                        visible: !modelData.shown
                        color: Kirigami.Theme.neutralTextColor
                        text: "held: " + modelData.why
                        textFormat: Text.PlainText
                    }
                    QQC2.Label { opacity: 0.7; text: modelData.zone; textFormat: Text.PlainText }
                    QQC2.Label { opacity: 0.7; text: Qt.formatTime(new Date(modelData.t), "hh:mm:ss") }
                }
                QQC2.Label {
                    visible: (page.showText || row.expanded) && modelData.body !== ""
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    opacity: 0.8
                    text: modelData.body
                    textFormat: Text.PlainText
                }
                // opened: change how notifications like this one are shown, as a rule of their own
                Loader {
                    Layout.fillWidth: true
                    Layout.topMargin: Kirigami.Units.smallSpacing
                    active: row.expanded
                    visible: active
                    sourceComponent: Kirigami.AbstractCard {
                        contentItem: HistoryTweak {
                            entry: row.modelData
                            model: page.model
                            recent: page.recent
                            onOpenRules: page.openPage("rules")
                        }
                    }
                }
            }
        }

        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: list.count === 0
            icon.name: "view-history"
            text: page.online
                  ? (page.model.historyTotal > 0 ? "Nothing matches the filter" : "No notifications yet")
                  : "notifd is not running"
        }
    }
}
