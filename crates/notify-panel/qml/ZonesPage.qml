import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Where popups appear: the zones (a name, a corner, an accent colour) and the layout numbers. Edits a draft; Save writes it.
Kirigami.Page {
    id: page
    title: "Zones & layout"
    property var model
    property var ui
    signal openTab(string name)
    readonly property bool online: model !== undefined && model !== null && model.connected === true

    readonly property var settings: { try { return JSON.parse(page.model ? page.model.settingsJson : "{}") } catch (e) { return {} } }
    readonly property var vals: page.settings.values !== undefined ? page.settings.values : ({})
    readonly property var zones: page.settings.zones !== undefined ? page.settings.zones : []
    property string selectedId: ""
    property bool sizesOpen: false
    readonly property var selectedZone: { for (const z of page.zones) if (z.id === page.selectedId) return z; return null }
    onZonesChanged: if (selectedZone === null && zones.length) selectedId = zones[0].id

    // the nine grid places row by row, then "free"
    readonly property var cornerIds: ["top-left", "top-center", "top-right", "middle-left", "center", "middle-right", "bottom-left", "bottom-center", "bottom-right", "free"]
    readonly property var anchorNames: ({ "top-left": "Top left", "top-center": "Top middle", "top-right": "Top right", "middle-left": "Middle left", "center": "Middle",
                                          "middle-right": "Middle right", "bottom-left": "Bottom left", "bottom-center": "Bottom middle", "bottom-right": "Bottom right",
                                          "free": "Free (drag it)" })
    readonly property bool selectedFree: page.selectedZone !== null && page.selectedZone.anchor === "free"
    // snap: the nearest of the nine places to a point on the mini screen (fractions 0..1); free: where a place sits (x, y as the zone stores them)
    function cellAt(fx, fy) {
        const c = fx < 1 / 3 ? 0 : fx > 2 / 3 ? 2 : 1
        const r = fy < 1 / 3 ? 0 : fy > 2 / 3 ? 2 : 1
        return page.cornerIds[r * 3 + c]
    }
    function snapSelected() {
        const z = page.selectedZone
        if (z === null || z.anchor !== "free") return
        const fx = Number(z.x), fy = Number(z.y)
        page.model.zoneSet(z.id, "anchor", page.cellAt(fx, fy))
    }
    function freeSelected() {
        const z = page.selectedZone
        if (z === null || z.anchor === "free") return
        const i = Math.max(0, page.cornerIds.indexOf(z.anchor))
        page.model.zonePlace(z.id, (i % 3) / 2, [0.03, 0.45, 0.97][Math.floor(i / 3)])
    }
    function zonesAt(anchor) { return page.zones.filter(z => z.anchor === anchor) }
    function screenNames() {
        const n = ["(first screen)"]
        for (const s of Qt.application.screens) n.push(s.name)
        if (page.vals.screen && n.indexOf(page.vals.screen) < 0) n.push(page.vals.screen)
        return n
    }

    readonly property var uiState: ({
        page: "zones",
        zones: page.zones.length,
        topLeft: page.zonesAt("top-left").map(z => z.label || z.id).join(","),
        topRight: page.zonesAt("top-right").map(z => z.label || z.id).join(","),
        bottomLeft: page.zonesAt("bottom-left").map(z => z.label || z.id).join(","),
        bottomRight: page.zonesAt("bottom-right").map(z => z.label || z.id).join(","),
        topCenter: page.zonesAt("top-center").map(z => z.label || z.id).join(","),
        center: page.zonesAt("center").map(z => z.label || z.id).join(","),
        free: page.zonesAt("free").map(z => z.label || z.id).join(","),
        selected: page.selectedId,
        maxVisible: page.vals.maxVisible !== undefined ? page.vals.maxVisible : -1,
        dirty: page.model ? page.model.dirty : false,
        valid: page.model ? page.model.draftValid : false,
        banner: banners.errorText,
        saveMessage: banners.saveText
    })

    actions: [
        Kirigami.Action {
            text: "Save"; icon.name: "document-save"
            enabled: page.online && page.model.dirty && page.model.draftValid && !page.model.checking
            onTriggered: page.model.rulesSave(false)
        },
        Kirigami.Action {
            text: "Revert"; icon.name: "document-revert"
            enabled: page.online && (page.model.dirty || page.model.externalChanged)
            onTriggered: page.model.rulesReload()
        },
        Kirigami.Action {
            text: "Preview on screen"; icon.name: "preview"
            enabled: page.online && page.model.draftValid
            onTriggered: page.model.rulesPreview()
        },
        Kirigami.Action {
            text: "Add zone"; icon.name: "list-add"
            onTriggered: page.selectedId = page.model.zoneAdd()
        }
    ]

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        DraftBanners { id: banners; Layout.fillWidth: true; model: page.model }

        QQC2.ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            ColumnLayout {
                width: parent.width
                spacing: Kirigami.Units.largeSpacing

                // ---- pick the zone to edit (the map and the card below follow it)
                Flow {
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    Layout.bottomMargin: 0
                    spacing: Kirigami.Units.smallSpacing
                    Repeater {
                        model: page.zones
                        delegate: QQC2.Button {
                            id: pick
                            required property var modelData
                            checkable: true; autoExclusive: true
                            checked: modelData.id === page.selectedId
                            onClicked: page.selectedId = modelData.id
                            contentItem: RowLayout {
                                spacing: Kirigami.Units.smallSpacing
                                Rectangle { implicitWidth: 10; implicitHeight: 10; radius: 5; color: pick.modelData.accent !== "" ? pick.modelData.accent : Kirigami.Theme.disabledTextColor }
                                QQC2.Label { text: pick.modelData.label || pick.modelData.id; textFormat: Text.PlainText }
                            }
                            Accessible.name: "Edit zone " + (modelData.label || modelData.id)
                        }
                    }
                    QQC2.Button { icon.name: "list-add"; text: "Add zone"; flat: true; onClicked: page.selectedId = page.model.zoneAdd(); Accessible.name: "Add a zone" }
                }

                // ---- the mini screen: nine places on a grid, or drag a zone anywhere ("free")
                QQC2.Label {
                    Layout.margins: Kirigami.Units.largeSpacing
                    Layout.bottomMargin: 0
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    opacity: 0.7
                    text: page.selectedZone === null ? "Add a zone to place it on the screen."
                        : page.selectedFree ? "Freeform: drag “" + (page.selectedZone.label || page.selectedZone.id) + "” anywhere on the screen."
                        : "Snap to grid: click a square, or drag “" + (page.selectedZone.label || page.selectedZone.id) + "”, and it lands on the nearest of nine places."
                }
                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: 0
                    enabled: page.selectedZone !== null
                    QQC2.Button {
                        text: "Snap to grid"; icon.name: "view-grid"
                        checkable: true; autoExclusive: true; checked: !page.selectedFree
                        onClicked: page.snapSelected()
                        Accessible.name: "Snap the selected zone to the grid"
                    }
                    QQC2.Button {
                        text: "Freeform"; icon.name: "transform-move"
                        checkable: true; autoExclusive: true; checked: page.selectedFree
                        onClicked: page.freeSelected()
                        Accessible.name: "Place the selected zone freely"
                    }
                }
                Rectangle {
                    id: screenBox
                    Layout.alignment: Qt.AlignHCenter
                    Layout.preferredWidth: Math.min(parent.width - Kirigami.Units.gridUnit * 2, Kirigami.Units.gridUnit * 30)
                    Layout.preferredHeight: width * 9 / 16
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: Kirigami.Theme.disabledTextColor
                    border.width: 2
                    radius: 6
                    readonly property real chipW: Math.min(width * 0.3, Math.max(width * 0.18, width * (Number(page.vals.width) || 380) / 1920))
                    readonly property real chipH: Kirigami.Units.gridUnit * 1.6

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
                            readonly property real posX: isFree
                                ? Number(modelData.x) * (screenBox.width - width)
                                : col * cw + (col === 0 ? 8 : col === 2 ? cw - width - 8 : (cw - width) / 2)
                            readonly property real posY: isFree
                                ? (Number(modelData.y) <= 0.5 ? Number(modelData.y) * screenBox.height : Number(modelData.y) * screenBox.height - height)
                                : (row === 0 ? 8 + offset : row === 2 ? (row + 1) * ch - height - 8 - offset : row * ch + (ch - height) / 2 + offset)
                            width: screenBox.chipW
                            height: screenBox.chipH
                            x: posX
                            y: posY
                            z: modelData.id === page.selectedId ? 2 : 1
                            radius: 4
                            color: Kirigami.Theme.backgroundColor
                            border.width: modelData.id === page.selectedId ? 2 : 1
                            border.color: modelData.accent !== "" ? modelData.accent : Kirigami.Theme.disabledTextColor
                            QQC2.Label {
                                anchors.centerIn: parent
                                text: chip.modelData.label || chip.modelData.id
                                elide: Text.ElideRight
                                width: parent.width - 8
                                horizontalAlignment: Text.AlignHCenter
                                textFormat: Text.PlainText
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

                // ---- the selected zone
                Repeater {
                    model: page.selectedZone ? [page.selectedZone] : []
                    delegate: Kirigami.AbstractCard {
                        id: card
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        contentItem: ColumnLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Kirigami.Heading { level: 4; text: "Zone: " + (card.modelData.label || card.modelData.id) }
                            Kirigami.FormLayout {
                                QQC2.TextField {
                                    id: labelField
                                    Kirigami.FormData.label: "Name:"
                                    Layout.fillWidth: true
                                    placeholderText: card.modelData.id
                                    Binding on text { when: !labelField.activeFocus; value: card.modelData.label }
                                    onTextEdited: page.model.zoneSet(card.modelData.id, "label", text)
                                    onActiveFocusChanged: if (activeFocus) page.selectedId = card.modelData.id
                                    Accessible.name: "Zone name " + card.modelData.id
                                }
                                QQC2.ComboBox {
                                    Kirigami.FormData.label: "Place:"
                                    model: page.cornerIds.map(a => page.anchorNames[a])
                                    currentIndex: page.cornerIds.indexOf(card.modelData.anchor)
                                    onActivated: index => page.model.zoneSet(card.modelData.id, "anchor", page.cornerIds[index])
                                    Accessible.name: "Place of " + card.modelData.id
                                }
                                QQC2.Label {
                                    visible: card.modelData.anchor === "free"
                                    Kirigami.FormData.label: "Position:"
                                    text: Math.round(card.modelData.x * 100) + "% from the left, " + Math.round(card.modelData.y * 100) + "% from the top"
                                    opacity: 0.8
                                }
                                ColorField {
                                    Kirigami.FormData.label: "Accent colour:"
                                    value: card.modelData.accent
                                    hint: "#60a5fa"
                                    onEdited: text => page.model.zoneSet(card.modelData.id, "accent", text)
                                }
                                RowLayout {
                                    Kirigami.FormData.label: "Used by:"
                                    QQC2.Label { text: card.modelData.rules === 1 ? "1 rule" : card.modelData.rules + " rules"; Layout.fillWidth: true }
                                    QQC2.Label { visible: page.vals.defaultZone === card.modelData.id; text: "default zone"; color: Kirigami.Theme.positiveTextColor }
                                    QQC2.Button {
                                        icon.name: "edit-delete"; text: "Delete zone"
                                        onClicked: page.model.zoneDelete(card.modelData.id)
                                        Accessible.name: "Delete zone " + card.modelData.id
                                    }
                                }
                            }
                            QQC2.Button {
                                Layout.alignment: Qt.AlignHCenter
                                text: look.visible ? "Hide look and behavior" : "Change how this zone's popups look…"
                                icon.name: look.visible ? "go-up" : "go-down"
                                onClicked: look.visible = !look.visible
                            }
                            StyleFields {
                                id: look
                                visible: false
                                Layout.fillWidth: true
                                showGroup: false
                                inheritFrom: "the theme"
                                values: card.modelData.style
                                onEdited: (key, text) => page.model.zoneStyle(card.modelData.id, key, text)
                            }
                        }
                    }
                }

                // ---- layout numbers
                Kirigami.FormLayout {
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }
                    Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "All zones" }
                    QQC2.ComboBox {
                        Kirigami.FormData.label: "Show popups on:"
                        model: page.screenNames()
                        currentIndex: page.vals.screen ? Math.max(0, model.indexOf(page.vals.screen)) : 0
                        onActivated: index => page.model.settingSet("screen", index === 0 ? "" : model[index])
                        Accessible.name: "Screen"
                    }
                    QQC2.ComboBox {
                        Kirigami.FormData.label: "Default zone:"
                        model: page.zones.map(z => z.id)
                        currentIndex: Math.max(0, model.indexOf(page.vals.defaultZone))
                        onActivated: index => page.model.settingSet("defaultZone", model[index])
                        Accessible.name: "Default zone"
                    }
                    QQC2.Button {
                        Kirigami.FormData.label: ""
                        icon.name: page.sizesOpen ? "go-up" : "go-down"
                        text: page.sizesOpen ? "Hide size, spacing and limits" : "Size, spacing and limits…"
                        onClicked: page.sizesOpen = !page.sizesOpen
                    }
                    Repeater {
                        model: !page.sizesOpen ? [] : [
                            { path: "maxVisible", label: "Popups per zone:", note: "0 = no limit; extras wait" },
                            { path: "width", label: "Popup width (px):", note: "" },
                            { path: "margin", label: "Gap from the screen edge (px):", note: "" },
                            { path: "spacing", label: "Gap between popups (px):", note: "" },
                            { path: "defaultTimeout", label: "Default timeout (ms):", note: "0 = stays until dismissed" }
                        ]
                        delegate: RowLayout {
                            id: numRow
                            required property var modelData
                            Kirigami.FormData.label: modelData.label
                            QQC2.TextField {
                                id: num
                                Layout.preferredWidth: Kirigami.Units.gridUnit * 7
                                inputMethodHints: Qt.ImhDigitsOnly
                                validator: IntValidator { bottom: 0 }
                                Binding on text { when: !num.activeFocus; value: page.vals[numRow.modelData.path] !== undefined ? String(page.vals[numRow.modelData.path]) : "" }
                                onTextEdited: page.model.settingSet(numRow.modelData.path, text)
                                Accessible.name: numRow.modelData.label
                            }
                            QQC2.Label { visible: numRow.modelData.note !== ""; opacity: 0.7; text: numRow.modelData.note }
                        }
                    }
                }
            }
        }
    }
}
