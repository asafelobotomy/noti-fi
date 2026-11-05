import QtQuick

// A notification with a progress value (or a speed such as "3.2 MB/s" in its text) is a download or transfer; its progress is sampled over time
// so the popup can draw the rate (see TransferChart.qml). `samples` maps a notification id to [{ t, pct, bps }]; `rev` changes whenever any list does.
QtObject {
    property var samples: ({})
    readonly property var none: []      // one shared empty list: a fresh [] per popup would make every popup re-evaluate on every sample
    property int rev: 0
    readonly property var speedUnits: ({ "": 1, "k": 1e3, "m": 1e6, "g": 1e9, "t": 1e12 })
    function parseSpeed(text) {
        const m = /(\d+(?:[.,]\d+)?)\s*([KMGT]?)(i?)B\/s/i.exec(String(text || ""))
        if (!m) return null
        const base = m[3] ? 1024 : 1000
        const exp = "kmgt".indexOf(m[2].toLowerCase()) + 1
        const v = parseFloat(m[1].replace(",", ".")) * Math.pow(base, exp)
        return isFinite(v) ? v : null
    }
    function note(n, f, t) {
        const h = n.hints || {}
        const pct = (typeof h["value"] === "number" && h["value"] >= 0 && h["value"] <= 100) ? h["value"] : -1
        const bps = parseSpeed(String(f.summary || "") + " " + String(f.body || ""))
        if (pct < 0 && bps === null) return
        const prev = samples[n.id] || []
        const last = prev.length ? prev[prev.length - 1] : null
        const now = t ?? Date.now()
        if (last && now - last.t < 50) return                 // an app that updates in bursts: keep one sample per 50 ms
        const next = prev.length >= 240 ? prev.slice(prev.length - 239) : prev.slice()
        next.push({ t: now, pct: pct, bps: bps })
        const all = Object.assign({}, samples)
        all[n.id] = next
        samples = all
        rev++
    }
    function drop(nid) {
        if (samples[nid] === undefined) return
        const all = Object.assign({}, samples)
        delete all[nid]
        samples = all
        rev++
    }

    function samplesFor(nid) { return samples[nid] ?? none }
    function copyOf(nid) { return (samples[Number(nid)] || []).map(x => Object.assign({}, x)) }

    // A made-up but plausible transfer history so a preview shows the graph
    function seed(nid, pct) {
        const out = []
        const now = Date.now()
        const n = 36
        let p = 0
        for (let i = 0; i < n; i++) {
            const speed = 4e6 + 3e6 * Math.sin(i / 3) + 1.5e6 * Math.sin(i * 1.7) + (i > 22 && i < 27 ? -3.5e6 : 0)
            p = Math.min(pct, p + pct / n)
            out.push({ t: now - (n - i) * 1000, pct: p, bps: Math.max(2e5, speed) })
        }
        const all = Object.assign({}, samples)
        all[nid] = out
        samples = all
        rev++
    }
}
