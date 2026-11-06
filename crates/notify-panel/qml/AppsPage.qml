import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Every app that sends notifications, and every installed app that could but never has. Per app: mute it, send it to a zone,
// change how long its popups stay (all saved as a rule), and for Flatpak apps change the app's own notification permission.
Kirigami.Page {
    id: page
    title: "Apps"
    property var model
    property var ui
    signal openTab(string name)
    readonly property bool online: model !== undefined && model !== null && model.connected === true

    property bool showWays: false
    // test/screenshot hook: --open-details expands every app and every kind
    readonly property bool openAll: Qt.application.arguments.indexOf("--open-details") >= 0
    property string filter: "all"          // all | sending | silent
    property string query: ""

    // grid | list | compact; --apps-view=NAME picks one at start (screenshots and tests)
    property string view: { const a = Qt.application.arguments.filter(x => x.indexOf("--apps-view=") === 0); return a.length ? a[0].substring(12) : "list" }
    property string selectedApp: ""         // the app whose settings the grid view shows in a window
    property var groups: []                 // [{name, open, count, sending, apps}] in display order (the grid reads this)
    property var collapsed: ({})            // category name -> true/false once the user opened or closed it
    readonly property var groupOrder: ["Messaging and email", "Internet", "Office", "Development", "Media", "Graphics", "Games", "Education and science", "System", "Utilities", "Other"]
    ListModel { id: rows }                  // the apps only (what the tests read)
    ListModel { id: listRows }              // the list views: a heading row for each category, then its apps

    function toggleGroup(name) {
        const o = Object.assign({}, page.collapsed)
        const g = page.groups.find(x => x.name === name)
        o[name] = g ? !g.open : false
        page.collapsed = o
        page.sync()
    }
    function asApp(m) {
        return {
            appId: m.appId, name: m.name, iconName: m.iconName, sent: m.sent, count: m.count, last: m.last, reasons: m.reasons,
            flatpak: m.flatpak, perm: m.perm, matchField: m.matchField, matchValue: m.matchValue, muted: m.muted, zone: m.zone,
            timeout: m.timeout, kindCount: m.kindCount, appStyle: JSON.parse(m.appStyleJson), kinds: JSON.parse(m.kindsJson)
        }
    }
    function appById(id) {
        for (let i = 0; i < rows.count; i++) if (rows.get(i).appId === id) return page.asApp(rows.get(i))
        return null
    }
    readonly property var zoneNames: { try { return JSON.parse(page.model ? page.model.draftZonesJson : "[]") } catch (e) { return [] } }
    readonly property var zoneObjs: { try { return JSON.parse(page.model ? page.model.settingsJson : "{}").zones || [] } catch (e) { return [] } }
    function zoneInfo(id) { return page.zoneObjs.find(z => z.id === id) || null }
    function zoneAccent(id) { const z = page.zoneInfo(id); return z && z.accent ? z.accent : "" }
    readonly property var zoneChoices: ["(by rules)"].concat(page.zoneNames)

    function ago(ms) {
        if (!ms) return "never"
        const s = Math.max(0, (Date.now() - ms) / 1000)
        if (s < 90) return "just now"
        if (s < 5400) return Math.round(s / 60) + " min ago"
        if (s < 129600) return Math.round(s / 3600) + " h ago"
        return Math.round(s / 86400) + " days ago"
    }
    function reasonText(reasons) {
        const names = { declared: "says it uses notifications", category: "a messaging or email app", kde: "a KDE app with notification events", plasma: "Plasma has seen it notify", flatpak: "Flatpak notification permission" }
        return reasons.map(r => names[r] || r).join("; ")
    }
    function flat(a) {
        return {
            appId: a.id, group: a.group || "Other", name: a.name, iconName: a.icon, sent: a.sent, count: a.count, last: a.last, capable: a.capable,
            reasons: page.reasonText(a.reasons), flatpak: a.flatpak,
            perm: a.flatpak_permission === true ? 1 : a.flatpak_permission === false ? 2 : 0,
            matchField: a.match_field, matchValue: a.match_value,
            muted: a.routing.muted, zone: a.routing.zone, timeout: a.routing.timeout, hasRule: a.routing.exists,
            appStyleJson: JSON.stringify(a.routing.style), kindsJson: JSON.stringify(a.kinds), kindCount: a.kinds.length
        }
    }
    function channelName(c) {
        return c === "web" ? "website (browser push)" : c === "portal" ? "sandboxed app (portal)" : "app (D-Bus)"
    }
    function shown(a) {
        if (page.filter === "sending" && !a.sent) return false
        if (page.filter === "silent" && a.sent) return false
        const q = page.query.trim().toLowerCase()
        return q === "" || a.name.toLowerCase().indexOf(q) >= 0 || a.id.toLowerCase().indexOf(q) >= 0
    }
    property int totalSending: 0
    property int totalSilent: 0
    function sync() {
        if (!page.model) return
        let list = []
        try { list = JSON.parse(page.model.appsJson) } catch (e) { return }
        let sending = 0, silent = 0
        const visible = []
        for (const a of list) {
            if (a.sent) sending++; else silent++
            if (page.shown(a)) visible.push(page.flat(a))
        }
        page.totalSending = sending
        page.totalSilent = silent
        // update in place when the same apps are listed in the same order (typing in a row must not lose focus)
        let same = visible.length === rows.count
        for (let i = 0; same && i < visible.length; i++) if (rows.get(i).appId !== visible[i].appId) same = false
        if (same) { for (let i = 0; i < visible.length; i++) rows.set(i, visible[i]) }
        else { rows.clear(); for (const v of visible) rows.append(v) }
        page.buildGroups(visible)
    }
    // categories in a fixed order; a category opens by itself when it holds an app that has sent something, or while searching
    function buildGroups(visible) {
        const by = {}
        for (const v of visible) (by[v.group || "Other"] = by[v.group || "Other"] || []).push(v)
        const names = page.groupOrder.filter(n => by[n]).concat(Object.keys(by).filter(n => page.groupOrder.indexOf(n) < 0))
        const gs = names.map(n => {
            const apps = by[n]
            const sending = apps.filter(a => a.sent).length
            const auto = sending > 0 || page.query.trim() !== "" || page.filter === "silent"
            return { name: n, count: apps.length, sending: sending, open: page.collapsed[n] !== undefined ? page.collapsed[n] : auto, apps: apps }
        })
        page.groups = gs
        const items = []
        for (const g of gs) {
            items.push(Object.assign({}, page.blank, { kind: "header", group: g.name, open: g.open, count: g.count, sending: g.sending, appId: "#" + g.name }))
            if (g.open) for (const a of g.apps) items.push(Object.assign({}, a, { kind: "app", open: false, sending: 0 }))
        }
        let same = items.length === listRows.count
        for (let i = 0; same && i < items.length; i++) if (listRows.get(i).appId !== items[i].appId) same = false
        if (same) { for (let i = 0; i < items.length; i++) listRows.set(i, items[i]) }
        else { listRows.clear(); for (const it of items) listRows.append(it) }
    }
    // every field a row has, empty: header rows use it so all rows share the same roles
    readonly property var blank: ({ appId: "", group: "", name: "", iconName: "", sent: false, count: 0, last: 0, capable: false, reasons: "", flatpak: false, perm: 0,
        matchField: "", matchValue: "", muted: false, zone: "", timeout: "", hasRule: false, appStyleJson: "{}", kindsJson: "[]", kindCount: 0,
        kind: "app", open: false, sending: 0 })
    onFilterChanged: { collapsed = ({}); sync() }
    onQueryChanged: sync()
    Connections { target: page.model; function onAppsJsonChanged() { page.sync() } }
    Connections { target: page.model; function onConnectedChanged() { if (page.online) page.model.appsRescan() } }
    Component.onCompleted: { sync(); if (online) model.appsRescan() }

    readonly property var uiState: ({
        page: "apps",
        rows: rows.count,
        sending: page.totalSending,
        silent: page.totalSilent,
        filter: page.filter,
        scanning: page.model ? page.model.appsScanning : false,
        names: (function () { const n = []; for (let i = 0; i < Math.min(rows.count, 40); i++) n.push(rows.get(i).name); return n })(),
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
            text: "Rescan"; icon.name: "view-refresh"
            enabled: page.online && !page.model.appsScanning
            onTriggered: page.model.appsRescan()
        }
    ]

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        DraftBanners { id: banners; Layout.fillWidth: true; model: page.model }

        RowLayout {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.smallSpacing
            spacing: Kirigami.Units.smallSpacing
            Kirigami.SearchField {
                Layout.fillWidth: true
                placeholderText: "Find an app"
                onTextChanged: page.query = text
                Accessible.name: "Find an app"
            }
            QQC2.ComboBox {
                model: ["All apps", "Sending (" + page.totalSending + ")", "Never sent (" + page.totalSilent + ")"]
                onActivated: index => page.filter = ["all", "sending", "silent"][index]
                Accessible.name: "Which apps to show"
            }
            // how the apps are laid out
            RowLayout {
                spacing: 0
                Repeater {
                    model: [{ id: "grid", icon: "view-list-icons", tip: "Grid" }, { id: "list", icon: "view-list-details", tip: "List" }, { id: "compact", icon: "view-list-text", tip: "List (compact)" }]
                    delegate: QQC2.ToolButton {
                        required property var modelData
                        icon.name: modelData.icon
                        display: QQC2.AbstractButton.IconOnly
                        checkable: true; autoExclusive: true
                        checked: page.view === modelData.id
                        onClicked: page.view = modelData.id
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.text: modelData.tip
                        Accessible.name: "View as " + modelData.tip
                    }
                }
            }
            QQC2.ToolButton {
                icon.name: "help-about"
                display: QQC2.AbstractButton.IconOnly
                checkable: true
                checked: page.showWays
                onClicked: page.showWays = !page.showWays
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: "How this page works, and what a notification can do"
                Accessible.name: "Help"
            }
        }

        Kirigami.AbstractCard {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.smallSpacing
            visible: page.showWays
            contentItem: ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
                QQC2.Label { font.bold: true; text: "How this page works" }
                QQC2.Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    text: "Pick where an app's popups go, or mute it. Open an app for its timeout, look and notification types. Changes are saved as rules named “App: …” (see the Rules section). Apps are grouped by their system category."
                }
                QQC2.Label { font.bold: true; text: "Ways a notification can arrive" }
                QQC2.Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    text: "• App (D-Bus): normal desktop apps through libnotify, KDE's KNotification or Electron.\n• Sandboxed app (portal): Flatpak and Snap apps, through the desktop portal.\n• Website (browser push): pages that use the web Notifications or Push API in Brave, Chrome or Firefox."
                }
                QQC2.Label { font.bold: true; text: "What a notification can contain or do" }
                QQC2.Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    text: "title and text (with simple bold, italic and underline), an app icon or an image, action buttons, an inline reply box, a progress bar, urgency (low, normal, critical), a sound, a kind (category, such as email.arrived), 'stays until dismissed' (resident) and 'do not keep in history' (transient). Each app's list below shows what it has actually used."
                }
            }
        }
        // ---- the apps: grouped by system category, shown as a list, a compact list or a grid
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ListView {
                id: list
                anchors.fill: parent
                visible: page.view !== "grid"
                clip: true
                model: listRows
                spacing: page.view === "compact" ? 2 : Kirigami.Units.smallSpacing
                delegate: Loader {
                    id: ld
                    required property var model
                    width: ListView.view.width
                    sourceComponent: model.kind === "header" ? headerComp : rowComp
                    Component {
                        id: headerComp
                        GroupHeader { group: ({ name: ld.model.group, open: ld.model.open, count: ld.model.count, sending: ld.model.sending }) }
                    }

                    Component {
                        id: rowComp
                        Kirigami.AbstractCard {
                            id: card
                            readonly property var m: ld.model
                            property bool detailsOpen: page.openAll
                            readonly property var asApp: page.asApp(m)
                        topPadding: page.view === "compact" ? 2 : Kirigami.Units.largeSpacing
                        bottomPadding: topPadding
                            x: Kirigami.Units.smallSpacing
                            width: ld.width - Kirigami.Units.smallSpacing * 2
                            contentItem: ColumnLayout {
                                spacing: Kirigami.Units.smallSpacing
                                AppHeader {
                                    Layout.fillWidth: true
                                    host: page
                                    app: card.asApp
                                    compact: page.view === "compact"
                                    detailsOpen: card.detailsOpen
                                    onToggleDetails: card.detailsOpen = !card.detailsOpen
                                }
                                AppDetails {
                                    visible: card.detailsOpen
                                    Layout.fillWidth: true
                                    host: page
                                    app: card.asApp
                                }
                            }
                        }
                    }
                }
            }

            ListView {
                id: gridList
                anchors.fill: parent
                visible: page.view === "grid"
                clip: true
                model: page.view === "grid" ? page.groups : []
                spacing: Kirigami.Units.smallSpacing
                delegate: ColumnLayout {
                    id: grp
                    required property var modelData
                    width: ListView.view.width
                    spacing: Kirigami.Units.smallSpacing
                    GroupHeader { group: grp.modelData }
                    Flow {
                        visible: grp.modelData.open
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.smallSpacing
                        Layout.rightMargin: Kirigami.Units.smallSpacing
                        spacing: Kirigami.Units.smallSpacing
                        Repeater {
                            model: grp.modelData.open ? grp.modelData.apps : []
                            delegate: AppTile { host: page; app: modelData; onOpened: page.selectedApp = modelData.appId }
                        }
                    }
                }
            }

            Kirigami.PlaceholderMessage {
                anchors.centerIn: parent
                width: parent.width - Kirigami.Units.gridUnit * 4
                visible: rows.count === 0
                icon.name: page.online ? "edit-find" : "network-disconnect"
                text: !page.online ? "notifd is not running" : page.model && page.model.appsScanning ? "Looking for apps…" : "No apps found"
                explanation: page.online ? "Apps appear here once they send a notification, or when they are installed and able to." : ""
            }
        }
    }

    // a category heading that opens and closes its apps
    component GroupHeader: QQC2.ItemDelegate {
        id: gh
        property var group
        width: parent ? parent.width : implicitWidth
        onClicked: page.toggleGroup(gh.group.name)
        contentItem: RowLayout {
            Kirigami.Icon { source: gh.group.open ? "arrow-down" : "arrow-right"; Layout.preferredWidth: Kirigami.Units.iconSizes.small; Layout.preferredHeight: Layout.preferredWidth }
            QQC2.Label { font.bold: true; text: gh.group.name }
            QQC2.Label { opacity: 0.7; text: gh.group.count + (gh.group.count === 1 ? " app" : " apps") + (gh.group.sending > 0 ? " · " + gh.group.sending + " sending" : "") }
            Item { Layout.fillWidth: true }
        }
        Accessible.name: (gh.group.open ? "Collapse " : "Expand ") + gh.group.name
    }

    // grid mode: the details of the chosen app in a window of their own
    QQC2.Dialog {
        id: details
        parent: QQC2.Overlay.overlay
        anchors.centerIn: parent
        modal: true
        width: Math.min(parent.width - Kirigami.Units.gridUnit * 2, Kirigami.Units.gridUnit * 34)
        height: Math.min(parent.height - Kirigami.Units.gridUnit * 2, Kirigami.Units.gridUnit * 38)
        visible: page.selectedApp !== "" && page.view === "grid"
        onClosed: page.selectedApp = ""
        standardButtons: QQC2.Dialog.Close
        readonly property var app: page.appById(page.selectedApp)
        contentItem: Loader {
            active: details.visible && details.app !== null
            sourceComponent: QQC2.ScrollView {
                contentWidth: availableWidth
                ColumnLayout {
                    width: parent.width
                    spacing: Kirigami.Units.largeSpacing
                    AppHeader { Layout.fillWidth: true; host: page; app: details.app; showArrow: false }
                    AppDetails { Layout.fillWidth: true; host: page; app: details.app }
                }
            }
        }
    }
}
