import QtQuick

// A small text button in the notification center (its header, and a notification's own buttons).
Rectangle {
    id: btn
    required property var theme
    property string text: ""
    signal clicked()

    implicitWidth: label.implicitWidth + 20
    implicitHeight: 26
    radius: 6
    opacity: enabled ? 1 : 0.45
    color: area.containsMouse && enabled ? (theme.buttonHover ?? "#323646") : (theme.button ?? "#272a36")
    border.width: 1
    border.color: theme.border ?? "#2c2f3a"
    Accessible.role: Accessible.Button
    Accessible.name: text

    // a MouseArea (not a TapHandler) so the click is not also taken by the notification card around it; disabled with the button
    MouseArea { id: area; anchors.fill: parent; hoverEnabled: true; onClicked: btn.clicked() }
    Text {
        id: label
        anchors.centerIn: parent
        text: btn.text
        color: btn.theme.text ?? "#f2f3f7"
        font.family: btn.theme.fontFamily ?? ""
        font.pixelSize: Math.max(6, (btn.theme.fontSize ?? 12) - 1)
    }
}
