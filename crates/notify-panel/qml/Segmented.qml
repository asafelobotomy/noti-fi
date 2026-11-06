import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// A row of choices where exactly one is picked (a segmented control). `options`: [{ value, label, tip? }]; picking emits `picked`,
// the shown choice always follows `value` (never the click), so the draft stays the one source of truth.
// It asks for room for every choice on one line, and wraps when the layout gives it less.
Item {
    id: root
    property var options: []
    property string value: ""
    property string accessibleName: ""
    signal picked(string value)
    implicitWidth: { let w = 0; for (let i = 0; i < rep.count; i++) { const it = rep.itemAt(i); if (it) w += it.implicitWidth + flow.spacing } return Math.max(0, w - flow.spacing) }
    implicitHeight: flow.implicitHeight
    Flow {
        id: flow
        width: root.width
        spacing: 2
        Repeater {
            id: rep
            model: root.options
            delegate: QQC2.Button {
                id: b
                required property var modelData
                text: modelData.label
                checkable: true
                checked: root.value === String(modelData.value)
                onToggled: { root.picked(String(modelData.value)); checked = Qt.binding(() => root.value === String(b.modelData.value)) }
                QQC2.ToolTip.visible: hovered && modelData.tip !== undefined
                QQC2.ToolTip.text: modelData.tip !== undefined ? modelData.tip : ""
                Accessible.name: (root.accessibleName !== "" ? root.accessibleName + ": " : "") + modelData.label
            }
        }
    }
}
