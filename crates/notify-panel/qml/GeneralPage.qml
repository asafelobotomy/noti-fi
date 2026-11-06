import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// How the panel itself behaves: the system tray icon and starting at login.
Kirigami.ScrollablePage {
    id: page
    // in a wide window the content keeps a readable width, centred, instead of stretching labels and their controls apart
    leftPadding: Math.max(Kirigami.Units.largeSpacing, (width - Kirigami.Units.gridUnit * 56) / 2)
    rightPadding: leftPadding
    title: "General"
    property var model
    property var ui
    property bool trayAvailable: false
    signal hideRequested()

    readonly property var uiState: ({
        page: "general",
        trayAvailable: page.trayAvailable,
        closeToTray: page.ui ? page.ui.closeToTray : false,
        minimizeToTray: page.ui ? page.ui.minimizeToTray : false,
        autostart: page.model ? page.model.autostart : false,
        version: Qt.application.version,
        updateState: page.model ? page.model.updateState : "",
        autoCheck: page.ui ? page.ui.autoCheckUpdates : false
    })

    Kirigami.FormLayout {

        Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }

        Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Window and tray" }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            visible: !page.trayAvailable
            color: Kirigami.Theme.neutralTextColor
            text: "No system tray on this desktop, so closing the window quits the panel."
        }
        QQC2.CheckBox {
            Kirigami.FormData.label: "Close:"
            text: "keep running in the tray when the window is closed"
            enabled: page.trayAvailable
            checked: page.ui ? page.ui.closeToTray : true
            onToggled: page.ui.closeToTray = checked
            Accessible.name: "Close to tray"
        }
        QQC2.CheckBox {
            Kirigami.FormData.label: "Minimise:"
            text: "move to the tray when the window is minimised (not on Wayland)"
            enabled: page.trayAvailable
            checked: page.ui ? page.ui.minimizeToTray : true
            onToggled: page.ui.minimizeToTray = checked
            Accessible.name: "Minimise to tray"
        }
        QQC2.Button {
            Kirigami.FormData.label: ""
            text: "Hide to tray"
            icon.name: "go-down"
            enabled: page.trayAvailable
            onClicked: page.hideRequested()
            Accessible.name: "Hide to tray"
        }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            opacity: 0.7
            font: Kirigami.Theme.smallFont
            text: "Plasma on Wayland never tells a window it was minimised, so use Hide to tray instead. Quit from the tray menu to exit for good."
        }

        Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }

        Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Start at login" }
        QQC2.CheckBox {
            Kirigami.FormData.label: "Launch:"
            text: "start noti-fi when I log in (hidden in the tray)"
            checked: page.model ? page.model.autostart : false
            onToggled: {
                page.model.setAutostartEnabled(checked)
                checked = Qt.binding(() => page.model ? page.model.autostart : false)   // follow what was actually written
            }
            Accessible.name: "Start at login"
        }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            opacity: 0.7
            font: Kirigami.Theme.smallFont
            text: "Adds an autostart entry in ~/.config/autostart that points at this program. The daemon already starts at login by itself."
        }
        Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }
        Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Updates" }
        QQC2.Label { Kirigami.FormData.label: "Installed:"; text: "noti-fi " + Qt.application.version }
        RowLayout {
            Kirigami.FormData.label: "Latest:"
            spacing: Kirigami.Units.largeSpacing
            QQC2.Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                textFormat: Text.PlainText
                color: page.model && page.model.updateState === "error" ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                text: !page.model ? "" : page.model.updateState === "checking" ? "Checking github.com…"
                    : page.model.updateState === "installing" ? "Downloading and installing…"
                    : page.model.updateMessage !== "" ? page.model.updateMessage : "Not checked yet. Nothing is contacted until you ask."
            }
        }
        RowLayout {
            Kirigami.FormData.label: ""
            spacing: Kirigami.Units.smallSpacing
            QQC2.Button {
                text: "Check now"
                icon.name: "view-refresh"
                enabled: page.model && page.model.updateState !== "checking" && page.model.updateState !== "installing"
                onClicked: page.model.checkForUpdates()
                Accessible.name: "Check for updates"
            }
            QQC2.Button {
                visible: page.model && page.model.updateState === "available"
                text: "Install " + (page.model ? page.model.updateLatest : "")
                icon.name: "download"
                highlighted: true
                onClicked: page.model.installUpdate()
                Accessible.name: "Install the update"
            }
            QQC2.Button {
                visible: page.model && page.model.updateState === "installed"
                text: "Restart noti-fi"
                icon.name: "system-reboot"
                highlighted: true
                onClicked: page.model.restartApp()
                Accessible.name: "Restart noti-fi"
            }
            QQC2.Button {
                visible: page.model && page.model.updatePage !== "" && (page.model.updateState === "available" || page.model.updateState === "installed")
                flat: true
                text: "What's new"
                icon.name: "help-about"
                onClicked: Qt.openUrlExternally(page.model.updatePage)
            }
        }
        QQC2.CheckBox {
            Kirigami.FormData.label: "Automatically:"
            text: "check for updates once a day (contacts github.com)"
            checked: page.ui ? page.ui.autoCheckUpdates : false
            onToggled: page.ui.autoCheckUpdates = checked
            Accessible.name: "Check for updates daily"
        }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            opacity: 0.7
            font: Kirigami.Theme.smallFont
            text: "Updates come from this project's GitHub releases and are checked against their published checksum before anything is replaced. Notifications pause for about a second while the daemon restarts. System-wide installs are updated with your package tool."
        }

    }
}
