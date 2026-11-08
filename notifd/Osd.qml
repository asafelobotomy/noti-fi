pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland

// The on-screen display: the small confirmation pill ("Do Not Disturb on", config errors), or a level (volume, mic, brightness)
// sent over the control socket by a desktop shell. On the screen Screens.qml picks (Hyprland: the focused monitor). Click-through.
PanelWindow {
    id: osd
    required property var root          // the daemon (shell.qml): configuration, screens, and what to show
    readonly property var level: root.osdLevel          // {kind, value, max, muted} or null for the pill
    readonly property var theme: root.cfg.theme
    screen: root.screens.osd
    anchors { top: level === null; bottom: level !== null }      // no left/right anchor: centred horizontally
    margins { top: 48; bottom: 96 }
    implicitWidth: Math.max(300, (level === null ? pill.width : bar.width) + 8)
    implicitHeight: level === null ? 52 : 64
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "notifd-osd"
    color: "transparent"
    visible: screen !== null && (root.osdShown || fadeOut.running)     // stays up while it fades (not bound to the opacity: a loop)
    Timer { id: fadeOut; interval: 200 }
    Connections { target: osd.root; function onOsdShownChanged() { if (!osd.root.osdShown) fadeOut.restart() } }
    mask: Region {}                             // click-through

    Rectangle {
        id: pill
        anchors.centerIn: parent
        width: pillText.implicitWidth + 56
        height: 44
        radius: 22
        color: osd.theme.card
        border.width: 1
        border.color: osd.theme.border
        opacity: root.osdShown && osd.level === null ? 1 : 0
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
            color: osd.theme.text
            font.pixelSize: 14
            font.weight: Font.DemiBold
        }
    }

    // a level: an icon, a bar, and the percentage
    Rectangle {
        id: bar
        readonly property real frac: osd.level ? Math.max(0, Math.min(1, osd.level.value / osd.level.max)) : 0
        readonly property bool muted: osd.level ? osd.level.muted === true : false
        readonly property string icon: {
            const l = osd.level
            if (!l) return ""
            if (l.kind === "brightness") return "display-brightness"
            if (l.kind === "mic") return bar.muted ? "microphone-sensitivity-muted" : "audio-input-microphone"
            return bar.muted || bar.frac === 0 ? "audio-volume-muted" : bar.frac < 0.34 ? "audio-volume-low" : bar.frac < 0.67 ? "audio-volume-medium" : "audio-volume-high"
        }
        anchors.centerIn: parent
        width: 300
        height: 52
        radius: 26
        color: osd.theme.card
        border.width: 1
        border.color: osd.theme.border
        opacity: root.osdShown && osd.level !== null ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 160 } }

        Image {
            id: icon
            anchors { left: parent.left; leftMargin: 16; verticalCenter: parent.verticalCenter }
            width: 22; height: 22
            sourceSize: Qt.size(22, 22)
            source: bar.icon !== "" ? Quickshell.iconPath(bar.icon, true) : ""
        }
        Rectangle {          // the track
            id: track
            anchors { left: icon.right; leftMargin: 12; right: pct.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
            height: 6
            radius: 3
            color: osd.theme.button ?? "#272a36"
            Rectangle {
                width: parent.width * bar.frac
                height: parent.height
                radius: 3
                color: bar.muted ? (osd.theme.subtext ?? "#b9bccb") : root.osdColor
                Behavior on width { NumberAnimation { duration: 80 } }
            }
        }
        Text {
            id: pct
            anchors { right: parent.right; rightMargin: 18; verticalCenter: parent.verticalCenter }
            width: 40
            horizontalAlignment: Text.AlignRight
            text: bar.muted ? "off" : Math.round(bar.frac * 100) + "%"
            textFormat: Text.PlainText
            color: osd.theme.text
            font.pixelSize: 14
            font.weight: Font.DemiBold
        }
    }
}
