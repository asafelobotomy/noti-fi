// The History tab: how much is kept, and the recent notifications with Clear history.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.uq.settings
import "logic.js" as Logic

SettingsTab {
    id: tab

    /// The page (Page.qml): its state and its changes.
    property var host
    readonly property alias recent: recent

    /// Put the limit box back on what was last read.
    function sync() {
        limitBox.currentIndex = Logic.limitIndex(host.settings.limit);
    }

    title: "History"
    iconName: "document-open-recent"

    SettingsCard {
        title: "What is kept"
        iconName: "folder-open"
        help: "Only the text of notifications is kept on this computer, and only if you turn on Keep after restart."
        Layout.fillWidth: true

        SettingRow {
            id: historyRow
            settingId: "history"
            label: "Remember at most"
            highlight: host.highlight
            enabled: host.known && host.idle
            Layout.fillWidth: true

            UqComboBox {
                id: limitBox
                objectName: "limitBox"
                model: Logic.limitTexts(host.settings.limit)
                onActivated: (index) => host.chooseLimit(index)
            }
        }
        SwitchRow {
            label: "Keep after restart"
            value: host.prefOn("history.persist", false)
            enabled: host.known && host.idle
            onChosen: (isOn) => host.setPref("history.persist", Logic.boolWord(isOn), "Couldn't change whether history is kept after a restart")
            Layout.fillWidth: true
        }
        UqLabel {
            text: "Turning this on saves the text of notifications to a file on this computer."
            color: host.mutedColor()
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        Notice {
            summary: (host.noteFor("history.limit") || host.noteFor("history.persist") || { message: "" }).message
            detail: (host.noteFor("history.limit") || host.noteFor("history.persist") || { detail: "" }).detail
            color: host.errorColor()
            mutedColor: host.mutedColor()
            Layout.fillWidth: true
        }
    }

    SettingsCard {
        title: "Recent notifications"
        iconName: "view-history"
        help: "Who sent each one, which group it went to and when. Titles and text are not shown."
        wide: true
        Layout.fillWidth: true
        actions: [
            QQC2.Button {
                text: "Clear history…"
                enabled: host.ready && host.idle && host.status.history > 0
                // Settings shows what would go, grouped by app, then asks "Clear the notification history? This can't be undone."
                onClicked: host.change("history-clear", [], "Couldn't clear the history", () => {
                    host.ctx.command.run("status", [], host.guard(host.showStatus));
                    recent.refresh();
                }, "history-clear")
            }
        ]

        HistorySection {
            id: recent
            host: page
            Layout.fillWidth: true
        }
    }
}
