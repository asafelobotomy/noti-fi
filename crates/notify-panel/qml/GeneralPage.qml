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

    readonly property var uiState: ({
        page: "general",
        trayAvailable: page.trayAvailable,
        closeToTray: page.ui ? page.ui.closeToTray : false,
        minimizeToTray: page.ui ? page.ui.minimizeToTray : false,
        autostart: page.model ? page.model.autostart : false
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
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            opacity: 0.7
            font.pixelSize: Kirigami.Theme.smallFont.pixelSize
            text: "On Plasma with Wayland the window manager does not tell a window when it is minimised, so “minimise to tray” only takes effect on desktops that do (for example X11). “Close to tray” works everywhere. Use Quit in the tray menu to really exit."
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
