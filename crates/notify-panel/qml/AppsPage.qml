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

    ListModel { id: rows }
    readonly property var zoneNames: { try { return JSON.parse(page.model ? page.model.draftZonesJson : "[]") } catch (e) { return [] } }
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
            appId: a.id, name: a.name, iconName: a.icon, sent: a.sent, count: a.count, last: a.last, capable: a.capable,
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
    }
    onFilterChanged: sync()
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
                model: ["All apps", "Sending (" + page.totalSending + ")", "Could send, never has (" + page.totalSilent + ")"]
                onActivated: index => page.filter = ["all", "sending", "silent"][index]
                Accessible.name: "Which apps to show"
            }
        }
        QQC2.Label {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            wrapMode: Text.Wrap
            opacity: 0.7
            text: "Choose where each app's popups appear, or mute it. Use the arrow for more. Changes are saved as rules (named “App: …” on the Rules tab)."
        }

        Kirigami.AbstractCard {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.smallSpacing
            visible: page.showWays
            contentItem: ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
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
        QQC2.ToolButton {
            Layout.leftMargin: Kirigami.Units.largeSpacing
            text: page.showWays ? "Hide" : "What can a notification do?"
            icon.name: "help-about"
            onClicked: page.showWays = !page.showWays
        }

        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: rows
            spacing: Kirigami.Units.smallSpacing
            delegate: Kirigami.AbstractCard {
                id: card
                required property int index
                required property string appId
                required property string name
                required property string iconName
                required property bool sent
                required property int count
                required property double last
                required property bool capable
                required property string reasons
                required property bool flatpak
                required property int perm
                required property string matchField
                required property string matchValue
                required property bool muted
                required property string zone
                required property string timeout
                required property bool hasRule
                required property string appStyleJson
                required property string kindsJson
                required property int kindCount
                readonly property var appStyle: JSON.parse(appStyleJson)
                readonly property var kinds: JSON.parse(kindsJson)
                property bool detailsOpen: page.openAll
                property bool lookOpen: page.openAll
                property bool kindsOpen: page.openAll
                width: ListView.view.width - Kirigami.Units.largeSpacing * 2
                x: Kirigami.Units.largeSpacing
                contentItem: ColumnLayout {
                    spacing: Kirigami.Units.smallSpacing
                    RowLayout {
                        spacing: Kirigami.Units.smallSpacing
                        Kirigami.Icon {
                            source: card.iconName !== "" ? card.iconName : "application-x-executable"
                            fallback: "application-x-executable"
                            Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                            Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0
                            QQC2.Label { Layout.fillWidth: true; font.bold: true; elide: Text.ElideRight; text: card.name; textFormat: Text.PlainText }
                            QQC2.Label {
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                opacity: 0.7
                                textFormat: Text.PlainText
                                text: card.sent
                                    ? card.count + (card.count === 1 ? " notification" : " notifications") + " · " + page.ago(card.last)
                                    : "Never sent" + (card.reasons !== "" ? " · " + card.reasons : "")
                            }
                        }
                        QQC2.ComboBox {
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 8
                            model: page.zoneChoices
                            displayText: card.zone === "" ? "Zone: by rules" : "Zone: " + card.zone
                            currentIndex: card.zone === "" ? 0 : Math.max(0, page.zoneChoices.indexOf(card.zone))
                            onActivated: index => page.model.appSet(card.matchField, card.matchValue, card.name, "zone", index === 0 ? "" : page.zoneChoices[index])
                            Accessible.name: "Zone for " + card.name
                        }
                        QQC2.Switch {
                            text: "Mute"
                            checked: card.muted
                            onToggled: {
                                page.model.appSet(card.matchField, card.matchValue, card.name, "muted", checked ? "true" : "false")
                                checked = Qt.binding(() => card.muted)
                            }
                            Accessible.name: "Mute " + card.name
                        }
                        QQC2.ToolButton {
                            icon.name: card.detailsOpen ? "go-up" : "go-down"
                            display: QQC2.AbstractButton.IconOnly
                            onClicked: card.detailsOpen = !card.detailsOpen
                            QQC2.ToolTip.visible: hovered
                            QQC2.ToolTip.text: card.detailsOpen ? "Hide details" : "Timeout, look and notification types (" + card.kindCount + ")"
                            Accessible.name: "Details for " + card.name
                        }
                    }
                    ColumnLayout {
                        visible: card.detailsOpen
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.largeSpacing

                    GridLayout {
                            columns: 4
                            columnSpacing: Kirigami.Units.largeSpacing
                            rowSpacing: Kirigami.Units.smallSpacing
                            QQC2.Label { text: "Hide after (ms):" }
                            QQC2.TextField {
                                id: timeoutField
                                Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                                placeholderText: "default"
                                inputMethodHints: Qt.ImhDigitsOnly
                                validator: IntValidator { bottom: 0 }
                                Binding on text { when: !timeoutField.activeFocus; value: card.timeout }
                                onTextEdited: page.model.appSet(card.matchField, card.matchValue, card.name, "timeout", text)
                                Accessible.name: "Timeout for " + card.name
                            }
                            QQC2.Label { visible: card.flatpak; text: "Flatpak permission:" }
                            QQC2.ComboBox {
                                visible: card.flatpak
                                Layout.preferredWidth: Kirigami.Units.gridUnit * 9
                                model: ["Not set", "Allowed", "Blocked"]
                                currentIndex: card.perm
                                onActivated: index => page.model.appFlatpak(card.appId, ["reset", "allow", "deny"][index])
                                Accessible.name: "Flatpak notification permission for " + card.name
                            }
                        }

                        QQC2.Button {
                            Layout.alignment: Qt.AlignLeft
                            flat: true
                            icon.name: card.lookOpen ? "go-up" : "go-down"
                            text: card.lookOpen ? "Hide look and behavior" : "Change how its popups look and behave…"
                            onClicked: card.lookOpen = !card.lookOpen
                            Accessible.name: "Look and behavior of " + card.name
                        }
                        StyleFields {
                            visible: card.lookOpen
                            Layout.fillWidth: true
                            showGroup: true
                            inheritFrom: "its zone and the theme"
                            values: card.appStyle
                            onEdited: (key, text) => page.model.appSet(card.matchField, card.matchValue, card.name, key, text)
                        }

                        QQC2.Button {
                            Layout.alignment: Qt.AlignLeft
                            flat: true
                            icon.name: card.kindsOpen ? "go-up" : "go-down"
                            text: card.kindsOpen ? "Hide notification types" : "Treat its notification types separately (" + card.kindCount + ")…"
                            onClicked: card.kindsOpen = !card.kindsOpen
                            Accessible.name: "Notification types of " + card.name
                        }
                        QQC2.Label {
                            visible: card.kindsOpen
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                            opacity: 0.7
                            text: card.kindCount > 0
                                ? "Each type can go to its own zone, be muted, be grouped, or look different from the app's other notifications."
                                : "None yet. A kind (for example email.arrived or a web page's notification) is listed as soon as the app sends one."
                        }
                        Repeater {
                            model: card.kindsOpen ? card.kindCount : 0
                            delegate: Kirigami.AbstractCard {
                                id: kindCard
                                required property int index
                                readonly property var kind: card.kinds[index] !== undefined ? card.kinds[index] : ({ routing: { muted: false, zone: "", style: {} }, caps: [], category: "", channel: "app", count: 0, matches: [], key: "" })
                                property bool lookOpen: page.openAll
                                Layout.fillWidth: true
                                contentItem: ColumnLayout {
                                    spacing: Kirigami.Units.smallSpacing
                                    RowLayout {
                                        QQC2.Label {
                                            Layout.fillWidth: true
                                            font.bold: true
                                            textFormat: Text.PlainText
                                            elide: Text.ElideRight
                                            text: (kindCard.kind.category !== "" ? kindCard.kind.category : "General notifications") + "  ·  " + page.channelName(kindCard.kind.channel)
                                        }
                                        QQC2.Switch {
                                            text: "Mute"
                                            checked: kindCard.kind.routing.muted
                                            onToggled: {
                                                page.model.appSetKind(JSON.stringify(kindCard.kind.matches), card.name + ": " + (kindCard.kind.category || "general") + " " + kindCard.kind.channel, "muted", checked ? "true" : "false")
                                                checked = Qt.binding(() => kindCard.kind.routing.muted)
                                            }
                                            Accessible.name: "Mute this kind"
                                        }
                                    }
                                    QQC2.Label {
                                        Layout.fillWidth: true
                                        wrapMode: Text.Wrap
                                        opacity: 0.7
                                        textFormat: Text.PlainText
                                        text: "Sent " + kindCard.kind.count + "×. Can do: " + (kindCard.kind.caps.length ? kindCard.kind.caps.join(", ") : "plain text only")
                                    }
                                    RowLayout {
                                        QQC2.Label { text: "Show in:" }
                                        QQC2.ComboBox {
                                            Layout.preferredWidth: Kirigami.Units.gridUnit * 9
                                            model: page.zoneChoices
                                            currentIndex: kindCard.kind.routing.zone === "" ? 0 : Math.max(0, page.zoneChoices.indexOf(kindCard.kind.routing.zone))
                                            onActivated: i => page.model.appSetKind(JSON.stringify(kindCard.kind.matches), card.name + ": " + (kindCard.kind.category || "general") + " " + kindCard.kind.channel, "zone", i === 0 ? "" : page.zoneChoices[i])
                                            Accessible.name: "Zone for this kind"
                                        }
                                        Item { Layout.fillWidth: true }
                                        QQC2.Button {
                                            icon.name: kindCard.lookOpen ? "go-up" : "go-down"
                                            text: kindCard.lookOpen ? "Hide look" : "Look, group…"
                                            onClicked: kindCard.lookOpen = !kindCard.lookOpen
                                        }
                                    }
                                    StyleFields {
                                        visible: kindCard.lookOpen
                                        Layout.fillWidth: true
                                        showGroup: true
                                        inheritFrom: "the app's settings"
                                        values: kindCard.kind.routing.style
                                        onEdited: (key, text) => page.model.appSetKind(JSON.stringify(kindCard.kind.matches), card.name + ": " + (kindCard.kind.category || "general") + " " + kindCard.kind.channel, key, text)
                                    }
                                }
                            }
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
}
