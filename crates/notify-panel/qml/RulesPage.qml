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
    signal openTab(string name)
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
    Component.onCompleted: { sync(); refreshSuggestions() }

    function addFromTemplate(t) {
        const i = page.model.ruleAdd()
        page.selected = i
        page.model.ruleSetName(i, t.name)
        for (const f in t.match) page.model.ruleSetMatch(i, f, t.match[f])
        if (t.zone) page.model.ruleSetZone(i, t.zone === "$first" ? (page.zones[0] || "") : t.zone)
        if (t.timeout !== undefined) page.model.ruleSetTimeout(i, t.timeout)
        if (t.silent) page.model.ruleSetSilent(i, true)
    }

    readonly property var zoneObjs: { try { return JSON.parse(page.model ? page.model.settingsJson : "{}").zones || [] } catch (e) { return [] } }
    function zoneInfo(id) { return page.zoneObjs.find(z => z.id === id) || null }
    function zoneLabel(id) { const z = page.zoneInfo(id); return z && z.label ? z.label : id }
    readonly property var zoneChoiceLabels: page.zoneChoices.map((z, i) => i === 0 ? z : page.zoneLabel(z))
    function zoneAccent(id) { const z = page.zoneInfo(id); return z && z.accent ? z.accent : "" }
    readonly property var zones: {
        try { return JSON.parse(page.model ? page.model.draftZonesJson : "[]") } catch (e) { return [] }
    }
    readonly property var zoneChoices: {
        const z = ["(leave to later rules)"].concat(page.zones)
        if (page.current && page.current.zone !== "" && z.indexOf(page.current.zone) < 0) z.push(page.current.zone)
        return z
    }
    RuleData { id: data }
    readonly property var templates: data.templates
    // the look section opens by itself when the rule already changes something
    property bool lookOpen: Qt.application.arguments.indexOf("--open-details") >= 0
    property bool simOpen: false
    function hasLook(r) { return !!r && [r.sOpacity, r.sClickThrough, r.sNoFocus, r.sWidth, r.sFontFamily, r.sFontSize, r.sGroup, r.sPriority, r.sMedia, r.sStack, r.sTimer].some(v => v !== undefined && v !== null && v !== "") }
    onCurrentChanged: lookOpen = hasLook(current)
    property var opened: ({})                // conditions added by hand for the selected rule
    onSelectedChanged: opened = ({})
    readonly property var hiddenFields: data.matchFields.filter(f => !(page.current && page.current[f.role] !== "") && page.opened[f.field] !== true)

    // values seen in history and the app list, offered as tags in the match fields
    property var suggest: ({})
    property var recent: []
    function refreshSuggestions() {
        try { page.suggest = JSON.parse(page.model.ruleSuggestions()) } catch (e) { page.suggest = ({}) }
        try { page.recent = JSON.parse(page.model.recentNotifications()) } catch (e) { page.recent = [] }
    }
    Connections { target: page.model; function onAppsJsonChanged() { page.refreshSuggestions() } function onHistoryTotalChanged() { page.refreshSuggestions() } }

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
            columns: page.width > Kirigami.Units.gridUnit * 40 ? 3 : 1   // wide: list | divider | editor; narrow: stacked
            columnSpacing: 0
            rowSpacing: 0

            // ================= left: the ordered list of rules =================
            RuleList {
                page: page
                rules: rules
                columns: parent.columns === 3 ? 2 : 1
            }
            Kirigami.Separator { Layout.fillHeight: true; visible: parent.columns === 3 }

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
                    // the selected rule: every field has its label above it, so the editor fits any pane width
                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.preferredWidth: 1
                        Layout.margins: Kirigami.Units.largeSpacing
                        enabled: page.current !== null
                        spacing: Kirigami.Units.smallSpacing

                        QQC2.Label { text: "Name"; font.bold: true }
                        QQC2.TextField {
                            id: nameField
                            Layout.fillWidth: true
                            placeholderText: "(unnamed rule)"
                            Binding on text { when: !nameField.activeFocus; value: page.current ? page.current.name : "" }
                            onTextEdited: page.model.ruleSetName(page.selected, text)
                            Accessible.name: "Rule name"
                        }

                        Kirigami.Heading { Layout.topMargin: Kirigami.Units.largeSpacing; level: 3; text: "When a notification matches" }
                        QQC2.Label {
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                            opacity: 0.7
                            font: Kirigami.Theme.smallFont
                            text: "Every condition must match; any one tag within a condition is enough. Pick from what you have received or type your own and press Enter."
                        }
                        Repeater {
                            model: data.matchFields
                            delegate: ColumnLayout {
                                id: cond
                                required property var modelData
                                readonly property bool shown: field.value !== "" || page.opened[modelData.field] === true
                                visible: shown
                                Layout.fillWidth: true
                                spacing: 2
                                QQC2.Label { text: cond.modelData.label.replace(":", ""); font.bold: true }
                                MatchTags {
                                    id: field
                                    Layout.fillWidth: true
                                    label: cond.modelData.label
                                    kind: cond.modelData.kind
                                    placeholder: cond.modelData.hint
                                    suggestions: page.suggest[cond.modelData.field] || []
                                    value: page.current ? page.current[cond.modelData.role] : ""
                                    onEdited: text => page.model.ruleSetMatch(page.selected, cond.modelData.field, text)
                                }
                            }
                        }
                        QQC2.Button {
                            visible: page.hiddenFields.length > 0
                            text: page.hiddenFields.length === data.matchFields.length ? "Add a condition…" : "Add another condition…"
                            icon.name: "list-add"
                            onClicked: fieldMenu.popup()
                            QQC2.Menu {
                                id: fieldMenu
                                Repeater {
                                    model: page.hiddenFields
                                    delegate: QQC2.MenuItem {
                                        required property var modelData
                                        text: modelData.menu
                                        onTriggered: { const o = Object.assign({}, page.opened); o[modelData.field] = true; page.opened = o }
                                    }
                                }
                            }
                        }
                        QQC2.Label {
                            Layout.fillWidth: true
                            visible: page.hiddenFields.length === data.matchFields.length
                            wrapMode: Text.Wrap
                            opacity: 0.7
                            font: Kirigami.Theme.smallFont
                            text: "With no conditions the rule matches every notification."
                        }

                        RulePreview { Layout.fillWidth: true; Layout.topMargin: Kirigami.Units.smallSpacing; rule: page.current; recent: page.recent }

                        Kirigami.Heading { Layout.topMargin: Kirigami.Units.largeSpacing; level: 3; text: "Then" }
                        QQC2.Label { text: "Show in zone"; font.bold: true }
                        RowLayout {
                          Layout.fillWidth: true
                          spacing: Kirigami.Units.largeSpacing
                          MiniScreen {
                            readonly property var zn: page.current ? page.zoneInfo(page.current.zone) : null
                            dim: zn === null
                            anchorName: zn ? zn.anchor : "top-right"
                            fx: zn ? Number(zn.x) : 0.5; fy: zn ? Number(zn.y) : 0.1
                            accent: zn && zn.accent ? zn.accent : ""
                            Accessible.name: "Where this rule puts popups"
                          }
                          QQC2.ComboBox {
                            id: zoneBox
                            Layout.fillWidth: true
                            model: page.zoneChoiceLabels
                            currentIndex: page.current && page.current.zone !== "" ? Math.max(0, page.zoneChoices.indexOf(page.current.zone)) : 0
                            onActivated: index => page.model.ruleSetZone(page.selected, index === 0 ? "" : page.zoneChoices[index])
                            Accessible.name: "Show in zone"
                          }
                        }
                        QQC2.Label { Layout.topMargin: Kirigami.Units.smallSpacing; text: "Timeout (ms)"; font.bold: true }
                        QQC2.TextField {
                            id: timeoutField
                            Layout.fillWidth: true
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
                            font: Kirigami.Theme.smallFont
                            text: "0 keeps the popup until you dismiss it."
                        }
                        QQC2.CheckBox {
                            Layout.topMargin: Kirigami.Units.smallSpacing
                            text: "Never show it, keep it in history only"
                            checked: page.current ? page.current.silent : false
                            onToggled: {
                                page.model.ruleSetSilent(page.selected, checked)
                                checked = Qt.binding(() => page.current ? page.current.silent : false)
                            }
                            Accessible.name: "Never show, keep in history only"
                        }
                    }

                    QQC2.Button {
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        icon.name: page.lookOpen ? "go-up" : "go-down"
                        text: page.lookOpen ? "Hide look and behavior" : "Change how it looks and behaves…"
                        onClicked: page.lookOpen = !page.lookOpen
                        Accessible.name: "Look and behavior of this rule"
                    }
                    StyleFields {
                        visible: page.lookOpen
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        enabled: page.current !== null
                        inheritFrom: "later rules, the zone and the theme"
                        model: page.model
                        accent: page.zoneAccent(page.current ? page.current.zone : "")
                        timeoutMs: page.current && page.current.timeout !== "" ? Number(page.current.timeout) : -1
                        values: page.current ? ({ opacity: page.current.sOpacity, clickThrough: page.current.sClickThrough, noFocus: page.current.sNoFocus, width: page.current.sWidth,
                                                  fontFamily: page.current.sFontFamily, fontSize: page.current.sFontSize, group: page.current.sGroup,
                                                  priority: page.current.sPriority, media: page.current.sMedia, stack: page.current.sStack, timerStyle: page.current.sTimer }) : ({})
                        onEdited: (key, text) => page.model.ruleSetStyle(page.selected, key, text)
                    }

                    Kirigami.Separator { Layout.fillWidth: true }

                    // ---- the simulator
                    QQC2.Button {
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        icon.name: "system-search"
                        text: page.simOpen ? "Hide the tester" : "Test the rules with a sample notification…"
                        onClicked: page.simOpen = !page.simOpen
                        Accessible.name: "Test the rules"
                    }
                    RulesSimulator {
                        visible: page.simOpen
                        Layout.fillWidth: true
                        Layout.margins: Kirigami.Units.largeSpacing
                        model: page.model
                        result: page.sim
                    }
                }
            }
        }
    }
}
