import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import "Json.js" as Json

// Where popups appear: the zones (a name, a corner, an accent colour) and the layout numbers. Edits a draft; Save writes it.
Kirigami.Page {
    id: page
    title: "Groups"
    property var model
    property var ui
    signal openTab(string name)
    readonly property bool online: model !== undefined && model !== null && model.connected === true

    readonly property var settings: Json.parse(page.model ? page.model.settingsJson : "", ({}))
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
        if (z !== null && z.anchor === "free") page.model.zoneSet(z.id, "anchor", page.cellAt(Number(z.x), Number(z.y)))
    }
    function freeSelected() {
        const z = page.selectedZone, i = z ? Math.max(0, page.cornerIds.indexOf(z.anchor)) : 0
        if (z !== null && z.anchor !== "free") page.model.zonePlace(z.id, (i % 3) / 2, [0.03, 0.45, 0.97][Math.floor(i / 3)])
    }
    function zonesAt(anchor) { return page.zones.filter(z => z.anchor === anchor) }
    function screenNames() {
        const n = ["(first screen)", "(focused monitor, Hyprland)"]
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
            GridLayout {
                width: parent.width
                columns: page.width > Kirigami.Units.gridUnit * 46 ? 2 : 1
                columnSpacing: Kirigami.Units.largeSpacing
                rowSpacing: Kirigami.Units.largeSpacing
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop
                    Layout.preferredWidth: 1
                    spacing: Kirigami.Units.largeSpacing

                // ---- pick the group to edit (its card below and its place on the map follow it)
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
                            Accessible.name: "Edit group " + (modelData.label || modelData.id)
                        }
                    }
                    QQC2.Button { icon.name: "list-add"; text: "Add group"; flat: true; onClicked: page.selectedId = page.model.zoneAdd(); Accessible.name: "Add a group" }
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
                            Kirigami.Heading { level: 4; text: "Group: " + (card.modelData.label || card.modelData.id) }
                            Kirigami.FormLayout {
                                QQC2.TextField {
                                    id: labelField
                                    enabled: card.modelData.id !== "system"        // System is locked: it always exists, under its own name
                                    Kirigami.FormData.label: card.modelData.id === "system" ? "Name 🔒:" : "Name:"
                                    Layout.fillWidth: true
                                    placeholderText: card.modelData.id
                                    Binding on text { when: !labelField.activeFocus; value: card.modelData.label }
                                    onTextEdited: page.model.zoneSet(card.modelData.id, "label", text)
                                    onActiveFocusChanged: if (activeFocus) page.selectedId = card.modelData.id
                                    Accessible.name: "Zone name " + card.modelData.id
                                }
                                QQC2.Label {
                                    visible: card.modelData.anchor === "free"
                                    Kirigami.FormData.label: "Position:"
                                    text: Math.round(card.modelData.x * 100) + "% from the left, " + Math.round(card.modelData.y * 100) + "% from the top"
                                    opacity: 0.8
                                }
                                SwatchPicker {
                                    Kirigami.FormData.label: "Colour:"
                                    accessibleName: "Group colour"
                                    value: card.modelData.accent
                                    onEdited: text => page.model.zoneSet(card.modelData.id, "accent", text)
                                }
                                RowLayout {
                                    Kirigami.FormData.label: ""
                                    QQC2.Label { visible: page.vals.defaultZone === card.modelData.id; text: "Default group: notifications no rule places go here"; color: Kirigami.Theme.positiveTextColor; Layout.fillWidth: true }
                                    QQC2.Label { visible: card.modelData.id === "system"; text: "🔒 System always stays"; opacity: 0.7; Layout.fillWidth: page.vals.defaultZone !== card.modelData.id }
                                    QQC2.Button {
                                        visible: card.modelData.id !== "system"
                                        icon.name: "edit-delete"; text: "Delete group"
                                        onClicked: page.model.zoneDelete(card.modelData.id)
                                        Accessible.name: "Delete group " + card.modelData.id
                                    }
                                }
                            }
                            GroupMembers {
                                Layout.fillWidth: true
                                model: page.model
                                groupId: card.modelData.id
                                groupLabel: card.modelData.label || card.modelData.id
                                onOpenRules: page.openTab("rules")
                            }
                            Kirigami.Heading { level: 4; text: "How this group's popups look and behave" }
                            LookEditor {
                                id: look
                                Layout.fillWidth: true
                                showGroup: false
                                inheritFrom: "the theme"
                                levels: ["Theme", "Group: " + (card.modelData.label || card.modelData.id)]
                                values: card.modelData.style
                                model: page.model
                                accent: card.modelData.accent
                                onEdited: (key, text) => page.model.zoneStyle(card.modelData.id, key, text)
                            }
                        }
                    }
                }

                }
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignTop
                    Layout.preferredWidth: 1
                    spacing: Kirigami.Units.largeSpacing
                Kirigami.Heading { Layout.leftMargin: Kirigami.Units.largeSpacing; Layout.topMargin: Kirigami.Units.largeSpacing; level: 3; text: "Where groups appear on screen" }
                // ---- the mini screen: nine places on a grid, or drag a zone anywhere ("free")
                QQC2.Label {
                    Layout.margins: Kirigami.Units.largeSpacing
                    Layout.bottomMargin: 0
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    opacity: 0.7
                    text: page.selectedZone === null ? "Add a group to place it on the screen."
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
                ZoneMap { page: page }

                // ---- layout numbers
                Kirigami.FormLayout {
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }
                    Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "All groups" }
                    QQC2.ComboBox {
                        Kirigami.FormData.label: "Show popups on:"
                        model: page.screenNames()
                        currentIndex: page.vals.screen === "focused" ? 1 : page.vals.screen ? Math.max(0, model.indexOf(page.vals.screen)) : 0
                        onActivated: index => page.model.settingSet("screen", index === 0 ? "" : index === 1 ? "focused" : model[index])
                        Accessible.name: "Screen"
                    }
                    QQC2.ComboBox {
                        Kirigami.FormData.label: "Default group:"
                        model: page.zones.map(z => z.label || z.id)
                        displayText: { const z = page.zones.find(x => x.id === page.vals.defaultZone); return z ? (z.label || z.id) : "" }
                        currentIndex: Math.max(0, page.zones.findIndex(z => z.id === page.vals.defaultZone))
                        onActivated: index => page.model.settingSet("defaultZone", page.zones[index].id)
                        Accessible.name: "Default zone"
                    }
                    DurationChips {
                        Kirigami.FormData.label: "Popups stay:"
                        Layout.fillWidth: true
                        allowUnset: false
                        value: page.vals.defaultTimeout !== undefined ? String(page.vals.defaultTimeout) : "6000"
                        onEdited: text => page.model.settingSet("defaultTimeout", text)
                    }
                    QQC2.Button {
                        Kirigami.FormData.label: ""
                        icon.name: page.sizesOpen ? "go-up" : "go-down"
                        text: page.sizesOpen ? "Hide size, spacing and limits" : "Size, spacing and limits…"
                        onClicked: page.sizesOpen = !page.sizesOpen
                    }
                    Repeater {
                        model: !page.sizesOpen ? [] : [
                            { path: "maxVisible", label: "Popups per group:", note: "0 = no limit; extras wait" },
                            { path: "width", label: "Popup width (px):", note: "" },
                            { path: "margin", label: "Gap from the screen edge (px):", note: "" },
                            { path: "spacing", label: "Gap between popups (px):", note: "" }

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
}
