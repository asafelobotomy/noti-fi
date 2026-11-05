import QtQuick

// What shows a popup's remaining time on the card itself: a footer line that shortens, and/or a frame that is drawn around the card and
// shrinks (starting at the top middle, going clockwise). The circle that empties is TimerRing.qml, which sits in the popup's header.
// `styles` is the popup's timerStyle: any of "line", "frame" separated by commas ("none" or "" shows nothing). `fraction` is the time left, 1 to 0.
Item {
    id: root
    property real fraction: 1
    property string styles: "none"
    property color color: "#60a5fa"
    property real cornerRadius: 12
    readonly property bool hasLine: styles.indexOf("line") >= 0
    readonly property bool hasFrame: styles.indexOf("frame") >= 0

    Rectangle {
        visible: root.hasLine
        anchors { left: parent.left; bottom: parent.bottom; leftMargin: 14; bottomMargin: 4 }
        width: Math.max(0, (parent.width - 28) * root.fraction)
        height: 3
        radius: 1.5
        color: root.color
    }

    Canvas {
        id: frame
        visible: root.hasFrame
        anchors.fill: parent
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
        Connections { target: root; function onFractionChanged() { if (root.hasFrame) frame.requestPaint() } }
        onPaint: {
            const ctx = getContext("2d")
            ctx.reset()
            if (!root.hasFrame || root.fraction <= 0) return
            const lw = 2, x0 = lw / 2, y0 = lw / 2, x1 = width - lw / 2, y1 = height - lw / 2
            const rr = Math.max(0, root.cornerRadius - lw / 2), mid = (x0 + x1) / 2, q = Math.PI / 2
            // the outline clockwise from the top middle: straight sides and corner arcs, each with its length
            const segs = [
                { line: [mid, y0, x1 - rr, y0] }, { arc: [x1 - rr, y0 + rr, -q, 0] },
                { line: [x1, y0 + rr, x1, y1 - rr] }, { arc: [x1 - rr, y1 - rr, 0, q] },
                { line: [x1 - rr, y1, x0 + rr, y1] }, { arc: [x0 + rr, y1 - rr, q, 2 * q] },
                { line: [x0, y1 - rr, x0, y0 + rr] }, { arc: [x0 + rr, y0 + rr, 2 * q, 3 * q] },
                { line: [x0 + rr, y0, mid, y0] }
            ]
            for (const g of segs) g.len = g.line ? Math.hypot(g.line[2] - g.line[0], g.line[3] - g.line[1]) : rr * q
            let left = segs.reduce((n, g) => n + g.len, 0) * root.fraction     // how much of the outline is still drawn
            ctx.beginPath()
            ctx.moveTo(mid, y0)
            for (const g of segs) {
                if (left <= 0) break
                const t = Math.min(1, left / Math.max(g.len, 0.0001))
                if (g.line) ctx.lineTo(g.line[0] + (g.line[2] - g.line[0]) * t, g.line[1] + (g.line[3] - g.line[1]) * t)
                else if (rr > 0) ctx.arc(g.arc[0], g.arc[1], rr, g.arc[2], g.arc[2] + (g.arc[3] - g.arc[2]) * t)
                left -= g.len
            }
            ctx.lineWidth = lw
            ctx.strokeStyle = root.color
            ctx.stroke()
        }
    }
}
