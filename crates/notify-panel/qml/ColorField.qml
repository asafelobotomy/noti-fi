import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// A colour as #rgb / #rrggbb / #rrggbbaa text with a live swatch. `value` is what the draft holds; edits are emitted, not applied here.
RowLayout {
    id: root
    property string value: ""
    property string hint: ""
    signal edited(string text)
    readonly property bool valid: /^#[0-9a-fA-F]{3,8}$/.test(field.text)
    spacing: Kirigami.Units.smallSpacing

    Rectangle {
        implicitWidth: Kirigami.Units.gridUnit * 2
        implicitHeight: Kirigami.Units.gridUnit * 1.4
        radius: 4
        color: root.valid ? field.text : "transparent"
        border.width: 1
        border.color: root.valid ? Kirigami.Theme.disabledTextColor : Kirigami.Theme.negativeTextColor
    }
    QQC2.TextField {
        id: field
        Layout.preferredWidth: Kirigami.Units.gridUnit * 8
        font.family: "monospace"
        placeholderText: root.hint
        maximumLength: 9
        Binding on text { when: !field.activeFocus; value: root.value }
        onTextEdited: root.edited(text)
        Accessible.name: "Colour " + root.hint
    }
    QQC2.Label {
        visible: !root.valid
        color: Kirigami.Theme.negativeTextColor
        text: "use #rrggbb"
    }
}
