// The "Your rules" part of the page: the rules' names (read only) and a button that opens noti-fi's own app.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.uq.settings

ColumnLayout {
    id: section

    property var names: []
    property string highlight
    property color mutedColor: "gray"
    /// The heading row, for scrolling to it.
    readonly property alias row: heading
    signal openRequested

    spacing: 8

    SettingRow {
        id: heading
        settingId: "rules"
        label: "Your rules"
        highlight: section.highlight
        Layout.fillWidth: true
    }
    Repeater {
        model: section.names

        UqLabel {
            required property string modelData
            text: modelData
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
    }
    UqLabel {
        text: "Change your rules in noti-fi's own app."
        color: section.mutedColor
        wrapMode: Text.Wrap
        Layout.fillWidth: true
    }
    QQC2.Button {
        text: "Open noti-fi"
        onClicked: section.openRequested()
    }
}
