pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland

// One layer-shell window per zone, showing that zone's popups.
Variants {
    id: zones
    required property var root          // the daemon (shell.qml): configuration, rows, stacks
    model: Object.keys(root.cfg.zones)

    PanelWindow {
        id: win
        required property string modelData
        readonly property var zone: root.cfg.zones[modelData]
        readonly property bool free: zone.anchor === "free"
        readonly property real fx: free ? Math.min(1, Math.max(0, Number(zone.x))) : 0
        readonly property real fy: free ? Math.min(1, Math.max(0, Number(zone.y))) : 0
        // vertical: top / middle / bottom (a free zone grows down from the upper half and up from the lower half)
        readonly property bool atTop: free ? fy <= 0.5 : zone.anchor.startsWith("top")
        readonly property bool atBottom: free ? fy > 0.5 : zone.anchor.startsWith("bottom")
        // horizontal: left / center / right (a free zone is placed from its left edge)
        readonly property bool atLeft: free ? true : zone.anchor.endsWith("left")
        readonly property bool atCenter: !free && (zone.anchor.endsWith("center") || zone.anchor === "center")
        readonly property bool atRight: !free && !atLeft && !atCenter

        screen: Quickshell.screens.find(s => s.name === root.cfg.screen) ?? Quickshell.screens[0]
        readonly property real screenW: screen ? screen.width : 1920
        readonly property real screenH: screen ? screen.height : 1080
        // the window is as wide as the widest popup it shows (popups can have their own width, or be resized by hand)
        readonly property real zw: { root.topo; return root.layout.zoneWidth(modelData) }
        readonly property real baseW: zone.width ?? root.cfg.width

        // an edge left unanchored means "centred on that axis"
        anchors { top: atTop; bottom: atBottom; left: atLeft; right: atRight }
        margins {
            top: free ? Math.round(fy * screenH) : root.cfg.margin
            bottom: free ? Math.round((1 - fy) * screenH) : root.cfg.margin
            left: free ? Math.round(fx * Math.max(0, screenW - baseW)) : root.cfg.margin
            right: root.cfg.margin
        }
        implicitWidth: zw
        implicitHeight: Math.max(1, stack.height)
        exclusionMode: ExclusionMode.Normal   // respect panels (taskbar)
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "notifd-" + modelData
        // take keyboard focus only when you click into a reply box, never otherwise (and never for popups set to "no focus")
        readonly property bool wantsKeys: { root.topo; return root.layout.zoneWantsKeys(modelData) }
        WlrLayershell.keyboardFocus: wantsKeys ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None
        color: "transparent"

        // Where clicks land on this window: on the popups, except those set to "click through", which let clicks reach what is underneath.
        readonly property var ctRects: {
            root.topo; stack.height; win.zw
            const out = []
            for (let i = 0; i < rep.count; i++) {
                const sl = rep.itemAt(i)
                if (!sl || !sl.shown || !sl.clickThrough) continue
                out.push({ i: i, x: sl.x + sl.popupItem.x, y: sl.y + sl.popupItem.y, w: sl.popupItem.width, h: sl.popupItem.height })   // `i` keeps equal rectangles distinct (Variants drops duplicates)
            }
            return out
        }
        Variants {
            id: ctRegions
            model: win.ctRects
            Region {
                required property var modelData
                intersection: Intersection.Subtract
                x: modelData.x
                y: modelData.y
                width: modelData.w
                height: modelData.h
            }
        }
        // Always mapped (so delegates exist); input only where a popup is, nothing when empty.
        mask: Region { item: stack.height > 0 ? stack : null; regions: ctRegions.instances }

        Column {
            id: stack
            width: parent.width
            anchors.top: win.atBottom ? undefined : parent.top
            anchors.bottom: win.atBottom ? parent.bottom : undefined
            spacing: 0

            move: Transition { NumberAnimation { properties: "y"; duration: 180; easing.type: Easing.OutCubic } }

            Repeater {
                id: rep
                model: rows

                delegate: Item {
                    id: slot
                    required property int nid
                    required property string zone
                    required property int timeout
                    required property bool leaving
                    required property int gen
                    required property bool pinned
                    required property int userWidth
                    required property int cardWidth
                    required property real bgOpacity
                    required property bool clickThrough
                    required property bool noFocus
                    required property string fontFamily
                    required property real fontSize
                    required property string timerStyle
                    required property string group
                    required property string priority
                    required property bool media
                    required property bool players
                    required property bool stack
                    required property string stackKey
                    readonly property bool mine: zone === win.modelData
                    readonly property int rank: { root.topo; return root.layout.rankOf(nid, zone) }
                    // a stack of notifications takes one place; expanded, it shows one list's worth (the rest scroll with the mouse wheel)
                    readonly property var sinfo: { root.topo; return root.layout.stackInfo(nid) }
                    readonly property bool stacked: stackKey !== "" && sinfo.count > 1
                    readonly property bool expanded: stacked && sinfo.expanded
                    // newest N places per zone are shown; older ones wait their turn
                    readonly property bool shown: mine && (leaving || (rank < root.layout.cap() && (!stacked
                        || (expanded ? (sinfo.idx >= sinfo.offset && sinfo.idx < sinfo.offset + sinfo.listMax) : sinfo.idx === 0))))
                    readonly property bool head: { root.topo; return root.layout.isGroupHead(nid, zone, group) }
                    readonly property real headH: head ? 20 : 0
                    readonly property bool stackFirst: expanded && shown && !leaving && sinfo.idx === sinfo.offset
                    readonly property real stackHeaderH: stackFirst ? 26 : 0
                    readonly property real shadowH: stacked && !expanded && shown ? 12 : 0
                    readonly property alias popupItem: popup

                    width: stack.width
                    height: shown ? headH + stackHeaderH + popup.implicitHeight + shadowH + root.cfg.spacing : 0
                    visible: shown
                    opacity: leaving ? 0 : 1
                    Behavior on opacity { NumberAnimation { duration: 160 } }

                    // the heading of a group of notifications (same app/site, same kind) that are kept together
                    Text {
                        visible: slot.head && slot.shown
                        x: popup.x + 4
                        y: 2
                        width: Math.max(0, popup.width - 8)
                        text: slot.group
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: win.zone.accent ?? root.cfg.theme.subtext
                        font.pixelSize: 11
                        font.weight: Font.DemiBold
                        font.family: slot.fontFamily
                    }

                    // two faint cards behind a collapsed stack, so it reads as a pile
                    Repeater {
                        model: slot.stacked && !slot.expanded && slot.shown ? 2 : 0
                        delegate: Rectangle {
                            required property int index
                            x: popup.x + 7 * (index + 1)
                            y: popup.y + popup.height - 2 + 6 * index
                            width: popup.width - 14 * (index + 1)
                            height: 10
                            radius: 6
                            color: root.cfg.theme.card
                            opacity: 0.75 - 0.25 * index
                            border.width: 1
                            border.color: root.cfg.theme.border
                            z: -1 - index
                        }
                    }

                    // header of an expanded stack: how many, which ones are in view, collapse and clear
                    Rectangle {
                        visible: slot.stackFirst
                        x: popup.x
                        y: slot.headH
                        width: popup.width
                        height: 22
                        radius: 6
                        color: root.cfg.theme.button
                        border.width: 1
                        border.color: root.cfg.theme.border
                        Text {
                            anchors { left: parent.left; leftMargin: 8; verticalCenter: parent.verticalCenter }
                            text: slot.sinfo.count + " notifications" + (slot.sinfo.count > slot.sinfo.listMax ? "  ·  showing " + (slot.sinfo.offset + 1) + "–" + Math.min(slot.sinfo.count, slot.sinfo.offset + slot.sinfo.listMax) + ", scroll for more" : "")
                            color: root.cfg.theme.subtext
                            font.pixelSize: 11
                            font.family: slot.fontFamily
                        }
                        Text {
                            id: clearTxt
                            anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                            text: "Clear all"
                            color: win.zone.accent ?? root.cfg.theme.text
                            font.pixelSize: 11
                            font.weight: Font.DemiBold
                            font.family: slot.fontFamily
                            TapHandler { onTapped: root.layout.dismissStack(slot.zone, slot.stackKey) }
                        }
                        Text {
                            anchors { right: clearTxt.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
                            text: "Collapse"
                            color: win.zone.accent ?? root.cfg.theme.text
                            font.pixelSize: 11
                            font.weight: Font.DemiBold
                            font.family: slot.fontFamily
                            TapHandler { onTapped: root.layout.toggleStack(slot.zone, slot.stackKey) }
                        }
                    }
                    // the mouse wheel over an expanded stack scrolls through its notifications
                    WheelHandler {
                        enabled: slot.expanded
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                        onWheel: event => root.layout.scrollStack(slot.zone, slot.stackKey, event.angleDelta.y > 0 ? -1 : 1)
                    }

                    Popup {
                        id: popup
                        width: slot.userWidth > 0 ? slot.userWidth : slot.cardWidth
                        x: win.atRight ? stack.width - width : (win.atCenter ? (stack.width - width) / 2 : 0)
                        y: slot.headH + slot.stackHeaderH
                        notif: { root.rev; return root.byId[slot.nid] ?? null }
                        theme: root.cfg.theme
                        accent: win.zone.accent
                        label: win.zone.label
                        timeoutMs: slot.timeout
                        gen: slot.gen
                        fromLeft: win.atLeft
                        active: slot.shown && !slot.leaving
                        pinned: slot.pinned
                        bgOpacity: slot.bgOpacity
                        fontFamily: slot.fontFamily
                        fontSize: slot.fontSize
                        timerStyle: slot.timerStyle
                        noFocus: slot.noFocus
                        clickThrough: slot.clickThrough
                        handleOnLeft: win.atRight
                        centered: win.atCenter
                        maxWidth: win.screenW - 2 * root.cfg.margin
                        priority: slot.priority
                        media: slot.media
                        players: slot.players
                        samples: { tx.rev; return tx.samplesFor(slot.nid) }
                        chartDefault: root.cfg.transfer.chart
                        stackCount: slot.stacked ? slot.sinfo.count : 1
                        stackExpanded: slot.expanded
                        onStackToggled: root.layout.toggleStack(slot.zone, slot.stackKey)
                        onDismissed: { const n = root.byId[slot.nid]; if (n && !slot.leaving) n.dismiss() }
                        onExpired:   { if (!slot.leaving) root.timedOut(slot.nid) }
                        onPinToggled: root.layout.setPinned(slot.nid, !slot.pinned)
                        onResized: w => root.layout.setUserWidth(slot.nid, w)
                    }
                }
            }

            // "+N more" when a zone has more notifications than it shows
            Item {
                id: more
                readonly property int count: { root.topo; return root.layout.waiting(win.modelData) }
                width: stack.width
                height: count > 0 ? 30 : 0
                visible: count > 0
                Rectangle {
                    anchors {
                        right: win.atRight ? parent.right : undefined
                        left: win.atLeft ? parent.left : undefined
                        horizontalCenter: win.atCenter ? parent.horizontalCenter : undefined
                    }
                    width: moreText.implicitWidth + 24
                    height: 24
                    radius: 12
                    color: root.cfg.theme.card
                    border.width: 1
                    border.color: root.cfg.theme.border
                    Text {
                        id: moreText
                        anchors.centerIn: parent
                        text: "+" + more.count + " more waiting"
                        color: root.cfg.theme.subtext
                        font.pixelSize: 11
                    }
                }
            }
        }
    }
}
