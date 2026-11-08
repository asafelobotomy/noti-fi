import QtQuick

// A popup's action buttons (Reply, Open, Pause ...). A tap here is the button's, not the card's: the card checks `hovered`
// before dismissing, so pressing a button of a resident notification (a running copy's Pause) leaves it on screen.
Flow {
    id: row
    required property var popup          // the Popup: acts, active, resident, colours, font; dismissed()
    readonly property bool hovered: hover.hovered
    spacing: 6
    HoverHandler { id: hover }
    Repeater {
        model: row.popup.acts
        delegate: Rectangle {
            id: btnRect
            required property var modelData
            radius: 6
            color: btn.hovered ? row.popup.cButtonHover : row.popup.cButton
            implicitWidth: btnLabel.implicitWidth + 20
            implicitHeight: 26
            HoverHandler { id: btn }
            TapHandler {
                enabled: row.popup.active
                onTapped: { btnRect.modelData.invoke(); if (!row.popup.resident) row.popup.dismissed() }
            }
            Text {
                id: btnLabel
                anchors.centerIn: parent
                text: btnRect.modelData.text
                textFormat: Text.PlainText
                color: row.popup.cText
                font.pixelSize: row.popup.fontSize
                font.family: row.popup.fontFamily
            }
        }
    }
}
