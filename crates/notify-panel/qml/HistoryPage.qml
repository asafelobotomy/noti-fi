import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    id: page
    title: "History"
    property var model
    readonly property bool online: model !== undefined && model !== null && model.connected === true
    property var ui
    readonly property bool showText: ui ? ui.showText : false
    readonly property bool missedOnly: ui ? ui.missedOnly : false

    // filtering happens in Rust; this page only displays the result
    // what the user actually sees (read by the headless tests)
    readonly property var uiState: ({ page: "history", rows: list.count, summary: countLabel.text })

    readonly property var items: {
        try { return JSON.parse(page.model ? page.model.historyJson : "[]") } catch (e) { return [] }
    }
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
                placeholderText: "Filter by app, site, title or zone (Ctrl+F)"
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
        reuseItems: true
        delegate: QQC2.ItemDelegate {
            required property var modelData
            width: ListView.view.width
            contentItem: ColumnLayout {
                spacing: 2
                RowLayout {
                    Layout.fillWidth: true
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
                    }
                    QQC2.Label { opacity: 0.7; text: modelData.zone }
                    QQC2.Label { opacity: 0.7; text: Qt.formatTime(new Date(modelData.t), "hh:mm:ss") }
                }
                QQC2.Label {
                    visible: page.showText && modelData.body !== ""
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    opacity: 0.8
                    text: modelData.body
                    textFormat: Text.PlainText
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
