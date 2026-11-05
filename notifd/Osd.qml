pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland

// The small confirmation pill ("Do Not Disturb on", config errors).
PanelWindow {
    id: osd
    required property var root          // the daemon (shell.qml): configuration and the pill's text
    screen: Quickshell.screens.find(s => s.name === root.cfg.screen) ?? Quickshell.screens[0]
    anchors { top: true }                       // no left/right anchor: centred horizontally
    margins { top: 48 }
    implicitWidth: Math.max(300, pill.width + 8)
    implicitHeight: 52
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "notifd-osd"
    color: "transparent"
    visible: root.osdShown || pill.opacity > 0
    mask: Region {}                             // click-through

    Rectangle {
        id: pill
        anchors.centerIn: parent
        width: pillText.implicitWidth + 56
        height: 44
        radius: 22
        color: root.cfg.theme.card
        border.width: 1
        border.color: root.cfg.theme.border
        opacity: root.osdShown ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 160 } }

        Rectangle {
            width: 10; height: 10; radius: 5
            anchors { left: parent.left; leftMargin: 20; verticalCenter: parent.verticalCenter }
            color: root.osdColor
        }
        Text {
            id: pillText
            anchors { left: parent.left; leftMargin: 40; verticalCenter: parent.verticalCenter }
            text: root.osdText
            textFormat: Text.PlainText
            color: root.cfg.theme.text
            font.pixelSize: 14
            font.weight: Font.DemiBold
        }
    }
}
