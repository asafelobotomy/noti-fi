import QtQuick

// A small text button in the notification center's header.
Rectangle {
    id: btn
    required property var theme
    property string text: ""
    signal clicked()

    implicitWidth: label.implicitWidth + 20
    implicitHeight: 26
    anchors.verticalCenter: parent ? parent.verticalCenter : undefined
    radius: 6
    opacity: enabled ? 1 : 0.45
    color: hover.hovered && enabled ? (theme.buttonHover ?? "#323646") : (theme.button ?? "#272a36")
    Accessible.role: Accessible.Button
    Accessible.name: text

    HoverHandler { id: hover }
    TapHandler { onTapped: btn.clicked() }   // an item that is not `enabled` gets no taps
    Text {
        id: label
        anchors.centerIn: parent
        text: btn.text
        color: btn.theme.text ?? "#f2f3f7"
        font.family: btn.theme.fontFamily ?? ""
        font.pixelSize: Math.max(6, (btn.theme.fontSize ?? 12) - 1)
    }
}
