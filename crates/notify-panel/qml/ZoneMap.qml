import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// The mini screen: nine places on a grid (or free placement), with each zone drawn as the stack of popups it would show, to scale.
Rectangle {
    id: screenBox
    property var page
    Layout.alignment: Qt.AlignHCenter
    Layout.preferredWidth: Math.min(parent.width - Kirigami.Units.gridUnit * 2, Kirigami.Units.gridUnit * 30)
    Layout.preferredHeight: width * 9 / 16
    color: Kirigami.Theme.alternateBackgroundColor
    border.color: Kirigami.Theme.disabledTextColor
    border.width: 2
    radius: 6
    readonly property real chipW: Math.min(width * 0.3, Math.max(width * 0.18, width * (Number(page.vals.width) || 380) / 1920))
    // the numbers from "Size, spacing and limits", drawn to scale (the mini screen stands for a 1920 px wide screen)
    readonly property real px: width / 1920
    readonly property real cardH: Kirigami.Units.gridUnit * 1.15
    readonly property int stackN: Math.max(1, Math.min(3, Number(page.vals.maxVisible) > 0 ? Number(page.vals.maxVisible) : 3))
    readonly property real gapPx: Math.max(2, (page.vals.spacing !== undefined ? Number(page.vals.spacing) : 10) * px)
    readonly property real edge: Math.max(4, (page.vals.margin !== undefined ? Number(page.vals.margin) : 16) * px)
    readonly property real chipH: stackN * cardH + (stackN - 1) * gapPx

    // the nine places; click one to move the selected zone there
    Repeater {
        model: page.cornerIds.slice(0, 9)
        delegate: Item {
            id: cell
            required property string modelData
            required property int index
            width: screenBox.width / 3
            height: screenBox.height / 3
            x: (index % 3) * width
            y: Math.floor(index / 3) * height
            Rectangle {
                anchors.fill: parent
                anchors.margins: 3
                radius: 4
                color: cellArea.containsMouse && !page.selectedFree ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.18) : "transparent"
                border.width: 1
                border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, page.selectedFree ? 0.07 : 0.22)
            }
            MouseArea {
                id: cellArea
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: page.selectedFree ? Qt.ArrowCursor : Qt.PointingHandCursor
                onClicked: if (page.selectedId !== "" && !page.selectedFree) page.model.zoneSet(page.selectedId, "anchor", cell.modelData)
                Accessible.role: Accessible.Button
                Accessible.name: "Move the selected zone to " + page.anchorNames[cell.modelData]
            }
        }
    }

    // one draggable chip per zone
    Repeater {
        model: page.zones
        delegate: Rectangle {
            id: chip
            required property var modelData
            required property int index
            // where it sits among zones sharing its place (so they do not hide each other)
            readonly property int slot: { let n = 0; for (let i = 0; i < index; i++) if (page.zones[i].anchor === modelData.anchor) n++; return n }
            readonly property bool isFree: modelData.anchor === "free"
            readonly property int cellIdx: Math.max(0, page.cornerIds.indexOf(modelData.anchor))
            readonly property int col: isFree ? 0 : cellIdx % 3
            readonly property int row: isFree ? 0 : Math.floor(cellIdx / 3)
            readonly property real cw: screenBox.width / 3
            readonly property real ch: screenBox.height / 3
            readonly property real offset: slot * (screenBox.chipH + 3)
            readonly property bool atBottom: isFree ? Number(modelData.y) > 0.5 : row === 2
            readonly property real posX: isFree
                ? Number(modelData.x) * (screenBox.width - width)
                : col * cw + (col === 0 ? screenBox.edge : col === 2 ? cw - width - screenBox.edge : (cw - width) / 2)
            readonly property real posY: isFree
                ? (Number(modelData.y) <= 0.5 ? Number(modelData.y) * screenBox.height : Number(modelData.y) * screenBox.height - height)
                : (row === 0 ? screenBox.edge + offset : row === 2 ? (row + 1) * ch - height - screenBox.edge - offset : row * ch + (ch - height) / 2 + offset)
            width: screenBox.chipW
            height: screenBox.chipH
            x: posX
            y: posY
            z: modelData.id === page.selectedId ? 2 : 1
            color: "transparent"
            // the zone as the popups really stack in it: up to three cards, the one nearest the screen edge first, the rest faded (waiting)
            Repeater {
                model: screenBox.stackN
                delegate: Rectangle {
                    required property int index
                    width: chip.width
                    height: screenBox.cardH
                    y: chip.atBottom ? chip.height - (index + 1) * screenBox.cardH - index * screenBox.gapPx : index * (screenBox.cardH + screenBox.gapPx)
                    radius: 3
                    color: Kirigami.Theme.backgroundColor
                    opacity: index === 0 ? 1 : 0.55
                    border.width: chip.modelData.id === page.selectedId ? 2 : 1
                    border.color: chip.modelData.accent !== "" ? chip.modelData.accent : Kirigami.Theme.disabledTextColor
                    QQC2.Label {
                        visible: index === 0
                        anchors.centerIn: parent
                        text: chip.modelData.label || chip.modelData.id
                        elide: Text.ElideRight
                        width: parent.width - 8
                        horizontalAlignment: Text.AlignHCenter
                        textFormat: Text.PlainText
                    }
                }
            }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.OpenHandCursor
                drag.target: chip
                drag.minimumX: 0
                drag.maximumX: screenBox.width - chip.width
                drag.minimumY: 0
                drag.maximumY: screenBox.height - chip.height
                onPressed: page.selectedId = chip.modelData.id
                onReleased: {
                    if (drag.active && !chip.isFree) {
                        // snapping: the zone lands on the place under the middle of the chip
                        page.model.zoneSet(chip.modelData.id, "anchor", page.cellAt((chip.x + chip.width / 2) / screenBox.width, (chip.y + chip.height / 2) / screenBox.height))
                    } else if (drag.active) {
                        const span = Math.max(1, screenBox.width - chip.width)
                        const x = chip.x / span
                        // upper half: the zone's top edge; lower half: its bottom edge (popups stack upward from there)
                        const upper = chip.y + chip.height / 2 <= screenBox.height / 2
                        const y = upper ? chip.y / screenBox.height : (chip.y + chip.height) / screenBox.height
                        page.model.zonePlace(chip.modelData.id, x, y)
                    }
                    // a drag replaces the bindings; put them back so the chip always follows the draft
                    chip.x = Qt.binding(() => chip.posX)
                    chip.y = Qt.binding(() => chip.posY)
                }
                Accessible.name: "Zone " + (chip.modelData.label || chip.modelData.id) + ", drag to place it"
            }
        }
    }
}
