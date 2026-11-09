pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland

// The notification center: recent notifications by group, opened from the tray icon (or `notify center`), in the corner set by
// center.anchor. Each group is listed under its own heading in its colour, and folds into a stack with a click on the heading.
// "Mark all read" and "Clear all" act on the whole history. Escape, or a click anywhere else, closes it.
Scope {
    id: center
    required property var shell          // centerOpen, cfg, history, api

    readonly property bool open: shell.centerOpen
    readonly property var cfg: shell.cfg
    readonly property var theme: cfg.theme
    readonly property string corner: (cfg.center && cfg.center.anchor) || "bottom-right"
    readonly property var screenObj: shell.screens.popups
    property var folded: ({})            // group -> true while it is folded into a stack
    property real now: Date.now()
    Timer { interval: 30000; repeat: true; running: center.open; triggeredOnStart: true; onTriggered: center.now = Date.now() }
    onOpenChanged: { if (!open) folded = ({}); list.currentIndex = -1; if (open) panel.forceActiveFocus() }

    function close() { shell.api.center("close") }
    // the keyboard (see CenterNav.qml): move the selection, or act on the selected row
    readonly property CenterNav nav: CenterNav {}
    function key(event) {
        if (!panel.activeFocus) return       // Tab is on a header button: keys belong to it, not to a selection you cannot see
        const d = nav.moveBy(event.key)
        if (d !== 0) {
            list.currentIndex = nav.step(list.count, list.currentIndex, d)
            list.positionViewAtIndex(list.currentIndex, ListView.Contain)
            event.accepted = true
            return
        }
        const i = list.currentIndex, row = i >= 0 ? rows[i] : null
        const card = list.currentItem && list.currentItem.item
        const a = nav.action(row, event.key)
        if (a.what === "") return
        event.accepted = true
        if (a.what === "fold") toggleFold(row.zone)
        else if (a.what === "open") open_(row.entry, card ? card.live : null)
        else if (a.what === "remove") shell.api.removeHistory(row.entry.id, row.entry.t)
        else if (a.what === "button" && card && a.n < card.buttons.length) press(row.entry.id, card.buttons[a.n].identifier)
    }
    // press a notification's button and close the list; its app may have closed it a moment ago (then just mark it read)
    function press(id, key) {
        try { shell.api.invoke(id, key) } catch (e) { shell.api.markRead([id]) }
        close()
    }
    // a click on a notification: what clicking its popup does (the app's default action) while the app still holds it; else just mark it read
    function open_(entry, live) {
        if (live && (live.actions || []).some(a => a.identifier === "default")) press(entry.id, "default")
        else shell.api.markRead([entry.id])
    }
    function accent(z) { return (cfg.zones[z] && cfg.zones[z].accent) || "#60a5fa" }
    function label(z) { return (cfg.zones[z] && cfg.zones[z].label) || z }
    function toggleFold(z) { const f = Object.assign({}, folded); if (f[z]) delete f[z]; else f[z] = true; folded = f }

    // what the list shows: a heading per group (in the config's order, then any group no longer configured), then its notifications
    readonly property var rows: {
        const by = {}
        for (const h of shell.history) (by[h.zone] = by[h.zone] || []).push(h)
        const order = Object.keys(cfg.zones).filter(z => by[z]).concat(Object.keys(by).filter(z => !(z in cfg.zones)))
        const out = []
        for (const z of order) {
            const list = by[z]
            out.push({ kind: "group", zone: z, count: list.length, unread: list.filter(h => h.read !== true).length })
            if (!folded[z]) for (const h of list) out.push({ kind: "item", zone: z, entry: h })
        }
        return out
    }
    readonly property int unread: shell.api.unread.all

    // a click outside the list closes it
    PanelWindow {
        visible: center.open
        screen: center.screenObj
        anchors { top: true; bottom: true; left: true; right: true }
        exclusionMode: ExclusionMode.Ignore
        WlrLayershell.layer: WlrLayer.Top
        WlrLayershell.namespace: "notifd-center-backdrop"
        color: "transparent"
        MouseArea { anchors.fill: parent; onClicked: center.close() }
    }

    PanelWindow {
        id: win
        visible: center.open
        screen: center.screenObj
        anchors { top: center.corner.startsWith("top"); bottom: center.corner.startsWith("bottom"); left: center.corner.endsWith("left"); right: center.corner.endsWith("right") }
        margins { top: 8; bottom: 8; left: 8; right: 8 }
        exclusionMode: ExclusionMode.Normal   // stays clear of the taskbar
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "notifd-center"
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
        color: "transparent"
        implicitWidth: 420
        implicitHeight: Math.min(head.height + 34 + list.contentHeight + (empty.visible ? empty.height : 0), (screen ? screen.height : 1080) * 0.7)

        Rectangle {
            anchors.fill: parent
            radius: center.theme.radius ?? 12
            color: center.theme.card ?? "#1d1f27"
            border.width: 1
            border.color: center.theme.border ?? "#2c2f3a"
            id: panel
            focus: true
            Keys.onEscapePressed: center.close()
            Keys.onPressed: event => center.key(event)

            Row {
                id: head
                x: 12; y: 12
                width: parent.width - 24
                height: 30
                spacing: 8
                Column {
                    width: parent.width - markBtn.width - clearBtn.width - 16
                    anchors.verticalCenter: parent.verticalCenter
                    Text {
                        text: "Notifications"
                        color: center.theme.text ?? "#f2f3f7"
                        font.family: center.theme.fontFamily ?? ""
                        font.pixelSize: (center.theme.fontSize ?? 12) + 3
                        font.weight: Font.DemiBold
                    }
                    Text {
                        text: center.unread > 0 ? center.unread + " unread" : (center.shell.history.length > 0 ? "All read" : "")
                        visible: text !== ""
                        color: center.theme.subtext ?? "#b9bccb"
                        font.family: center.theme.fontFamily ?? ""
                        font.pixelSize: Math.max(6, (center.theme.fontSize ?? 12) - 1)
                    }
                }
                CenterButton { id: markBtn; activeFocusOnTab: true; anchors.verticalCenter: parent.verticalCenter; theme: center.theme; text: "Mark all read"; enabled: center.unread > 0; onClicked: center.shell.api.markRead(null) }
                CenterButton { id: clearBtn; activeFocusOnTab: true; anchors.verticalCenter: parent.verticalCenter; theme: center.theme; text: "Clear all"; enabled: center.shell.history.length > 0; onClicked: center.shell.api.clearHistory() }
            }

            Text {
                id: empty
                visible: center.shell.history.length === 0
                anchors { top: head.bottom; topMargin: 18; horizontalCenter: parent.horizontalCenter }
                height: 40
                text: "No notifications"
                color: center.theme.subtext ?? "#b9bccb"
                font.family: center.theme.fontFamily ?? ""
                font.pixelSize: center.theme.fontSize ?? 12
            }

            ListView {
                id: list
                anchors { top: head.bottom; topMargin: 10; left: parent.left; right: parent.right; bottom: parent.bottom; margins: 12; bottomMargin: 12 }
                clip: true
                spacing: 6
                boundsBehavior: Flickable.StopAtBounds
                model: center.rows
                currentIndex: -1
                highlightFollowsCurrentItem: false
                delegate: Loader {
                    id: row
                    required property var modelData
                    required property int index
                    readonly property bool current: ListView.isCurrentItem && panel.activeFocus
                    width: ListView.view.width
                    sourceComponent: modelData.kind === "group" ? groupHead : itemCard
                    Component {
                        id: groupHead
                        Item {
                            implicitHeight: 26
                            HoverHandler { id: gh }
                            TapHandler { onTapped: center.toggleFold(row.modelData.zone) }
                            Row {
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 7
                                Rectangle { width: 10; height: 10; radius: 5; color: center.accent(row.modelData.zone); anchors.verticalCenter: parent.verticalCenter }
                                Text {
                                    text: center.label(row.modelData.zone) + "  " + row.modelData.count + (row.modelData.unread > 0 ? "  (" + row.modelData.unread + " unread)" : "")
                                    textFormat: Text.PlainText      // app-supplied text: never rich text (it could load remote images)
                                    color: gh.hovered || row.current ? (center.theme.text ?? "#f2f3f7") : (center.theme.subtext ?? "#b9bccb")
                                    font.family: center.theme.fontFamily ?? ""
                                    font.pixelSize: center.theme.fontSize ?? 12
                                    font.weight: Font.DemiBold
                                }
                                Text {
                                    text: center.folded[row.modelData.zone] ? "▸ show" : "▾"
                                    color: center.theme.subtext ?? "#b9bccb"
                                    font.pixelSize: Math.max(6, (center.theme.fontSize ?? 12) - 1)
                                    anchors.verticalCenter: parent.verticalCenter
                                }
                            }
                        }
                    }
                    Component {
                        id: itemCard
                        CenterItem {
                            entry: row.modelData.entry
                            accent: center.accent(row.modelData.zone)
                            theme: center.theme
                            now: center.now
                            live: { center.shell.kept.rev; return center.shell.kept.live(row.modelData.entry) }
                            hideText: center.shell.sharing
                            current: row.current
                            onOpened: center.open_(row.modelData.entry, live)
                            onInvoked: key => center.press(row.modelData.entry.id, key)
                            onRemoved: center.shell.api.removeHistory(row.modelData.entry.id, row.modelData.entry.t)
                        }
                    }
                }
            }
        }
    }
}
