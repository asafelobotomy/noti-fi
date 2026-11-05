import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// One app's main line: icon, name and activity, where its popups go, mute, and (in the lists) the arrow that opens its details.
RowLayout {
    id: root
    property var host
    property var app
    property bool compact: false
    property bool showArrow: true
    property bool detailsOpen: false
    signal toggleDetails()
    spacing: Kirigami.Units.smallSpacing

    Kirigami.Icon {
        source: root.app.iconName !== "" ? root.app.iconName : "application-x-executable"
        fallback: "application-x-executable"
        Layout.preferredWidth: root.compact ? Kirigami.Units.iconSizes.smallMedium : Kirigami.Units.iconSizes.medium
        Layout.preferredHeight: Layout.preferredWidth
    }
    ColumnLayout {
        Layout.fillWidth: true
        spacing: 0
        QQC2.Label { Layout.fillWidth: true; font.bold: true; elide: Text.ElideRight; text: root.app.name; textFormat: Text.PlainText }
        QQC2.Label {
            visible: !root.compact
            Layout.fillWidth: true
            elide: Text.ElideRight
            opacity: 0.7
            textFormat: Text.PlainText
            text: root.app.sent
                ? root.app.count + (root.app.count === 1 ? " notification" : " notifications") + " · " + root.host.ago(root.app.last)
                : "Never sent" + (root.app.reasons !== "" ? " · " + root.app.reasons : "")
        }
    }
    QQC2.ComboBox {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 8
        model: root.host.zoneChoices
        displayText: root.app.zone === "" ? "Zone: by rules" : "Zone: " + root.app.zone
        currentIndex: root.app.zone === "" ? 0 : Math.max(0, root.host.zoneChoices.indexOf(root.app.zone))
        onActivated: index => root.host.model.appSet(root.app.matchField, root.app.matchValue, root.app.name, "zone", index === 0 ? "" : root.host.zoneChoices[index])
        Accessible.name: "Zone for " + root.app.name
    }
    QQC2.Switch {
        text: root.compact ? "" : "Mute"
        checked: root.app.muted
        onToggled: {
            root.host.model.appSet(root.app.matchField, root.app.matchValue, root.app.name, "muted", checked ? "true" : "false")
            checked = Qt.binding(() => root.app.muted)
        }
        QQC2.ToolTip.visible: root.compact && hovered
        QQC2.ToolTip.text: "Mute"
        Accessible.name: "Mute " + root.app.name
    }
    QQC2.ToolButton {
        visible: root.showArrow
        icon.name: root.detailsOpen ? "go-up" : "go-down"
        display: QQC2.AbstractButton.IconOnly
        onClicked: root.toggleDetails()
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.text: root.detailsOpen ? "Hide details" : "Timeout, look and notification types (" + root.app.kindCount + ")"
        Accessible.name: "Details for " + root.app.name
    }
}
