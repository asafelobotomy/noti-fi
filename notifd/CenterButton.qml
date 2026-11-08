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
    border.width: activeFocus ? 2 : 1                   // reached with Tab: Enter or Space presses it
    border.color: activeFocus ? (theme.text ?? "#f2f3f7") : (theme.border ?? "#2c2f3a")
    Keys.onReturnPressed: if (enabled) btn.clicked()
    Keys.onEnterPressed: if (enabled) btn.clicked()
    Keys.onSpacePressed: if (enabled) btn.clicked()
    Accessible.role: Accessible.Button
    Accessible.name: text

    // a MouseArea (not a TapHandler) so the click is not also taken by the notification card around it; disabled with the button
    MouseArea { id: area; anchors.fill: parent; hoverEnabled: true; onClicked: btn.clicked() }
    Text {
        id: label
        anchors.centerIn: parent
        text: btn.text
        textFormat: Text.PlainText      // app-supplied text: never rich text (it could load remote images)
        color: btn.theme.text ?? "#f2f3f7"
        font.family: btn.theme.fontFamily ?? ""
        font.pixelSize: Math.max(6, (btn.theme.fontSize ?? 12) - 1)
    }
}
