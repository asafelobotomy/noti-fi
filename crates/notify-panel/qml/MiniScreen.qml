import QtQuick
import org.kde.kirigami as Kirigami

// A tiny screen with a marker where a zone's popups appear (one of nine places, or a free spot).
Rectangle {
    id: root
    property string anchorName: "top-right"
    property real fx: 0.5            // free placement, 0..1
    property real fy: 0.1
    property string accent: ""
    property bool dim: false          // no zone chosen: nothing to point at
    implicitWidth: Kirigami.Units.gridUnit * 6
    implicitHeight: implicitWidth * 9 / 16
    radius: 3
    color: Kirigami.Theme.alternateBackgroundColor
    border.width: 1
    border.color: Kirigami.Theme.disabledTextColor

    readonly property var ids: ["top-left", "top-center", "top-right", "middle-left", "center", "middle-right", "bottom-left", "bottom-center", "bottom-right"]
    readonly property int cell: ids.indexOf(root.anchorName)
    readonly property real mw: width * 0.3
    readonly property real mh: Math.max(5, height * 0.2)
    Rectangle {
        visible: !root.dim
        width: root.mw; height: root.mh; radius: 2
        color: root.accent !== "" ? root.accent : Kirigami.Theme.highlightColor
        x: root.cell < 0 ? root.fx * (root.width - width) : [3, (root.width - width) / 2, root.width - width - 3][root.cell % 3]
        y: root.cell < 0 ? (root.fy <= 0.5 ? root.fy * root.height : root.fy * root.height - height) : [3, (root.height - height) / 2, root.height - height - 3][Math.floor(root.cell / 3)]
    }
}
