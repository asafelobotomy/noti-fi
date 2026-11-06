import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// One app as a tile in the grid view: icon, name and a line of activity; click it to open its settings.
Kirigami.AbstractCard {
    id: tile
    property var host
    property var app
    signal opened()
    width: Kirigami.Units.gridUnit * 9
    showClickFeedback: true
    onClicked: tile.opened()
    contentItem: ColumnLayout {
        spacing: Kirigami.Units.smallSpacing
        Kirigami.Icon {
            Layout.alignment: Qt.AlignHCenter
            source: tile.app.iconName !== "" ? tile.app.iconName : "application-x-executable"
            fallback: "application-x-executable"
            Layout.preferredWidth: Kirigami.Units.iconSizes.large
            Layout.preferredHeight: Layout.preferredWidth
        }
        QQC2.Label { Layout.fillWidth: true; horizontalAlignment: Text.AlignHCenter; font.bold: true; elide: Text.ElideRight; text: tile.app.name; textFormat: Text.PlainText }
        QQC2.Label {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            opacity: 0.7
            font: Kirigami.Theme.smallFont
            textFormat: Text.PlainText
            text: tile.app.muted ? "Muted" : tile.app.zone !== "" ? "Zone: " + tile.host.zoneLabel(tile.app.zone) : tile.app.sent ? tile.app.count + " sent" : "Never sent"
            color: tile.app.muted ? Kirigami.Theme.neutralTextColor : Kirigami.Theme.textColor
        }
    }
    Accessible.role: Accessible.Button
    Accessible.name: "Open settings for " + tile.app.name
}
