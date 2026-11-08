import QtQuick

// One notification in the notification center: who sent it and when, its title and text, an unread dot in the group's colour,
// and its buttons while the app still holds it (`live`). A click marks it read (and does what clicking the popup would); the × forgets it.
Rectangle {
    id: item
    required property var entry          // a history entry: { id, t, app, summary, body, zone, shown, why, read }
    required property color accent
    required property var theme
    required property real now
    property var live: null              // the Notification while its app still holds it (see Kept.qml), else null
    signal opened()
    signal removed()
    signal invoked(string key)

    readonly property var buttons: live ? (live.actions || []).filter(a => a.identifier !== "default" && a.text !== "") : []

    readonly property bool unread: entry.read !== true
    readonly property string fontFamily: theme.fontFamily ?? ""
    readonly property int fontSize: theme.fontSize ?? 12

    function ago(ms) {
        const s = Math.max(0, Math.round((now - ms) / 1000))
        if (s < 90) return "now"
        if (s < 5400) return Math.round(s / 60) + " min"
        if (s < 129600) return Math.round(s / 3600) + " h"
        return Math.round(s / 86400) + " d"
    }
    // history keeps the body as the app sent it (it may be markup): show it as plain text
    function plain(t) {
        return String(t || "").replace(/<[^>]*>/g, "").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, "\"").replace(/&amp;/g, "&").trim()
    }

    implicitHeight: col.implicitHeight + 16
    radius: 8
    color: hover.hovered ? (theme.buttonHover ?? "#323646") : (theme.button ?? "#272a36")
    border.width: 1
    border.color: unread ? Qt.rgba(accent.r, accent.g, accent.b, 0.55) : "transparent"
    Accessible.role: Accessible.Button
    Accessible.name: (unread ? "Unread: " : "") + entry.app + ": " + entry.summary

    HoverHandler { id: hover }
    TapHandler { onTapped: item.opened() }

    Rectangle {          // the group's colour, down the left edge
        x: 0; y: 8; width: 3; height: parent.height - 16; radius: 1.5
        color: item.accent
        opacity: item.unread ? 1 : 0.4
    }

    Column {
        id: col
        x: 14; y: 8
        width: parent.width - 14 - 30
        spacing: 2
        Row {
            spacing: 6
            width: parent.width
            Rectangle {
                visible: item.unread
                width: 7; height: 7; radius: 3.5
                anchors.verticalCenter: parent.verticalCenter
                color: item.accent
            }
            Text {
                width: parent.width - 13
                elide: Text.ElideRight
                text: item.entry.app + "  ·  " + item.ago(item.entry.t) + (item.entry.shown === false ? "  ·  held (" + item.entry.why + ")" : "")
                textFormat: Text.PlainText      // app-supplied text: never rich text (it could load remote images)
                color: item.theme.subtext ?? "#b9bccb"
                font.family: item.fontFamily
                font.pixelSize: Math.max(6, item.fontSize - 1)
            }
        }
        Text {
            width: parent.width
            visible: text !== ""
            text: item.plain(item.entry.summary)
            textFormat: Text.PlainText      // app-supplied text: never rich text (it could load remote images)
            elide: Text.ElideRight
            color: item.theme.text ?? "#f2f3f7"
            font.family: item.fontFamily
            font.pixelSize: item.fontSize + 1
            font.weight: item.unread ? Font.DemiBold : Font.Normal
        }
        Text {
            width: parent.width
            visible: text !== ""
            text: item.plain(item.entry.body)
            wrapMode: Text.Wrap
            maximumLineCount: 2
            elide: Text.ElideRight
            textFormat: Text.PlainText
            color: item.theme.subtext ?? "#b9bccb"
            font.family: item.fontFamily
            font.pixelSize: item.fontSize
        }
        Flow {
            width: parent.width
            visible: item.buttons.length > 0
            spacing: 6
            topPadding: 4
            Repeater {
                model: item.buttons
                delegate: CenterButton {
                    required property var modelData
                    theme: item.theme
                    text: modelData.text
                    onClicked: item.invoked(modelData.identifier)
                }
            }
        }
    }

    Text {                // forget this one
        anchors { top: parent.top; right: parent.right; topMargin: 6; rightMargin: 8 }
        visible: hover.hovered
        text: "✕"
        color: closeHover.hovered ? (item.theme.text ?? "#f2f3f7") : (item.theme.subtext ?? "#b9bccb")
        font.pixelSize: item.fontSize + 2
        Accessible.role: Accessible.Button
        Accessible.name: "Remove from the list"
        HoverHandler { id: closeHover }
        MouseArea { anchors.fill: parent; anchors.margins: -4; onClicked: item.removed() }   // keeps the click from the card
    }
}
