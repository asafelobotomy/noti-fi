import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// The rules editor. Everything here edits a DRAFT held by the Rust model; nothing reaches notifd until Save.
// The daemon validates every edit and simulates the draft, so what the simulator says is what will happen.
Kirigami.Page {
    id: page
    title: "Rules"
    padding: 0
    property var model
    property var ui
    readonly property bool online: model !== undefined && model !== null && model.connected === true

    // ---- the list: a ListModel updated in place, so typing in the editor never loses focus or scroll position
    property int selected: -1
    ListModel { id: rules }
    readonly property var current: selected >= 0 && selected < rules.count ? rules.get(selected) : null

    function flat(r) {
        return {
            index: r.index, title: r.title, name: r.name, summary: r.summary, zone: r.zone, timeout: r.timeout, silent: r.silent,
            matched: r.matched, decidesZone: r.decidesZone, decidesTimeout: r.decidesTimeout, decidesSilent: r.decidesSilent, hasError: r.hasError,
            mAppName: r.match.appName, mDesktopEntry: r.match.desktopEntry, mOrigin: r.match.origin,
            mSummary: r.match.summary, mBody: r.match.body, mUrgency: r.match.urgency, mCategory: r.match.category, mChannel: r.match.channel,
            sOpacity: r.style.opacity, sClickThrough: r.style.clickThrough, sNoFocus: r.style.noFocus, sWidth: r.style.width,
            sFontFamily: r.style.fontFamily, sFontSize: r.style.fontSize, sGroup: r.style.group,
            sPriority: r.style.priority, sMedia: r.style.media, sStack: r.style.stack, sTimer: r.style.timerStyle
        }
    }
    function sync() {
        if (!page.model) return
        let list = []
        try { list = JSON.parse(page.model.rulesJson) } catch (e) { return }
        while (rules.count > list.length) rules.remove(rules.count - 1)
        for (let i = 0; i < list.length; i++) {
            const row = flat(list[i])
            if (i < rules.count) rules.set(i, row); else rules.append(row)
        }
        if (selected >= rules.count) selected = rules.count - 1
        if (selected < 0 && rules.count > 0) selected = 0
    }
    Connections { target: page.model; function onRulesJsonChanged() { page.sync() } }
    Component.onCompleted: sync()

    readonly property var zones: {
        try { return JSON.parse(page.model ? page.model.draftZonesJson : "[]") } catch (e) { return [] }
    }
    readonly property var zoneChoices: {
        const z = ["(leave to later rules)"].concat(page.zones)
        if (page.current && page.current.zone !== "" && z.indexOf(page.current.zone) < 0) z.push(page.current.zone)
        return z
    }
    readonly property var sim: {
        try { return JSON.parse(page.model ? page.model.simJson : "{}") } catch (e) { return {} }
    }

    // what the user actually sees (read by the headless tests)
    readonly property var uiState: ({
        page: "rules",
        rows: rules.count,
        selected: page.selected,
        selectedTitle: page.current ? page.current.title : "",
        selectedSummary: page.current ? page.current.summary : "",
        dirty: page.model ? page.model.dirty : false,
        valid: page.model ? page.model.draftValid : false,
        banner: errorBanner.visible ? errorBanner.text : "",
        infoBanner: externalBanner.visible ? externalBanner.text : "",
        saveMessage: saveBanner.visible ? saveBanner.text : "",
        simZone: page.sim.zone !== undefined ? page.sim.zone : "",
        simZoneBy: page.sim.zoneBy !== undefined ? page.sim.zoneBy : "",
        simOutcome: page.sim.outcome !== undefined ? page.sim.outcome : "",
        simError: page.sim.error !== undefined ? page.sim.error : "",
        matchedRows: (function () { let n = 0; for (let i = 0; i < rules.count; i++) if (rules.get(i).matched) n++; return n })(),
        conflictDialog: conflictDialog.visible
    })

    actions: [
        Kirigami.Action {
            text: "Save"
            icon.name: "document-save"
            enabled: page.online && page.model.dirty && page.model.draftValid && !page.model.checking
            onTriggered: page.model.rulesSave(false)
        },
        Kirigami.Action {
            text: "Revert"
            icon.name: "document-revert"
            enabled: page.online && (page.model.dirty || page.model.externalChanged)
            onTriggered: page.model.rulesReload()
        },
        Kirigami.Action {
            text: "Preview on screen"
            icon.name: "preview"
            enabled: page.online && page.model.draftValid
            onTriggered: page.model.rulesPreview()
        },
        Kirigami.Action {
            text: "Add rule"
            icon.name: "list-add"
            onTriggered: page.selected = page.model.ruleAdd()
        }
    ]

    // ---- the file was changed by hand while there were unsaved edits
    Kirigami.PromptDialog {
        id: conflictDialog
        title: "rules.json changed on disk"
        subtitle: "Someone edited the file after you started. Overwrite it with your version, or reload it and lose your edits? (A .bak copy of the file on disk is kept either way when you overwrite.)"
        standardButtons: Kirigami.Dialog.NoButton
        visible: page.model ? page.model.conflict : false
        showCloseButton: true
        customFooterActions: [
            Kirigami.Action { text: "Overwrite with my version"; icon.name: "document-save"; onTriggered: page.model.rulesSave(true) },
            Kirigami.Action { text: "Reload from disk"; icon.name: "view-refresh"; onTriggered: page.model.rulesReload() }
        ]
        onClosed: if (page.model && page.model.conflict) page.model.rulesReload()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ---- messages
        Kirigami.InlineMessage {
            id: errorBanner
            Layout.fillWidth: true
            Layout.margins: visible ? Kirigami.Units.smallSpacing : 0
            visible: page.model !== undefined && page.model !== null && !page.model.draftValid && !page.model.checking && page.model.draftError !== ""
            type: Kirigami.MessageType.Error
            text: page.model ? "Cannot save: " + page.model.draftError : ""
            showCloseButton: false
        }
        Kirigami.InlineMessage {
            id: externalBanner
            Layout.fillWidth: true
            Layout.margins: visible ? Kirigami.Units.smallSpacing : 0
            visible: page.model !== undefined && page.model !== null && page.model.externalChanged
            type: Kirigami.MessageType.Information
            text: "rules.json was edited outside this panel. Saving will ask before overwriting it."
            showCloseButton: false
            actions: [ Kirigami.Action { text: "Reload"; onTriggered: page.model.rulesReload() } ]
        }
        Kirigami.InlineMessage {
            id: saveBanner
            Layout.fillWidth: true
            Layout.margins: visible ? Kirigami.Units.smallSpacing : 0
            visible: page.model !== undefined && page.model !== null && page.model.saveMessage !== ""
            type: page.model && page.model.conflict ? Kirigami.MessageType.Warning : Kirigami.MessageType.Positive
            text: page.model ? page.model.saveMessage : ""
        }

        GridLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            columns: width > Kirigami.Units.gridUnit * 44 ? 2 : 1
            columnSpacing: 0
            rowSpacing: 0

            // ================= left: the ordered list of rules =================
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredWidth: 1
                Layout.minimumHeight: Kirigami.Units.gridUnit * 10
                Layout.preferredHeight: parent.columns === 1 ? Kirigami.Units.gridUnit * 16 : -1
                spacing: 0

                QQC2.Label {
                    Layout.margins: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    opacity: 0.7
                    text: "Checked top to bottom. For the zone, timeout and silent setting, the first matching rule that sets it wins."
                }
                Kirigami.Separator { Layout.fillWidth: true }

                ListView {
                    id: list
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: rules
                    currentIndex: page.selected
                    delegate: QQC2.ItemDelegate {
                        id: row
                        required property int index
                        required property string title
                        required property string summary
                        required property bool matched
                        required property bool decidesZone
                        required property bool decidesTimeout
                        required property bool decidesSilent
                        required property bool hasError
                        width: ListView.view.width
                        highlighted: ListView.isCurrentItem
                        onClicked: page.selected = index
                        contentItem: RowLayout {
                            spacing: Kirigami.Units.smallSpacing
                            QQC2.Label { text: (row.index + 1) + "."; opacity: 0.6; Layout.alignment: Qt.AlignTop }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 0
                                QQC2.Label { Layout.fillWidth: true; elide: Text.ElideRight; font.bold: true; text: row.title; textFormat: Text.PlainText }
                                QQC2.Label { Layout.fillWidth: true; elide: Text.ElideRight; opacity: 0.7; text: row.summary; textFormat: Text.PlainText }
                            }
                            Kirigami.Icon {
                                visible: row.hasError
                                source: "dialog-error"
                                Layout.preferredWidth: Kirigami.Units.iconSizes.small
                                Layout.preferredHeight: Kirigami.Units.iconSizes.small
                            }
                            QQC2.Label {
                                visible: row.matched
                                color: Kirigami.Theme.positiveTextColor
                                font.pixelSize: Kirigami.Theme.smallFont.pixelSize
                                text: {
                                    const d = []
                                    if (row.decidesZone) d.push("zone")
                                    if (row.decidesTimeout) d.push("timeout")
                                    if (row.decidesSilent) d.push("silent")
                                    return d.length ? "matches · sets " + d.join(", ") : "matches"
                                }
                            }
                        }
                    }
                    Kirigami.PlaceholderMessage {
                        anchors.centerIn: parent
                        width: parent.width - Kirigami.Units.gridUnit * 4
                        visible: rules.count === 0
                        icon.name: "view-list-details"
                        text: page.online ? "No rules yet" : "notifd is not running"
                        explanation: page.online ? "Without rules, every notification goes to the default zone." : ""
                    }
                }

                Kirigami.Separator { Layout.fillWidth: true }
                RowLayout {
                    Layout.margins: Kirigami.Units.smallSpacing
                    QQC2.ToolButton { icon.name: "go-up"; text: "Up"; enabled: page.selected > 0; onClicked: page.selected = page.model.ruleMove(page.selected, -1) }
                    QQC2.ToolButton { icon.name: "go-down"; text: "Down"; enabled: page.selected >= 0 && page.selected < rules.count - 1; onClicked: page.selected = page.model.ruleMove(page.selected, 1) }
                    QQC2.ToolButton { icon.name: "edit-copy"; text: "Duplicate"; enabled: page.selected >= 0; onClicked: page.selected = page.model.ruleDuplicate(page.selected) }
                    Item { Layout.fillWidth: true }
                    QQC2.ToolButton { icon.name: "edit-delete"; text: "Delete"; enabled: page.selected >= 0; onClicked: page.model.ruleDelete(page.selected) }
                }
            }

            Kirigami.Separator { Layout.fillHeight: true; visible: parent.columns === 2 }

            // ================= right: the editor and the simulator =================
            QQC2.ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredWidth: 1
                contentWidth: availableWidth

                ColumnLayout {
                    width: parent.width
                    spacing: Kirigami.Units.largeSpacing

                    // ---- the selected rule
                    Kirigami.FormLayout {
                        Layout.fillWidth: true
                        Layout.margins: Kirigami.Units.largeSpacing
                        enabled: page.current !== null

                        Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Rule" }
                        QQC2.TextField {
                            id: nameField
                            Kirigami.FormData.label: "Name:"
                            Layout.fillWidth: true
                            placeholderText: "(unnamed rule)"
                            Binding on text { when: !nameField.activeFocus; value: page.current ? page.current.name : "" }
                            onTextEdited: page.model.ruleSetName(page.selected, text)
                        }

                        Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Matches when…" }
                        QQC2.Label {
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                            opacity: 0.7
                            font.pixelSize: Kirigami.Theme.smallFont.pixelSize
                            text: "Regular expressions, case-insensitive. Leave a field empty to ignore it."
                        }
                        Repeater {
                            model: [
                                { field: "appName", label: "App name:", role: "mAppName", hint: "^steam$" },
                                { field: "desktopEntry", label: "Desktop entry:", role: "mDesktopEntry", hint: "^org\\.kde\\." },
                                { field: "origin", label: "Site (from a browser):", role: "mOrigin", hint: "example-shop|example-chat" },
                                { field: "summary", label: "Title contains:", role: "mSummary", hint: "message" },
                                { field: "body", label: "Text contains:", role: "mBody", hint: "" },
                                { field: "urgency", label: "Urgency:", role: "mUrgency", hint: "critical" },
                                { field: "category", label: "Kind (category):", role: "mCategory", hint: "^email" },
                                { field: "channel", label: "Arrives as:", role: "mChannel", hint: "web" }
                            ]
                            delegate: QQC2.TextField {
                                id: matchField
                                required property var modelData
                                Kirigami.FormData.label: modelData.label
                                Layout.fillWidth: true
                                placeholderText: modelData.hint !== "" ? "e.g. " + modelData.hint : ""
                                font.family: "monospace"
                                Binding on text { when: !matchField.activeFocus; value: page.current ? page.current[matchField.modelData.role] : "" }
                                onTextEdited: page.model.ruleSetMatch(page.selected, matchField.modelData.field, text)
                                Accessible.name: "Match " + modelData.label
                            }
                        }

                        Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Then" }
                        QQC2.ComboBox {
                            id: zoneBox
                            Kirigami.FormData.label: "Show in zone:"
                            model: page.zoneChoices
                            currentIndex: page.current && page.current.zone !== "" ? Math.max(0, page.zoneChoices.indexOf(page.current.zone)) : 0
                            onActivated: index => page.model.ruleSetZone(page.selected, index === 0 ? "" : page.zoneChoices[index])
                            Accessible.name: "Show in zone"
                        }
                        QQC2.TextField {
                            id: timeoutField
                            Kirigami.FormData.label: "Timeout (ms):"
                            placeholderText: "the app's own, or the default"
                            inputMethodHints: Qt.ImhDigitsOnly
                            validator: IntValidator { bottom: 0 }
                            Binding on text { when: !timeoutField.activeFocus; value: page.current ? page.current.timeout : "" }
                            onTextEdited: page.model.ruleSetTimeout(page.selected, text)
                            Accessible.name: "Timeout in milliseconds"
                        }
                        QQC2.Label {
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                            opacity: 0.7
                            font.pixelSize: Kirigami.Theme.smallFont.pixelSize
                            text: "0 keeps the popup until you dismiss it."
                        }
                        QQC2.CheckBox {
                            Kirigami.FormData.label: "Never show:"
                            text: "keep it in history only"
                            checked: page.current ? page.current.silent : false
                            onToggled: {
                                page.model.ruleSetSilent(page.selected, checked)
                                checked = Qt.binding(() => page.current ? page.current.silent : false)
                            }
                            Accessible.name: "Never show, keep in history only"
                        }
                    }

                    Kirigami.Heading { Layout.leftMargin: Kirigami.Units.largeSpacing; level: 3; text: "Look and behavior" }
                    StyleFields {
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        enabled: page.current !== null
                        inheritFrom: "later rules, the zone and the theme"
                        values: page.current ? ({ opacity: page.current.sOpacity, clickThrough: page.current.sClickThrough, noFocus: page.current.sNoFocus, width: page.current.sWidth,
                                                  fontFamily: page.current.sFontFamily, fontSize: page.current.sFontSize, group: page.current.sGroup,
                                                  priority: page.current.sPriority, media: page.current.sMedia, stack: page.current.sStack, timerStyle: page.current.sTimer }) : ({})
                        onEdited: (key, text) => page.model.ruleSetStyle(page.selected, key, text)
                    }

                    Kirigami.Separator { Layout.fillWidth: true }

                    // ---- the simulator
                    Kirigami.FormLayout {
                        Layout.fillWidth: true
                        Layout.margins: Kirigami.Units.largeSpacing

                        Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Simulator: what would happen to a notification like this?" }
                        Repeater {
                            model: [
                                { field: "appName", label: "App name:", hint: "Brave Origin" },
                                { field: "origin", label: "Site:", hint: "example-shop.com" },
                                { field: "category", label: "Kind (category):", hint: "email.arrived" },
                                { field: "desktopEntry", label: "Desktop entry:", hint: "" },
                                { field: "summary", label: "Title:", hint: "New message" },
                                { field: "body", label: "Text:", hint: "" }
                            ]
                            delegate: QQC2.TextField {
                                id: simField
                                required property var modelData
                                Kirigami.FormData.label: modelData.label
                                Layout.fillWidth: true
                                placeholderText: modelData.hint
                                onTextEdited: page.model.simSet(simField.modelData.field, text)
                                Accessible.name: "Simulate " + modelData.label
                            }
                        }
                        QQC2.ComboBox {
                            Kirigami.FormData.label: "Urgency:"
                            model: ["normal", "low", "critical"]
                            onActivated: index => page.model.simSet("urgency", model[index])
                            Accessible.name: "Simulated urgency"
                        }
                        QQC2.CheckBox {
                            Kirigami.FormData.label: "Timeout:"
                            text: "the app asked for no timeout"
                            onToggled: page.model.simSet("timeout", checked ? "never" : "default")
                        }
                        QQC2.CheckBox {
                            Kirigami.FormData.label: "Do Not Disturb:"
                            text: "ignore it, mutes and full screen right now"
                            checked: true
                            onToggled: page.model.simSet("ignoreState", checked ? "true" : "false")
                        }

                        Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Result" }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing
                            QQC2.Label {
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                visible: page.sim.error !== undefined
                                color: Kirigami.Theme.negativeTextColor
                                text: page.sim.error !== undefined ? "Cannot simulate: " + page.sim.error : ""
                            }
                            QQC2.Label { visible: page.sim.zone !== undefined; text: "Zone: " + page.sim.zone + "   (" + page.sim.zoneBy + ")"; font.bold: true }
                            QQC2.Label { visible: page.sim.timeout !== undefined; text: "Timeout: " + page.sim.timeout + "   (" + page.sim.timeoutBy + ")" }
                            QQC2.Label { visible: page.sim.outcome !== undefined; text: "Outcome: " + page.sim.outcome }
                            QQC2.Label {
                                visible: page.sim.look !== undefined && page.sim.look.length > 0
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                text: page.sim.look ? "Look: " + page.sim.look.join(", ") : ""
                            }
                            QQC2.Label {
                                visible: page.sim.matched !== undefined
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                opacity: 0.7
                                text: page.sim.matched && page.sim.matched.length ? "Matching rules: " + page.sim.matched.join(", ") : "No rule matches; the defaults apply."
                            }
                        }
                    }
                }
            }
        }
    }
}
