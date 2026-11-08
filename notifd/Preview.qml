import QtQuick
import Quickshell

// Preview: show REAL popups using a draft config (or the current one) for a few seconds.
// Stand-in notification objects feed the same Popup, so what you see is what you get.
// While a preview runs the draft is the active config; it is restored afterwards.
Scope {
    id: preview
    required property var shell          // cfg, compiled, config, byId, transfers, layout; decide(), drop(), touch(), emit()

    property int seq: 0
    property var ids: []
    property var saved: null             // { cfg, compiled } to restore when the preview ends
    Component {
        id: fakeComp
        QtObject {
            property int nid: 0
            property string appName: ""
            property string desktopEntry: ""
            property string summary: ""
            property string body: ""
            property string appIcon: ""
            property string image: ""
            property var hints: ({})
            property var actions: []
            property bool resident: false
            property bool hasInlineReply: false
            property string inlineReplyPlaceholder: ""
            function dismiss() { preview.shell.drop(nid) }
            function expire() { preview.shell.drop(nid) }
            function sendInlineReply(text) { preview.shell.drop(nid) }
        }
    }
    Timer { id: previewTimer; onTriggered: preview.end() }

    function samples(c, req) {
        if (Array.isArray(req.samples) && req.samples.length) return req.samples
        const zs = Object.keys(c.zones)
        const out = zs.map(z => ({ appName: c.zones[z].label || z, summary: "Preview: " + (c.zones[z].label || z) + " zone",
                                    body: "This is how notifications in this zone look.", zone: z }))
        out.push({ appName: "Chat app", summary: "Sam", body: "hey, are you free later today?", reply: true, actions: ["Open"], zone: zs[0] })
        out.push({ appName: "Downloads", summary: "Downloading file", body: "55% done", progress: 55, zone: zs[0] })
        return out
    }

    function start(req) {
        const s = preview.shell
        end()
        let c = s.cfg, comp = s.compiled
        if (req.config !== undefined) {
            const r = s.config.check(JSON.stringify(req.config))
            if (r.error) return { error: "config: " + r.error }
            c = r.cfg
            comp = s.config.compile(c)
        }
        const secs = Math.max(1, Math.min(30, Number(req.seconds) || 6))
        const calm = { dnd: false, muted: [], focus: "", auto: { fullscreen: false, share: false } }   // previews ignore Do Not Disturb
        saved = { cfg: s.cfg, compiled: s.compiled }
        s.cfg = c
        s.compiled = comp
        const shown = []
        for (const x of samples(c, req)) {
            const f = { appName: String(x.appName ?? "Preview"), desktopEntry: String(x.desktopEntry ?? ""), origin: String(x.origin ?? ""),
                        category: String(x.category ?? ""), channel: String(x.channel ?? (x.origin ? "web" : "app")),
                        summary: String(x.summary ?? ""), body: String(x.body ?? ""),
                        urgency: ["low", "normal", "critical"].indexOf(x.urgency) >= 0 ? x.urgency : "normal" }
            const d = s.decide(c, comp, f, -1, calm)
            const zone = (x.zone && (x.zone in c.zones)) ? x.zone : d.zone
            const nid = -(++seq)
            s.byId[nid] = fakeComp.createObject(s, {
                nid: nid, appName: f.appName, desktopEntry: f.desktopEntry, summary: f.summary, body: f.body,
                hints: typeof x.progress === "number" ? { value: x.progress } : {},
                actions: (x.actions || []).map(t => ({ text: String(t), invoke: () => {} })),
                hasInlineReply: x.reply === true, inlineReplyPlaceholder: x.reply === true ? "Reply…" : ""
            })
            if (typeof x.progress === "number") s.transfers.seed(nid, x.progress)
            s.layout.addRow(s.layout.rowFor(nid, zone, secs * 1000, d.style))
            shown.push(nid)
        }
        ids = shown
        previewTimer.interval = secs * 1000 + 400
        previewTimer.restart()
        s.touch()
        return { ok: true, shown: shown.length, seconds: secs }
    }

    function end() {
        const s = preview.shell
        const was = ids.length > 0 || saved !== null
        previewTimer.stop()
        for (const id of ids) s.drop(id)
        ids = []
        if (saved) { s.cfg = saved.cfg; s.compiled = saved.compiled; saved = null }
        s.touch()
        if (was) s.emit("preview", { state: "ended" })
    }
}
