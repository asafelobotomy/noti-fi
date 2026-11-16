// The Quiet tab: groups to mute, automatic Do not disturb, and whether urgent notifications get through.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.uq.settings
import "logic.js" as Logic

SettingsTab {
    id: tab

    /// The page (Page.qml): its state and its changes.
    property var host
    title: "Quiet"
    iconName: "audio-volume-muted"

    SettingsCard {
        title: "Groups"
        iconName: "view-group"
        help: "Mute a group to keep its notifications out of sight. They still go to the history."
        Layout.fillWidth: true

        SettingRow {
            id: groupsRow
            settingId: "groups"
            label: "Mute a group"
            highlight: host.highlight
            Layout.fillWidth: true
        }
        QQC2.ProgressBar {
            visible: host.loading
            indeterminate: true
            Layout.fillWidth: true
        }
        UqLabel {
            visible: host.ready && Logic.groupRows(host.status, host.settings).length === 0
            text: "No groups yet."
            color: host.mutedColor()
        }
        Repeater {
            model: Logic.groupRows(host.status, host.settings)

            SwitchRow {
                required property var modelData
                label: modelData.label
                value: modelData.muted
                enabled: host.ready && host.idle && modelData.changeable
                onChosen: (isOn) => host.change(Logic.muteCommand(isOn), [modelData.id], "Couldn't change " + modelData.label, host.takeStatus)
                Layout.fillWidth: true
            }
        }
    }

    SettingsCard {
        title: "Automatic Do not disturb"
        iconName: "media-playback-pause"
        help: "Popups wait while one of these is true, then show when it ends."
        Layout.fillWidth: true

        SettingRow {
            id: autoRow
            settingId: "auto-dnd"
            label: "Hold popups"
            highlight: host.highlight
            Layout.fillWidth: true
        }
        SwitchRow {
            label: "When an app is full screen"
            value: host.prefOn("autoDnd.fullscreen", true)
            enabled: host.known && host.idle
            onChosen: (isOn) => host.setPref("autoDnd.fullscreen", Logic.boolWord(isOn), "Couldn't change Do not disturb for full screen")
            Layout.fillWidth: true
        }
        SwitchRow {
            label: "When you share your screen"
            value: host.prefOn("autoDnd.screenShare", true)
            enabled: host.known && host.idle
            onChosen: (isOn) => host.setPref("autoDnd.screenShare", Logic.boolWord(isOn), "Couldn't change Do not disturb for screen sharing")
            Layout.fillWidth: true
        }
        Notice {
            summary: (host.noteFor("autoDnd.fullscreen") || host.noteFor("autoDnd.screenShare") || { message: "" }).message
            detail: (host.noteFor("autoDnd.fullscreen") || host.noteFor("autoDnd.screenShare") || { detail: "" }).detail
            color: host.errorColor()
            mutedColor: host.mutedColor()
            Layout.fillWidth: true
        }
    }

    SettingsCard {
        title: "Urgent notifications"
        iconName: "dialog-warning"
        help: "Urgent ones are the ones an app marks as critical, like a low battery."
        Layout.fillWidth: true

        SwitchRow {
            id: urgentRow
            settingId: "urgent"
            label: "Let urgent notifications through Do not disturb"
            highlight: host.highlight
            value: host.prefOn("dnd.allowCritical", true)
            enabled: host.known && host.idle
            onChosen: (isOn) => host.setPref("dnd.allowCritical", Logic.boolWord(isOn), "Couldn't change whether urgent notifications get through")
            Layout.fillWidth: true
        }
        Notice {
            summary: host.noteFor("dnd.allowCritical") ? host.noteFor("dnd.allowCritical").message : ""
            detail: host.noteFor("dnd.allowCritical") ? host.noteFor("dnd.allowCritical").detail : ""
            color: host.errorColor()
            mutedColor: host.mutedColor()
            Layout.fillWidth: true
        }
    }
}
