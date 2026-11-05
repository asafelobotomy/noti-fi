import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// The ordered list of rules with its template menu and the up / down / duplicate / delete buttons.
ColumnLayout {
    id: root
    property var page
    property var rules
    property int columns: 2
    Layout.fillWidth: true
    Layout.fillHeight: true
    Layout.preferredWidth: 1
    Layout.minimumHeight: Kirigami.Units.gridUnit * 10
    Layout.preferredHeight: root.columns === 1 ? Kirigami.Units.gridUnit * 16 : -1
    spacing: 0

    QQC2.Label {
        Layout.margins: Kirigami.Units.smallSpacing
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        opacity: 0.7
        text: "Checked top to bottom. For the zone, timeout and silent setting, the first matching rule that sets it wins."
    }
    RowLayout {
        Layout.margins: Kirigami.Units.smallSpacing
        Layout.topMargin: 0
        QQC2.Button {
            text: "New from template"
            icon.name: "document-new-from-template"
            onClicked: templateMenu.popup()
            Accessible.name: "New rule from a template"
            QQC2.Menu {
                id: templateMenu
                Repeater {
                    model: root.page.templates
                    delegate: QQC2.MenuItem {
                        required property var modelData
                        text: modelData.name
                        onTriggered: page.addFromTemplate(modelData)
                    }
                }
            }
        }
        Item { Layout.fillWidth: true }
    }
    Kirigami.Separator { Layout.fillWidth: true }

    ListView {
        id: list
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        model: root.rules
        currentIndex: page.selected
        delegate: QQC2.ItemDelegate {
            id: row
            required property int index
            required property string title
            required property string summary
            required property bool matched
            required property bool decidesZone
            required property bool decidesTimeout
            required property bool decidesSilent
            required property bool hasError
            width: ListView.view.width
            highlighted: ListView.isCurrentItem
            onClicked: page.selected = index
            contentItem: RowLayout {
                spacing: Kirigami.Units.smallSpacing
                QQC2.Label { text: (row.index + 1) + "."; opacity: 0.6; Layout.alignment: Qt.AlignTop }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0
                    QQC2.Label { Layout.fillWidth: true; elide: Text.ElideRight; font.bold: true; text: row.title; textFormat: Text.PlainText }
                    QQC2.Label { Layout.fillWidth: true; elide: Text.ElideRight; opacity: 0.7; text: row.summary; textFormat: Text.PlainText }
                }
                Kirigami.Icon {
                    visible: row.hasError
                    source: "dialog-error"
                    Layout.preferredWidth: Kirigami.Units.iconSizes.small
                    Layout.preferredHeight: Kirigami.Units.iconSizes.small
                }
                QQC2.Label {
                    visible: row.matched
                    color: Kirigami.Theme.positiveTextColor
                    font: Kirigami.Theme.smallFont
                    text: {
                        const d = []
                        if (row.decidesZone) d.push("zone")
                        if (row.decidesTimeout) d.push("timeout")
                        if (row.decidesSilent) d.push("silent")
                        return d.length ? "matches · sets " + d.join(", ") : "matches"
                    }
                }
            }
        }
        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: root.rules.count === 0
            icon.name: "view-list-details"
            text: page.online ? "No rules yet" : "notifd is not running"
            explanation: page.online ? "Without rules, every notification goes to the default zone." : ""
        }
    }

    Kirigami.Separator { Layout.fillWidth: true }
    RowLayout {
        Layout.margins: Kirigami.Units.smallSpacing
        QQC2.ToolButton { icon.name: "go-up"; text: "Up"; enabled: page.selected > 0; onClicked: page.selected = page.model.ruleMove(page.selected, -1) }
        QQC2.ToolButton { icon.name: "go-down"; text: "Down"; enabled: page.selected >= 0 && page.selected < root.rules.count - 1; onClicked: page.selected = page.model.ruleMove(page.selected, 1) }
        QQC2.ToolButton { icon.name: "edit-copy"; text: "Duplicate"; enabled: page.selected >= 0; onClicked: page.selected = page.model.ruleDuplicate(page.selected) }
        Item { Layout.fillWidth: true }
        QQC2.ToolButton { icon.name: "edit-delete"; text: "Delete"; enabled: page.selected >= 0; onClicked: page.model.ruleDelete(page.selected) }
    }
}
