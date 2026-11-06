import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami

// A colour picked from swatches: Plasma's accent first, then a fixed palette; "Custom…" opens the colour dialog. The draft keeps #rrggbb.
RowLayout {
    id: root
    property string value: ""
    property bool allowUnset: false          // show an "inherit" swatch for "" (look settings)
    property string unsetLabel: "Inherit"
    property string accessibleName: "Colour"
    signal edited(string text)
    spacing: 4

    function hex(c) { const h = v => ("0" + Math.round(v * 255).toString(16)).slice(-2); return "#" + h(c.r) + h(c.g) + h(c.b) }
    readonly property var palette: [hex(Kirigami.Theme.highlightColor), "#60a5fa", "#22c55e", "#fbbf24", "#f97316", "#ef4444", "#ec4899", "#c084fc", "#14b8a6", "#94a3b8"]
    readonly property bool custom: root.value !== "" && root.palette.indexOf(root.value.toLowerCase()) < 0

    QQC2.ToolButton {
        visible: root.allowUnset
        text: root.unsetLabel
        checkable: true
        checked: root.value === ""
        onToggled: { root.edited(""); checked = Qt.binding(() => root.value === "") }
    }
    Repeater {
        model: root.palette
        delegate: Rectangle {
            id: sw
            required property string modelData
            required property int index
            readonly property bool picked: root.value.toLowerCase() === modelData
            implicitWidth: Kirigami.Units.gridUnit * 1.4; implicitHeight: implicitWidth
            radius: width / 2
            color: modelData
            border.width: picked ? 3 : 1
            border.color: picked ? Kirigami.Theme.textColor : Qt.rgba(0, 0, 0, 0.25)
            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.edited(sw.modelData) }
            Accessible.role: Accessible.Button
            Accessible.name: root.accessibleName + " " + (index === 0 ? "Plasma accent" : modelData)
            QQC2.ToolTip.visible: hoverArea.containsMouse
            QQC2.ToolTip.text: index === 0 ? "Plasma's accent colour" : modelData
            HoverHandler { id: hoverArea; property bool containsMouse: hovered }
        }
    }
    QQC2.ToolButton {
        icon.name: "color-picker"
        text: root.custom ? root.value : "Custom…"
        display: root.custom ? QQC2.AbstractButton.TextBesideIcon : QQC2.AbstractButton.IconOnly
        onClicked: dialog.open()
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.text: "Any colour"
        Accessible.name: root.accessibleName + ": any colour"
        Rectangle { visible: root.custom; width: 10; height: 10; radius: 5; color: root.custom ? root.value : "transparent"; anchors { right: parent.right; top: parent.top; margins: 2 } }
    }
    ColorDialog {
        id: dialog
        selectedColor: root.value !== "" ? root.value : Kirigami.Theme.highlightColor
        onAccepted: root.edited(root.hex(selectedColor))
    }
}
