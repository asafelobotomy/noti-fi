import QtQuick

// The inline reply box of a chat notification: type and press Enter (or click Send); Escape asks to dismiss.
Rectangle {
    id: r
    property bool active: true
    property string placeholder: "Reply…"
    property color accent: "#60a5fa"
    property color cText: "#f2f3f7"
    property color cSub: "#b9bccb"
    property color cButton: "#272a36"
    property string fontFamily: ""
    property real fontSize: 12
    readonly property bool hovered: replyHover.hovered
    readonly property bool focused: replyInput.activeFocus
    signal submitted(string text)
    signal cancelled()
    function clear() { replyInput.text = "" }

    implicitHeight: 32
    radius: 6
    color: r.cButton
    border.width: replyInput.activeFocus ? 1 : 0
    border.color: r.accent
    HoverHandler { id: replyHover }

    TextInput {
        id: replyInput
        anchors { left: parent.left; right: sendLabel.left; verticalCenter: parent.verticalCenter; leftMargin: 10; rightMargin: 8 }
        color: r.cText
        font.pixelSize: r.fontSize
        font.family: r.fontFamily
        clip: true
        selectByMouse: true
        enabled: r.active
        onAccepted: r.submitted(text)
        Keys.onEscapePressed: r.cancelled()
        Text {
            visible: replyInput.text === "" && !replyInput.activeFocus
            text: r.placeholder
            textFormat: Text.PlainText
            color: r.cSub
            font.pixelSize: r.fontSize
            font.family: r.fontFamily
        }
    }
    Text {
        id: sendLabel
        anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
        text: "Send"
        color: replyInput.text.trim() !== "" ? r.accent : r.cSub
        font.pixelSize: r.fontSize
        font.family: r.fontFamily
        font.weight: Font.DemiBold
        TapHandler { enabled: r.active; onTapped: r.submitted(replyInput.text) }
    }
}
