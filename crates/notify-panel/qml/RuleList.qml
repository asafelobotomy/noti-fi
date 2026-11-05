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

    // one row: add a rule (blank or from a template), and the buttons that act on the selected one
    RowLayout {
        Layout.margins: Kirigami.Units.smallSpacing
        spacing: Kirigami.Units.smallSpacing
        QQC2.Button {
            text: "Add rule"
            icon.name: "list-add"
            onClicked: addMenu.popup()
            Accessible.name: "Add a rule"
            QQC2.Menu {
                id: addMenu
                QQC2.MenuItem { text: "Blank rule"; icon.name: "document-new"; onTriggered: page.selected = page.model.ruleAdd() }
                QQC2.MenuSeparator {}
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
        Repeater {
            model: [
                { icon: "go-up", tip: "Move up (checked earlier)", act: "up" }, { icon: "go-down", tip: "Move down (checked later)", act: "down" },
                { icon: "edit-copy", tip: "Duplicate", act: "dup" }, { icon: "edit-delete", tip: "Delete", act: "del" }
            ]
            delegate: QQC2.ToolButton {
                required property var modelData
                icon.name: modelData.icon
                display: QQC2.AbstractButton.IconOnly
                enabled: page.selected >= 0 && (modelData.act !== "up" || page.selected > 0) && (modelData.act !== "down" || page.selected < root.rules.count - 1)
                onClicked: {
                    if (modelData.act === "up") page.selected = page.model.ruleMove(page.selected, -1)
                    else if (modelData.act === "down") page.selected = page.model.ruleMove(page.selected, 1)
                    else if (modelData.act === "dup") page.selected = page.model.ruleDuplicate(page.selected)
                    else page.model.ruleDelete(page.selected)
                }
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: modelData.tip
                Accessible.name: modelData.tip
            }
        }
    }
    QQC2.Label {
        Layout.leftMargin: Kirigami.Units.smallSpacing
        Layout.rightMargin: Kirigami.Units.smallSpacing
        Layout.bottomMargin: Kirigami.Units.smallSpacing
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        opacity: 0.7
        font: Kirigami.Theme.smallFont
        text: "Checked from the top; the first matching rule that sets the zone, timeout or silent wins."
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
}
