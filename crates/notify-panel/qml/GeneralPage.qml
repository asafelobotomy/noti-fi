import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// How the panel itself behaves: the system tray icon and starting at login.
Kirigami.Page {
    id: page
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
        anchors.fill: parent

        Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "System tray" }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            visible: !page.trayAvailable
            color: Kirigami.Theme.neutralTextColor
            text: "This desktop session has no system tray, so the options below cannot take effect: closing the window quits the panel."
        }
        QQC2.CheckBox {
            Kirigami.FormData.label: "Close:"
            text: "keep running in the system tray when the window is closed"
            enabled: page.trayAvailable
            checked: page.ui ? page.ui.closeToTray : true
            onToggled: page.ui.closeToTray = checked
            Accessible.name: "Close to tray"
        }
        QQC2.CheckBox {
            Kirigami.FormData.label: "Minimise:"
            text: "move to the system tray when the window is minimised"
            enabled: page.trayAvailable
            checked: page.ui ? page.ui.minimizeToTray : true
            onToggled: page.ui.minimizeToTray = checked
            Accessible.name: "Minimise to tray"
        }
        QQC2.Button {
            Kirigami.FormData.label: "Now:"
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
            font.pixelSize: Kirigami.Theme.smallFont.pixelSize
            text: "On Plasma with Wayland the window manager does not tell a window when it is minimised, so “minimise to tray” only takes effect on desktops that do (for example X11). “Close to tray” works everywhere. Use Quit in the tray menu to really exit."
        }

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
            font.pixelSize: Kirigami.Theme.smallFont.pixelSize
            text: "Updates come from this project's releases on GitHub. The download is checked against its published checksum before anything is replaced, then the daemon is refreshed (notifications pause for about a second). This works for a copy installed in a bin folder you can write to; a system-wide install is updated with your package tool."
        }

        Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Start at login" }
        QQC2.CheckBox {
            Kirigami.FormData.label: "Launch:"
            text: "start noti-fi when I log in (hidden in the system tray)"
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
            font.pixelSize: Kirigami.Theme.smallFont.pixelSize
            text: "This adds a standard autostart entry for your user (~/.config/autostart) that points at this program, so move or reinstall the program before turning it on if its location will change. The daemon itself already starts at login on its own."
        }
    }
}
