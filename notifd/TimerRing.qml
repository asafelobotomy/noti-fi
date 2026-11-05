import QtQuick

// A small circle that empties as a popup's time runs out. `fraction` is the time left, 1 to 0.
Canvas {
    id: ring
    property real fraction: 1
    property color color: "#60a5fa"
    property color track: "#2c2f3a"
    implicitWidth: 18
    implicitHeight: 18
    onFractionChanged: requestPaint()
    onColorChanged: requestPaint()
    onPaint: {
        const ctx = getContext("2d")
        ctx.reset()
        const c = width / 2, r = c - 2
        ctx.lineWidth = 2.5
        ctx.strokeStyle = ring.track
        ctx.beginPath(); ctx.arc(c, c, r, 0, Math.PI * 2); ctx.stroke()
        if (ring.fraction <= 0) return
        ctx.strokeStyle = ring.color
        ctx.beginPath(); ctx.arc(c, c, r, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * ring.fraction); ctx.stroke()
    }
}
