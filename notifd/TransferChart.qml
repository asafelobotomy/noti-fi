pragma ComponentBehavior: Bound
import QtQuick

// Rate of a download or transfer over time, as bars, a line graph or an exact heatmap strip.
// `samples` is [{ t: ms, pct: 0-100 or -1, bps: bytes/second or null }, ...] in time order (from notifd).
// If any sample carries a speed (parsed from the notification's text) the chart shows bytes per second; otherwise it shows percent per second.
Item {
    id: root
    property var samples: []
    property string mode: "line"            // bars | line | heatmap
    property color accent: "#60a5fa"
    property color text: "#b9bccb"
    property color grid: "#2c2f3a"
    property string fontFamily: ""
    property real fontSize: 11

    implicitHeight: 118
    property int hoverIdx: -1

    readonly property bool useBytes: samples.some(s => s.bps !== null && s.bps !== undefined)
    // [{ t: seconds since the first sample, v: rate, dt: seconds this rate covers }]
    readonly property var points: {
        const out = []
        const s = root.samples
        if (!s || s.length === 0) return out
        const t0 = s[0].t
        for (let i = 0; i < s.length; i++) {
            let v = 0, dt = 0
            if (i > 0) {
                dt = Math.max(0.001, (s[i].t - s[i - 1].t) / 1000)
                if (root.useBytes) v = s[i].bps ?? 0
                else if (s[i].pct >= 0 && s[i - 1].pct >= 0) v = Math.max(0, (s[i].pct - s[i - 1].pct) / dt)
            } else if (root.useBytes) { v = s[i].bps ?? 0 }
            out.push({ t: (s[i].t - t0) / 1000, v: v, dt: dt })
        }
        return out
    }
    readonly property real maxV: { let m = 0; for (const p of root.points) m = Math.max(m, p.v); return m }
    readonly property real span: root.points.length > 1 ? Math.max(0.001, root.points[root.points.length - 1].t) : 1

    function rateText(v) {
        if (!root.useBytes) return (Math.round(v * 10) / 10) + " %/s"
        const u = ["B/s", "kB/s", "MB/s", "GB/s"]
        let i = 0, x = v
        while (x >= 1000 && i < u.length - 1) { x /= 1000; i++ }
        return (x >= 100 || i === 0 ? Math.round(x) : Math.round(x * 10) / 10) + " " + u[i]
    }
    function timeText(sec) {
        const s = Math.round(sec)
        return Math.floor(s / 60) + ":" + (s % 60 < 10 ? "0" : "") + (s % 60)
    }

    // the plot area inside the item (room for the readout line below)
    readonly property real plotH: height - 22

    onSamplesChanged: canvas.requestPaint()
    onModeChanged: canvas.requestPaint()
    onHoverIdxChanged: canvas.requestPaint()
    onWidthChanged: canvas.requestPaint()

    Canvas {
        id: canvas
        anchors { left: parent.left; right: parent.right; top: parent.top }
        height: root.plotH
        renderStrategy: Canvas.Cooperative
        onPaint: {
            const ctx = getContext("2d")
            ctx.reset()
            const w = width, h = height
            const pts = root.points
            ctx.strokeStyle = root.grid
            ctx.lineWidth = 1
            ctx.strokeRect(0.5, 0.5, w - 1, h - 1)
            if (pts.length < 2 || root.maxV <= 0) {
                ctx.fillStyle = root.text
                ctx.font = root.fontSize + "px sans-serif"
                ctx.fillText(pts.length < 2 ? "waiting for more data…" : "no transfer yet", 8, h / 2)
                return
            }
            const top = root.maxV * 1.1
            const xOf = t => (t / root.span) * (w - 2) + 1
            const yOf = v => h - 1 - (v / top) * (h - 2)
            if (root.mode === "bars") {
                ctx.fillStyle = root.accent
                for (let i = 1; i < pts.length; i++) {
                    const x0 = xOf(pts[i].t - pts[i].dt), x1 = xOf(pts[i].t)
                    const y = yOf(pts[i].v)
                    ctx.globalAlpha = i === root.hoverIdx ? 1 : 0.75
                    ctx.fillRect(x0 + 0.5, y, Math.max(1, x1 - x0 - 1), h - 1 - y)
                }
                ctx.globalAlpha = 1
            } else if (root.mode === "line") {
                ctx.beginPath()
                ctx.moveTo(xOf(pts[0].t), h - 1)
                for (let i = 0; i < pts.length; i++) ctx.lineTo(xOf(pts[i].t), yOf(pts[i].v))
                ctx.lineTo(xOf(pts[pts.length - 1].t), h - 1)
                ctx.closePath()
                ctx.globalAlpha = 0.25
                ctx.fillStyle = root.accent
                ctx.fill()
                ctx.globalAlpha = 1
                ctx.beginPath()
                for (let i = 0; i < pts.length; i++) {
                    if (i === 0) ctx.moveTo(xOf(pts[i].t), yOf(pts[i].v)); else ctx.lineTo(xOf(pts[i].t), yOf(pts[i].v))
                }
                ctx.lineWidth = 2
                ctx.strokeStyle = root.accent
                ctx.stroke()
                if (root.hoverIdx > 0) {
                    ctx.fillStyle = root.accent
                    ctx.beginPath()
                    ctx.arc(xOf(pts[root.hoverIdx].t), yOf(pts[root.hoverIdx].v), 3.5, 0, Math.PI * 2)
                    ctx.fill()
                }
            } else {
                // heatmap: one cell per measured interval (its width is the time it covers), colour = rate relative to the fastest
                for (let i = 1; i < pts.length; i++) {
                    const x0 = xOf(pts[i].t - pts[i].dt), x1 = xOf(pts[i].t)
                    const f = Math.min(1, pts[i].v / root.maxV)
                    // dark blue (idle) -> accent -> bright (fastest)
                    ctx.fillStyle = Qt.rgba(0.10 + 0.9 * f * f, 0.16 + 0.75 * f, 0.30 + 0.55 * f, 1)
                    ctx.fillRect(x0, 1, Math.max(1, x1 - x0 + (i === root.hoverIdx ? 0 : 0.5)), h - 2)
                    if (i === root.hoverIdx) { ctx.strokeStyle = "#ffffff"; ctx.lineWidth = 1.5; ctx.strokeRect(x0 + 0.5, 1.5, Math.max(1, x1 - x0 - 1), h - 3) }
                }
            }
        }
    }

    // readout: hovered interval, or the latest rate; and the scale
    Text {
        anchors { left: parent.left; bottom: parent.bottom }
        font.pixelSize: root.fontSize
        font.family: root.fontFamily
        color: root.text
        text: {
            const p = root.points
            if (p.length < 2) return ""
            const i = root.hoverIdx > 0 ? root.hoverIdx : p.length - 1
            return (root.hoverIdx > 0 ? "at " + root.timeText(p[i].t) + ": " : "now: ") + root.rateText(p[i].v) + "   ·   peak " + root.rateText(root.maxV)
        }
    }
    Text {
        anchors { right: parent.right; bottom: parent.bottom }
        font.pixelSize: root.fontSize
        font.family: root.fontFamily
        color: root.text
        text: root.points.length > 1 ? root.timeText(root.span) + " elapsed" : ""
    }

    MouseArea {
        anchors.fill: canvas
        hoverEnabled: true
        onPositionChanged: mouse => {
            const p = root.points
            if (p.length < 2) { root.hoverIdx = -1; return }
            const t = (mouse.x / Math.max(1, width)) * root.span
            let best = 1, bd = 1e9
            for (let i = 1; i < p.length; i++) { const d = Math.abs(p[i].t - p[i].dt / 2 - t); if (d < bd) { bd = d; best = i } }
            root.hoverIdx = best
        }
        onExited: root.hoverIdx = -1
    }
}
