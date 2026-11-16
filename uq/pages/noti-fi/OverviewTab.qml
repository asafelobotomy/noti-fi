// The Overview tab: Do not disturb, focus mode, what is on screen now, and a test notification.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.uq.settings
import "logic.js" as Logic

SettingsTab {
    id: tab

    /// The page (Page.qml): its state and its changes.
    property var host

    /// Put the focus box back on what was last read.
    function sync() {
        focusBox.currentIndex = Logic.focusIndex(host.focusModes.modes, host.status.focus);
    }

    title: "Overview"
    iconName: "view-list-details"

    SettingsCard {
        title: "Do not disturb and focus"
        iconName: "notifications-disabled"
        help: "Only important notifications show while a focus mode is on."
        Layout.fillWidth: true

        QQC2.ProgressBar {
            visible: host.loading
            indeterminate: true
            Layout.fillWidth: true
        }
        SwitchRow {
            id: dndRow
            settingId: "dnd"
            label: "Do not disturb"
            highlight: host.highlight
            value: host.status.dnd
            enabled: host.ready && host.idle
            onChosen: (isOn) => host.change("dnd", Logic.dndArgs(isOn), "Couldn't change Do not disturb", host.takeStatus)
            Layout.fillWidth: true
        }
        SettingRow {
            id: focusRow
            settingId: "focus"
            label: "Focus mode"
            highlight: host.highlight
            enabled: host.ready && host.idle
            Layout.fillWidth: true

            UqComboBox {
                id: focusBox
                objectName: "focusBox"
                model: Logic.focusTexts(host.focusModes.modes)
                onActivated: (index) => host.chooseFocus(index)
            }
        }
    }

    SettingsCard {
        title: "Right now"
        iconName: "dialog-information"
        help: "Counts only. What a notification says is never shown here."
        Layout.fillWidth: true

        SettingRow {
            settingId: "now"
            label: "On screen"
            highlight: host.highlight
            Layout.fillWidth: true
        }
        QQC2.ProgressBar {
            visible: host.loading
            indeterminate: true
            Layout.fillWidth: true
        }
        Repeater {
            model: host.ready ? Logic.nowLines(host.status) : []

            UqLabel {
                required property string modelData
                text: modelData
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
        }
        RowLayout {
            Layout.fillWidth: true

            QQC2.Button {
                text: "Send a test notification"
                enabled: host.ready && host.idle
                onClicked: host.change("test", [], "Couldn't send a test notification", () => {
                    host.notes = Logic.withNote(host.notes, "test", { message: "Sent one to each group.", detail: "", ok: true });
                }, "test")
            }
        }
        Notice {
            summary: host.noteFor("test") ? host.noteFor("test").message : ""
            detail: host.noteFor("test") ? host.noteFor("test").detail : ""
            color: host.noteFor("test") && host.noteFor("test").ok ? host.mutedColor() : host.errorColor()
            mutedColor: host.mutedColor()
            Layout.fillWidth: true
        }
    }
}
