import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// A sample popup drawn with the look being edited, with the "time left" indicator running so a timeout and an indicator style can be seen.
// `base` is the theme as the daemon has it; `over` holds what this level changes (each key "" = inherit).
Item {
    id: root
    property var base: ({})
    property var over: ({})
    property string accent: ""
    property int timeoutMs: 6000                // 0 = stays until dismissed
    property string title: "Sample notification"
    property string body: "This is how a popup looks with these settings."

    function pick(key, fallback) {
        const o = root.over ? root.over[key] : undefined
        if (o !== undefined && o !== null && o !== "") return o
        const b = root.base ? root.base[key] : undefined
        return b !== undefined && b !== null && b !== "" ? b : fallback
    }
    function col(v, fallback) { return typeof v === "string" && /^#[0-9a-fA-F]{3,8}$/.test(v) ? v : fallback }
    readonly property real opacityV: Math.max(0.1, Math.min(1, Number(pick("opacity", 1))))
    readonly property real widthV: Math.max(160, Math.min(560, Number(pick("width", 380))))
    readonly property real sizeV: Number(pick("fontSize", 12)) || 12
    readonly property string familyV: String(pick("fontFamily", ""))
    readonly property string timerV: String(pick("timerStyle", "line"))
    readonly property color cardC: root.col(root.base.card, "#1d1f27")
    readonly property color borderC: root.col(root.base.border, "#2c2f3a")
    readonly property color titleC: root.col(root.base.text, "#f2f3f7")
    readonly property color textC: root.col(root.base.subtext, "#b9bccb")
    readonly property color buttonC: root.col(root.base.button, "#272a36")
    readonly property color accentC: root.col(root.accent, root.borderC)

    // the card is drawn at its real width up to what fits, so a width change is visible
    readonly property real cardW: Math.min(root.width, root.widthV)
    implicitHeight: card.height + caption.height + Kirigami.Units.smallSpacing

    // 0 -> 1 over the timeout (looping; a very long timeout is shown as 6 s so the movement is visible)
    property real progress: 0
    readonly property int loopMs: root.timeoutMs > 0 ? Math.max(1500, Math.min(root.timeoutMs, 6000)) : 6000
    NumberAnimation on progress { from: 0; to: 1; duration: root.loopMs; loops: Animation.Infinite; running: root.timeoutMs > 0 && root.visible }

    Rectangle {
        id: card
        width: root.cardW
        height: col.implicitHeight + Kirigami.Units.largeSpacing * 2
        radius: 8
        opacity: root.opacityV
        color: root.cardC
        border.width: 1
        border.color: root.borderC

        // a coloured edge in the zone's accent
        Rectangle { visible: root.accent !== ""; width: 4; radius: 2; color: root.accentC; anchors { left: parent.left; top: parent.top; bottom: parent.bottom; margins: 6 } }

        ColumnLayout {
            id: col
            anchors { left: parent.left; right: parent.right; top: parent.top; margins: Kirigami.Units.largeSpacing }
            anchors.leftMargin: Kirigami.Units.largeSpacing + (root.accent !== "" ? 6 : 0)
            spacing: 2
            Text { Layout.fillWidth: true; elide: Text.ElideRight; text: root.title; font.bold: true; font.pixelSize: root.sizeV + 2; font.family: root.familyV; color: root.titleC }
            Text { Layout.fillWidth: true; wrapMode: Text.Wrap; text: root.body; font.pixelSize: root.sizeV; font.family: root.familyV; color: root.textC }
            Rectangle {
                Layout.topMargin: Kirigami.Units.smallSpacing
                implicitWidth: replyLabel.implicitWidth + 16; implicitHeight: replyLabel.implicitHeight + 8
                radius: 4; color: root.buttonC
                Text { id: replyLabel; anchors.centerIn: parent; text: "Reply"; font.pixelSize: root.sizeV; font.family: root.familyV; color: root.titleC }
            }
        }

        // time-left indicators, as in the real popup: a footer line, a circle in the corner, a frame around the card
        Rectangle {
            visible: root.timeoutMs > 0 && root.timerV.indexOf("line") >= 0
            height: 3; radius: 1.5; color: root.accent !== "" ? root.accentC : root.titleC; opacity: 0.8
            anchors { left: parent.left; bottom: parent.bottom; leftMargin: 8; bottomMargin: 5 }
            width: (parent.width - 16) * (1 - root.progress)
        }
        Canvas {
            visible: root.timeoutMs > 0 && root.timerV.indexOf("circle") >= 0
            width: 18; height: 18
            anchors { right: parent.right; top: parent.top; margins: 8 }
            property real p: root.progress
            onPChanged: requestPaint()
            onPaint: {
                const c = getContext("2d"); c.reset()
                c.lineWidth = 2; c.strokeStyle = root.accent !== "" ? root.accentC : root.titleC
                c.globalAlpha = 0.85
                c.beginPath(); c.arc(9, 9, 7, -Math.PI / 2, -Math.PI / 2 + 2 * Math.PI * (1 - p), false); c.stroke()
            }
        }
        Canvas {
            visible: root.timeoutMs > 0 && root.timerV.indexOf("frame") >= 0
            anchors.fill: parent
            property real p: root.progress
            onPChanged: requestPaint()
            onPaint: {
                const c = getContext("2d"); c.reset()
                const w = width - 2, h = height - 2
                const total = 2 * (w + h)
                let left = total * (1 - p)
                // from the top middle, clockwise
                const segs = [[w / 2, 1, w - 1, 1], [w - 1, 1, w - 1, h], [w - 1, h, 1, h], [1, h, 1, 1], [1, 1, w / 2, 1]]
                c.lineWidth = 2; c.strokeStyle = root.accent !== "" ? root.accentC : root.titleC; c.globalAlpha = 0.85
                c.beginPath(); c.moveTo(segs[0][0], segs[0][1])
                for (const s of segs) {
                    const len = Math.hypot(s[2] - s[0], s[3] - s[1])
                    const f = Math.min(1, left / len)
                    if (f <= 0) break
                    c.lineTo(s[0] + (s[2] - s[0]) * f, s[1] + (s[3] - s[1]) * f)
                    left -= len
                }
                c.stroke()
            }
        }
    }
    QQC2.Label {
        id: caption
        anchors { top: card.bottom; topMargin: Kirigami.Units.smallSpacing; left: card.left }
        opacity: 0.7
        font: Kirigami.Theme.smallFont
        text: (root.timeoutMs > 0 ? "Hides after " + (root.timeoutMs / 1000) + " s" : "Stays until dismissed") + " · " + Math.round(root.widthV) + " px wide" + (root.opacityV < 1 ? " · " + Math.round(root.opacityV * 100) + "% opaque" : "")
    }
}
