// A short plain sentence about something that went wrong, with the program's own words under "Advanced details".
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.uq.settings

ColumnLayout {
    id: notice

    property string summary
    property string detail
    property color color: "red"
    property color mutedColor: "gray"
    property bool open: false

    visible: notice.summary !== ""
    spacing: 4
    onSummaryChanged: notice.open = false

    UqLabel {
        text: notice.summary
        color: notice.color
        wrapMode: Text.Wrap
        Layout.fillWidth: true
    }
    QQC2.Button {
        visible: notice.detail !== ""
        text: notice.open ? "Hide details" : "Advanced details"
        onClicked: notice.open = !notice.open
    }
    UqLabel {
        visible: notice.open && notice.detail !== ""
        text: notice.detail
        color: notice.mutedColor
        wrapMode: Text.Wrap
        Layout.fillWidth: true
    }
}
