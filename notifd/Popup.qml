pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts

Rectangle {
    id: card

    property var notif: null
    property var theme: ({})
    property color accent: "#60a5fa"
    property string label: ""
    property int timeoutMs: 6000
    property int gen: 0            // bumps when the notification is replaced; restarts the timer
    property bool fromLeft: false
    property bool active: true     // only the popup actually shown in its zone is active

    property bool pinned: false        // a pinned popup stays until you dismiss it
    property real bgOpacity: 1         // background opacity (text stays solid)
    property string fontFamily: ""     // "" = the system font
    property real fontSize: 12         // body text size in px; the title is 2 larger, the app line 1 smaller
    property bool noFocus: false       // never takes keyboard focus: no reply box
    property bool clickThrough: false  // clicks pass through to what is underneath
    property bool handleOnLeft: false  // which edge carries the resize handle (the one facing the screen's inside)
    property bool centered: false      // a centred popup grows on both sides, so the handle moves it twice as fast
    property real maxWidth: 1200
    property bool media: true; property bool players: false   // large pictures; audio/video players are off unless a rule enables them (a notification can name any local file)
    property var samples: []             // transfer measurements (see TransferChart.qml); 2 or more make the popup expandable
    property string chartDefault: "line"
    property string chartMode: ""       // this popup's own choice; empty = the default
    property bool graphOpen: false
    property string priority: "normal"   // low / normal / high / urgent; shown as a small badge when above normal
    property int stackCount: 1          // >1: this popup stands for a stack of that many notifications
    property bool stackExpanded: false

    signal dismissed()
    signal expired()
    signal pinToggled()
    signal stackToggled()
    signal resized(real width)

    // theme with fallbacks, so a partial theme in rules.json still works
    readonly property color cCard: theme.card ?? "#1d1f27"
    readonly property color cBorder: theme.border ?? "#2c2f3a"
    readonly property color cText: theme.text ?? "#f2f3f7"
    readonly property color cSub: theme.subtext ?? "#b9bccb"
    readonly property color cButton: theme.button ?? "#272a36"
    readonly property color cButtonHover: theme.buttonHover ?? "#323646"

    // A notification object can vanish (closed) while its popup is still fading out; read through guards.
    readonly property var acts: (notif && notif.actions) ? notif.actions : []
    readonly property bool resident: notif ? notif.resident === true : false
    // freedesktop "value" hint: 0-100 progress, or -1 when absent
    readonly property int progress: {
        const v = notif && notif.hints ? notif.hints["value"] : undefined
        return (typeof v === "number" && v >= 0 && v <= 100) ? v : -1
    }

    // ---- untrusted text handling -------------------------------------------------
    // Notification text comes from apps and web pages. Titles are shown as plain text;
    // the body keeps only <b>, <i>, <u> and <br>. Images, links and every other tag are
    // dropped so a notification can never make this machine fetch a URL.
    function safeBody(s) {
        if (!s) return ""
        return String(s)
            .replace(/<\s*(\/?)\s*(b|i|u)\s*>/gi, "\u0001$1$2\u0002")
            .replace(/<\s*br\s*\/?\s*>/gi, "\u0001br\u0002")
            .replace(/<[^>]*>/g, "")
            .replace(/\u0001(\/?)(b|i|u|br)\u0002/g, "<$1$2>")
    }
    // Only local files, Quickshell's own image providers and theme icon names are loaded.
    function iconSource(src) {
        if (!src) return ""
        if (src.startsWith("file://")) return src
        if (src.startsWith("/")) return "file://" + src
        if (src.startsWith("image://icon/") || src.startsWith("image://qsimage/")) return src
        if (/^[A-Za-z0-9._+-]+$/.test(src)) return "image://icon/" + src
        return ""
    }

    // ---- media: a large picture, and a player for a local audio/video file. Never anything remote.
    // Apps that want a player set one of the hints x-notify-video, x-notify-audio or x-notify-media to a local file;
    // x-kde-urls (the files a notification is about) is honoured too.
    function mediaKind(path) {
        const ext = (String(path).split("?")[0].split(".").pop() || "").toLowerCase()
        if (["mp4", "mkv", "webm", "mov", "avi", "m4v", "ogv"].indexOf(ext) >= 0) return "video"
        if (["mp3", "ogg", "oga", "opus", "flac", "wav", "m4a", "aac"].indexOf(ext) >= 0) return "audio"
        return ""
    }
    function localMedia(v) {
        let u = String(v || "")
        if (u.startsWith("file://")) u = decodeURIComponent(u.slice(7))
        if (!u.startsWith("/") || u.indexOf("\u0000") >= 0) return null
        const kind = mediaKind(u)
        return kind === "" ? null : { kind: kind, url: "file://" + encodeURI(u) }
    }
    readonly property var mediaFile: {
        if (!media || !players || !notif || !notif.hints) return null
        const h = notif.hints
        const list = []
        for (const k of ["x-notify-video", "x-notify-audio", "x-notify-media"]) if (h[k]) list.push(h[k])
        const urls = h["x-kde-urls"]
        if (urls) for (const u of (Array.isArray(urls) ? urls : String(urls).split(","))) list.push(u)
        for (const c of list) { const m = localMedia(c); if (m) return m }
        return null
    }
    property bool mediaPlaying: false

    function sendReply(text) {
        const t = (text || "").trim()
        if (t === "" || !notif) return
        notif.sendInlineReply(t)
        replyInput.text = ""
        if (!resident) dismissed()
    }

    implicitHeight: content.implicitHeight + 24
    radius: 12
    color: Qt.rgba(cCard.r, cCard.g, cCard.b, cCard.a * bgOpacity)
    border.width: pinned ? 2 : 1
    border.color: pinned ? accent : (hover.hovered ? Qt.lighter(accent, 1.2) : Qt.rgba(cBorder.r, cBorder.g, cBorder.b, cBorder.a * Math.max(bgOpacity, 0.4)))

    // slide-in on creation
    transform: Translate { id: slide; x: card.fromLeft ? -24 : 24 }
    Component.onCompleted: slideIn.start()
    NumberAnimation { id: slideIn; target: slide; property: "x"; to: 0; duration: 220; easing.type: Easing.OutCubic }

    HoverHandler { id: hover }
    TapHandler { enabled: card.active && !card.clickThrough; onTapped: {
        if (replyHover.hovered || pinHover.hovered || mediaHover.hovered || graphHover.hovered || gripArea.containsMouse || gripArea.pressed) return
        // a collapsed stack opens when clicked; anything else is dismissed
        if (card.stackCount > 1 && !card.stackExpanded) card.stackToggled(); else card.dismissed()
    } }

    // zone accent stripe
    Rectangle {
        anchors { left: parent.left; top: parent.top; bottom: parent.bottom; margins: 1 }
        width: 4; radius: 2
        color: card.accent
    }

    // timeout: paused while hovered, restarted when the notification is replaced.
    // timeoutMs 0 means "never expire" (the app asked for a persistent notification).
    property bool restarting: false
    onGenChanged: { restarting = true; Qt.callLater(() => card.restarting = false) }
    Timer {
        interval: card.timeoutMs
        running: card.active && !card.pinned && !card.mediaPlaying && !card.graphOpen && card.timeoutMs > 0 && !hover.hovered && !replyInput.activeFocus && card.notif !== null && !card.restarting
        repeat: false
        onTriggered: card.expired()
    }

    ColumnLayout {
        id: content
        anchors { left: parent.left; right: parent.right; top: parent.top; margins: 12; leftMargin: 18 }
        spacing: 6

        RowLayout {
            spacing: 8
            Layout.fillWidth: true

            Image {
                source: card.notif ? card.iconSource(card.notif.image !== "" && !bigImage.visible ? card.notif.image : card.notif.appIcon) : ""
                sourceSize.width: 40; sourceSize.height: 40
                Layout.preferredWidth: 40; Layout.preferredHeight: 40
                visible: status === Image.Ready
                fillMode: Image.PreserveAspectFit
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text {
                    text: (card.notif ? (card.notif.appName || card.notif.desktopEntry) : "") + (card.label !== "" ? "  ·  " + card.label : "")
                    textFormat: Text.PlainText
                    color: card.accent
                    font.pixelSize: Math.max(6, card.fontSize - 1)
                    font.family: card.fontFamily
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                Text {
                    text: card.notif ? card.notif.summary : ""
                    textFormat: Text.PlainText
                    color: card.cText
                    font.pixelSize: card.fontSize + 2
                    font.family: card.fontFamily
                    font.weight: Font.DemiBold
                    wrapMode: Text.Wrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }

            // how many notifications this popup stands for
            Rectangle {
                Layout.alignment: Qt.AlignTop
                visible: card.stackCount > 1
                implicitWidth: Math.max(implicitHeight, stackLabel.implicitWidth + 12)
                implicitHeight: stackLabel.implicitHeight + 4
                radius: height / 2
                color: card.accent
                Text {
                    id: stackLabel
                    anchors.centerIn: parent
                    text: card.stackCount
                    color: card.cCard
                    font.pixelSize: Math.max(6, card.fontSize - 1)
                    font.family: card.fontFamily
                    font.weight: Font.Bold
                }
                Accessible.name: card.stackCount + " notifications in this stack"
            }
            // priority above normal is marked
            Rectangle {
                Layout.alignment: Qt.AlignTop
                visible: card.priority === "high" || card.priority === "urgent"
                implicitWidth: prioLabel.implicitWidth + 10
                implicitHeight: prioLabel.implicitHeight + 4
                radius: 8
                color: card.priority === "urgent" ? "#f87171" : "#fbbf24"
                Text {
                    id: prioLabel
                    anchors.centerIn: parent
                    text: card.priority === "urgent" ? "Urgent" : "High"
                    color: "#1a1a1a"
                    font.pixelSize: Math.max(6, card.fontSize - 2)
                    font.family: card.fontFamily
                    font.weight: Font.Bold
                }
            }

            Rectangle {
                id: pinPill
                Layout.alignment: Qt.AlignTop
                visible: card.active && !card.clickThrough
                implicitWidth: pinLabel.implicitWidth + 12
                implicitHeight: pinLabel.implicitHeight + 4
                radius: 8
                color: card.pinned ? card.accent : (pinHover.hovered ? card.cButtonHover : card.cButton)
                HoverHandler { id: pinHover }
                TapHandler { onTapped: card.pinToggled() }
                Text {
                    id: pinLabel
                    anchors.centerIn: parent
                    text: card.pinned ? "Pinned" : "Pin"
                    color: card.pinned ? card.cCard : card.cText
                    font.pixelSize: Math.max(6, card.fontSize - 2)
                    font.family: card.fontFamily
                    font.weight: Font.DemiBold
                }
            }
        }

        Text {
            visible: text !== ""
            text: card.notif ? card.safeBody(card.notif.body) : ""
            textFormat: Text.StyledText
            color: card.cSub
            font.pixelSize: card.fontSize
            font.family: card.fontFamily
            wrapMode: Text.Wrap
            maximumLineCount: 4
            elide: Text.ElideRight
            Layout.fillWidth: true
        }

        // a large picture (web push images, screenshots, covers); small images stay as the icon
        Image {
            id: bigImage
            source: card.media && card.notif ? card.iconSource(card.notif.image) : ""
            visible: status === Image.Ready && implicitWidth >= 200
            Layout.fillWidth: true
            Layout.preferredHeight: visible ? Math.min(180, Math.round(width * implicitHeight / Math.max(1, implicitWidth))) : 0
            fillMode: Image.PreserveAspectFit
            asynchronous: true
        }

        // audio or video from a local file; loaded only when needed (needs Qt Multimedia)
        Loader {
            id: mediaLoader
            active: card.mediaFile !== null
            visible: active && status === Loader.Ready
            Layout.fillWidth: true
            Layout.preferredHeight: visible && item ? item.implicitHeight : 0
            source: "Media.qml"
            onLoaded: {
                item.source = Qt.binding(() => card.mediaFile ? card.mediaFile.url : "")
                item.kind = Qt.binding(() => card.mediaFile ? card.mediaFile.kind : "audio")
                item.fg = Qt.binding(() => card.cText)
                item.sub = Qt.binding(() => card.cSub)
                item.track = Qt.binding(() => card.cButton)
                item.accent = Qt.binding(() => card.accent)
                item.fontFamily = Qt.binding(() => card.fontFamily)
                item.fontSize = Qt.binding(() => card.fontSize)
                card.mediaPlaying = Qt.binding(() => item ? item.playing : false)
            }
            HoverHandler { id: mediaHover }
        }

        // progress bar for notifications that carry a "value" hint
        Rectangle {
            visible: card.progress >= 0
            Layout.fillWidth: true
            Layout.preferredHeight: 6
            radius: 3
            color: card.cButton
            Rectangle {
                width: parent.width * Math.max(0, card.progress) / 100
                height: parent.height
                radius: 3
                color: card.accent
                Behavior on width { NumberAnimation { duration: 150 } }
            }
        }

        // a download or transfer: expandable graph of its rate over time
        ColumnLayout {
            id: graphBox
            visible: card.samples.length >= 2
            Layout.fillWidth: true
            spacing: 4
            HoverHandler { id: graphHover }

            RowLayout {
                Layout.fillWidth: true
                spacing: 6
                Text {
                    text: "Transfer rate"
                    color: card.cSub
                    font.pixelSize: Math.max(6, card.fontSize - 1)
                    font.family: card.fontFamily
                    Layout.fillWidth: true
                }
                Repeater {
                    model: card.graphOpen ? ["bars", "line", "heatmap"] : []
                    delegate: Rectangle {
                        id: modePill
                        required property string modelData
                        readonly property bool on: (card.chartMode !== "" ? card.chartMode : card.chartDefault) === modelData
                        implicitWidth: modeText.implicitWidth + 12
                        implicitHeight: modeText.implicitHeight + 4
                        radius: 8
                        color: on ? card.accent : card.cButton
                        Text {
                            id: modeText
                            anchors.centerIn: parent
                            text: modePill.modelData === "bars" ? "Bars" : modePill.modelData === "line" ? "Graph" : "Heatmap"
                            color: modePill.on ? card.cCard : card.cText
                            font.pixelSize: Math.max(6, card.fontSize - 2)
                            font.family: card.fontFamily
                        }
                        TapHandler { onTapped: card.chartMode = modePill.modelData }
                        Accessible.name: "Show as " + modelData
                    }
                }
                Rectangle {
                    implicitWidth: toggleText.implicitWidth + 12
                    implicitHeight: toggleText.implicitHeight + 4
                    radius: 8
                    color: card.cButton
                    Text {
                        id: toggleText
                        anchors.centerIn: parent
                        text: card.graphOpen ? "Hide graph" : "Show graph"
                        color: card.cText
                        font.pixelSize: Math.max(6, card.fontSize - 2)
                        font.family: card.fontFamily
                    }
                    TapHandler { onTapped: card.graphOpen = !card.graphOpen }
                    Accessible.name: card.graphOpen ? "Hide the graph" : "Show the graph"
                }
            }
            TransferChart {
                visible: card.graphOpen
                Layout.fillWidth: true
                Layout.preferredHeight: visible ? 118 : 0
                samples: card.samples
                mode: card.chartMode !== "" ? card.chartMode : card.chartDefault
                accent: card.accent
                text: card.cSub
                grid: card.cBorder
                fontFamily: card.fontFamily
                fontSize: Math.max(6, card.fontSize - 1)
            }
        }

        // inline reply (chat apps): type and press Enter, Escape dismisses
        Rectangle {
            id: replyBox
            visible: card.notif !== null && card.notif.hasInlineReply === true && !card.noFocus
            Layout.fillWidth: true
            Layout.preferredHeight: 32
            radius: 6
            color: card.cButton
            border.width: replyInput.activeFocus ? 1 : 0
            border.color: card.accent
            HoverHandler { id: replyHover }

            TextInput {
                id: replyInput
                anchors { left: parent.left; right: sendLabel.left; verticalCenter: parent.verticalCenter; leftMargin: 10; rightMargin: 8 }
                color: card.cText
                font.pixelSize: card.fontSize
                font.family: card.fontFamily
                clip: true
                selectByMouse: true
                enabled: card.active
                onAccepted: card.sendReply(text)
                Keys.onEscapePressed: card.dismissed()
                Text {
                    visible: replyInput.text === "" && !replyInput.activeFocus
                    text: card.notif && card.notif.inlineReplyPlaceholder ? card.notif.inlineReplyPlaceholder : "Reply…"
                    textFormat: Text.PlainText
                    color: card.cSub
                    font.pixelSize: card.fontSize
                    font.family: card.fontFamily
                }
            }
            Text {
                id: sendLabel
                anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                text: "Send"
                color: replyInput.text.trim() !== "" ? card.accent : card.cSub
                font.pixelSize: card.fontSize
                font.family: card.fontFamily
                font.weight: Font.DemiBold
                TapHandler { enabled: card.active; onTapped: card.sendReply(replyInput.text) }
            }
        }

        Flow {
            visible: card.acts.length > 0
            Layout.fillWidth: true
            spacing: 6
            Repeater {
                model: card.acts
                delegate: Rectangle {
                    id: btnRect
                    required property var modelData
                    radius: 6
                    color: btn.hovered ? card.cButtonHover : card.cButton
                    implicitWidth: btnLabel.implicitWidth + 20
                    implicitHeight: 26
                    HoverHandler { id: btn }
                    TapHandler {
                        enabled: card.active
                        onTapped: { btnRect.modelData.invoke(); if (!card.resident) card.dismissed() }
                    }
                    Text {
                        id: btnLabel
                        anchors.centerIn: parent
                        text: btnRect.modelData.text
                        textFormat: Text.PlainText
                        color: card.cText
                        font.pixelSize: card.fontSize
                        font.family: card.fontFamily
                    }
                }
            }
        }
    }

    // ---- resize handle: drag the edge that faces the screen's inside; the popup keeps its new width for as long as it lives
    Item {
        id: grip
        visible: card.active && !card.clickThrough
        width: 10
        anchors { top: parent.top; bottom: parent.bottom; topMargin: 14; bottomMargin: 14 }
        anchors.left: card.handleOnLeft ? parent.left : undefined
        anchors.right: card.handleOnLeft ? undefined : parent.right
        Rectangle {
            anchors.centerIn: parent
            width: 3; height: Math.min(parent.height, 28); radius: 1.5
            color: card.cSub
            opacity: gripArea.containsMouse || gripArea.pressed ? 0.8 : 0.0
        }
        MouseArea {
            id: gripArea
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.SizeHorCursor
            property real startW: 0
            property real startX: 0
            onPressed: mouse => { startW = card.width; startX = mapToGlobal(mouse.x, mouse.y).x }
            onPositionChanged: mouse => {
                if (!pressed) return
                const dx = mapToGlobal(mouse.x, mouse.y).x - startX
                const k = card.centered ? 2 : 1
                const w = startW + (card.handleOnLeft ? -dx : dx) * k
                card.resized(Math.max(160, Math.min(card.maxWidth, w)))
            }
        }
    }
}
