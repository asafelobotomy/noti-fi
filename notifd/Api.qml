import QtQuick
import Quickshell
import Quickshell.Io

// Control API. One implementation, two front doors:
//   - `qs ipc call notifd <fn>`   (strings in, JSON string out; used by scripts), the IpcHandler below
//   - the control socket          (JSON lines, request/response plus pushed events), see Control.qml
// The functions return plain objects and throw Error(message) with a `code` on failure (see fail()).
Scope {
    id: api
    required property var shell

    function fail(code, message) { const e = new Error(message); e.code = code; throw e }

    function status() {
        const s = api.shell, rows = s.rowList
        const per = {}                // notifications per zone
        const units = {}              // zone -> [[unit, notifications in it]] in display order
        let onScreen = 0, queued = 0
        for (let i = 0; i < rows.count; i++) {
            const x = rows.get(i)
            if (x.leaving) continue
            per[x.zone] = (per[x.zone] || 0) + 1
            const list = units[x.zone] = units[x.zone] || []
            const u = s.layout.unitOf(x)
            const hit = list.find(e => e[0] === u)
            if (hit) hit[1]++; else list.push([u, 1])
        }
        // a stack takes one place (or, expanded, up to a list's worth); places beyond the cap wait
        for (const z in units) {
            units[z].forEach((e, i) => {
                if (i >= s.layout.cap()) { queued++; return }
                const k = z + "|" + e[0].slice(2)
                onScreen += (e[0].startsWith("s:") && s.layout.expandedStacks[k] === true && e[1] > 1) ? Math.min(e[1], s.layout.listMax()) : 1
            })
        }
        return { dnd: s.ctl.dnd, muted: s.ctl.muted, zones: Object.keys(s.cfg.zones), active: onScreen, queued: queued,
                 focus: s.ctl.focus in s.cfg.focus.modes ? s.ctl.focus : "", perZone: per, stacks: s.layout.stackList(), cap: s.layout.cap(),
                 history: s.history.length, cfgError: s.cfgError, auto: s.auto, holding: s.holdReason(), previewing: s.preview.ids.length > 0,
                 unread: unread.all, unreadPerZone: unread.perZone, center: s.centerOpen, kept: s.kept.count, inhibitors: s.inhibitors }
    }
    // history entries not yet read (a popup the user dismissed counts as read; one that timed out does not)
    readonly property var unread: {
        const per = {}
        let all = 0
        for (const h of api.shell.history) if (h.read !== true) { all++; per[h.zone] = (per[h.zone] || 0) + 1 }
        return { all: all, perZone: per }
    }
    // the notification center: action is open, close or toggle
    function center(action) {
        const s = api.shell
        const on = action === "open" ? true : action === "close" ? false : action === "toggle" ? !s.centerOpen : null
        if (on === null) fail("bad_params", "action must be open, close or toggle")
        s.centerOpen = on
        s.scheduleState()
        return status()
    }
    // mark these history ids read, or every entry when ids is not a list
    function markRead(ids) {
        const s = api.shell
        const all = !Array.isArray(ids)
        if (!s.history.some(h => h.read !== true && (all || ids.indexOf(h.id) >= 0))) return status()
        s.history = s.history.map(h => h.read !== true && (all || ids.indexOf(h.id) >= 0) ? Object.assign({}, h, { read: true }) : h)
        s.saveSoon()
        s.scheduleState()
        return status()
    }
    // forget one history entry (id and time together: ids start again at 1 when the daemon restarts)
    function removeHistory(id, t) {
        const s = api.shell
        if (t >= s.kept.startedAt) s.kept.close(id, true)     // the app is told it was dismissed
        s.history = s.history.filter(h => !(h.id === id && h.t === t))
        s.saveSoon()
        s.scheduleState()
        return status()
    }
    function configStatus() {
        const c = api.shell.cfg
        return { ok: api.shell.cfgError === "", error: api.shell.cfgError, rules: c.rules.length, zones: Object.keys(c.zones),
                 screenFound: c.screen === "" || Quickshell.screens.some(x => x.name === c.screen) }
    }
    function configGet() { return { raw: api.shell.rawConfig, effective: api.shell.cfg, error: api.shell.cfgError } }
    function dnd(mode) {
        const s = api.shell
        const on = mode === "on" ? true : mode === "off" ? false : mode === "toggle" ? !s.ctl.dnd : null
        if (on === null) fail("bad_params", "mode must be on, off or toggle")
        s.ctl = Object.assign({}, s.ctl, { dnd: on })
        s.saveSoon()
        s.flash(on ? "Do Not Disturb on" : "Do Not Disturb off", on ? "#fbbf24" : "#34d399")
        return status()
    }
    function mute(zone, on) {
        const s = api.shell
        if (!(zone in s.cfg.zones)) fail("bad_params", "unknown zone '" + zone + "'")
        const m = s.ctl.muted.filter(z => z !== zone)
        if (on) m.push(zone)
        s.ctl = Object.assign({}, s.ctl, { muted: m })
        s.saveSoon()
        s.flash((s.cfg.zones[zone].label || zone) + (on ? " muted" : " unmuted"), s.cfg.zones[zone].accent || "#60a5fa")
        return status()
    }
    function focus(mode) {
        const s = api.shell
        const m = String(mode ?? "")
        if (m === "off" || m === "") s.ctl = Object.assign({}, s.ctl, { focus: "" })
        else {
            if (!(m in s.cfg.focus.modes)) fail("bad_params", "unknown focus mode '" + m + "' (have: " + Object.keys(s.cfg.focus.modes).join(", ") + ")")
            s.ctl = Object.assign({}, s.ctl, { focus: m })
        }
        s.saveSoon()
        s.flash(s.ctl.focus === "" ? "Focus off" : "Focus: " + (s.cfg.focus.modes[s.ctl.focus].label || s.ctl.focus), s.ctl.focus === "" ? "#34d399" : "#a78bfa")
        s.scheduleState()
        return status()
    }
    // action: toggle (expand or collapse), clear (dismiss every notification in the stack) or scroll (delta lines)
    function stack(zone, key, action, delta) {
        const s = api.shell
        if (!(zone in s.cfg.zones)) fail("bad_params", "unknown zone '" + zone + "'")
        if (action === "toggle") s.layout.toggleStack(zone, key)
        else if (action === "clear") s.layout.dismissStack(zone, key)
        else if (action === "scroll") s.layout.scrollStack(zone, key, Number(delta) || 0)
        else fail("bad_params", "action must be toggle, clear or scroll")
        return status()
    }
    function samples(nid) { return api.shell.transfers.copyOf(nid) }
    function setAuto(kind, on) {
        if (kind !== "fullscreen" && kind !== "share") fail("bad_params", "kind must be fullscreen or share")
        api.shell.setAuto(kind, on)
        return status()
    }
    function reply(nid, text) {
        const n = api.shell.byId[nid]
        if (!n) fail("not_found", "no such notification on screen")
        if (!n.hasInlineReply) fail("bad_params", "that notification does not accept replies")
        n.sendInlineReply(text)
        return { ok: true }
    }
    function validate(text) {
        const r = api.shell.config.check(text)
        return r.error ? { ok: false, error: r.error } : { ok: true, rules: r.cfg.rules.length, zones: Object.keys(r.cfg.zones) }
    }
    // req: { config?: {...}, notification: {appName, desktopEntry, origin, summary, body, urgency, expireTimeout}, ignoreState?: bool }
    function simulate(req) {
        const s = api.shell
        const n = req.notification || {}
        let c = s.cfg, comp = s.compiled
        if (req.config !== undefined) {
            const r = s.config.check(JSON.stringify(req.config))
            if (r.error) fail("bad_params", "config: " + r.error)
            c = r.cfg
            comp = s.config.compile(c)
        }
        const f = { appName: String(n.appName ?? ""), desktopEntry: String(n.desktopEntry ?? ""), origin: String(n.origin ?? ""),
                    category: String(n.category ?? ""), channel: String(n.channel ?? (n.origin ? "web" : "app")),
                    summary: String(n.summary ?? ""), body: String(n.body ?? ""),
                    urgency: ["low", "normal", "critical"].indexOf(n.urgency) >= 0 ? n.urgency : "normal" }
        // ignoreState shows the rule outcome alone; a `focus` name in the request tries a focus mode on top of that
        const st = req.ignoreState === true ? { dnd: false, muted: [], focus: String(req.focus ?? ""), auto: { fullscreen: false, share: false } } : s.liveState()
        const d = s.decide(c, comp, f, typeof n.expireTimeout === "number" ? n.expireTimeout : -1, st)
        return { ok: true, zone: d.zone, timeout: d.timeout, silent: d.silent, wouldShow: d.why === "",
                 hold: d.why, matched: d.matched, decidedBy: d.decidedBy, style: d.style,
                 matchedIdx: d.matchedIdx, decidedIdx: d.decidedIdx }
    }
    // req: { config?: {...}, samples?: [{appName, summary, body, urgency, zone?, progress?, reply?, actions?}], seconds?: 6 }
    function preview(req) {
        const r = api.shell.preview.start(req)
        if (r.error) fail("bad_params", r.error)
        api.shell.emit("preview", { state: "started", shown: r.shown, seconds: r.seconds })
        return r
    }
    function endPreview() { api.shell.preview.end(); return status() }
    // history entries, newest first; `live`: the app still holds it, so its buttons work (see Kept.qml), with `actions` [{key, text}]
    function history(limit) {
        const s = api.shell
        return s.history.slice(0, Math.max(0, Math.min(1000, Number(limit) || 0))).map(h => {
            const n = s.kept.live(h)
            return Object.assign({}, h, { live: n !== null, actions: n ? (n.actions || []).map(a => ({ key: a.identifier, text: a.text })) : [] })
        })
    }
    function seenApps() {
        const seen = api.shell.seen
        return Object.keys(seen).map(k => {
            const e = seen[k]
            const types = Object.keys(e.types || {}).map(t => Object.assign({ key: t }, e.types[t])).sort((x, y) => y.count - x.count)
            return { key: k, app: e.app, desktopEntry: e.desktopEntry, count: e.count, last: e.last, types: types }
        }).sort((a, b) => b.last - a.last)
    }
    function clearHistory() { api.shell.kept.closeAll(true); api.shell.history = []; api.shell.saveSoon(); api.shell.scheduleState(); return status() }
    // the apps that hold notifications (sent by `notify serve`, which answers Inhibit for notifd): replaces the whole list
    function inhibit(by) {
        if (!Array.isArray(by)) fail("bad_params", "by must be a list of {app, reason}")
        api.shell.inhibitors = by.slice(0, 64).map(i => ({ app: String((i && i.app) ?? "").slice(0, 200), reason: String((i && i.reason) ?? "").slice(0, 200) }))
        return status()
    }
    // press one of a notification's buttons (key "default" = clicking the notification itself), on screen or kept for the center
    function invoke(nid, key) {
        const s = api.shell
        const n = s.kept.byId[nid] || s.byId[nid]
        if (!n) fail("not_found", "that notification is no longer live (closed by its app, or from before notifd restarted)")
        const a = (n.actions || []).find(x => x.identifier === key)
        if (!a) fail("bad_params", "that notification has no button '" + key + "'")
        markRead([nid])
        a.invoke()
        return status()
    }
    function dismissAll() {
        const s = api.shell, rows = s.rowList
        // only notifications that are still live: one that is already fading out is closed and must not be closed again
        const ids = []
        for (let i = 0; i < rows.count; i++) { const x = rows.get(i); if (!x.leaving) ids.push(x.nid) }
        for (const id of ids) { const n = s.byId[id]; if (n) { try { n.dismiss() } catch (e) {} } }
        return status()
    }

    function json(fn) {
        try { return JSON.stringify(fn()) } catch (e) { return JSON.stringify({ error: e.message }) }
    }
    function parseReq(text) {
        try { return JSON.parse(text) } catch (e) { fail("bad_request", "request is not valid JSON") }
    }

    IpcHandler {
        target: "notifd"
        function status(): string { return api.json(() => api.status()) }
        function configStatus(): string { return api.json(() => api.configStatus()) }
        function dnd(mode: string): string { return api.json(() => api.dnd(mode)) }
        function mute(zone: string, on: bool): string { return api.json(() => api.mute(zone, on)) }
        function focus(mode: string): string { return api.json(() => api.focus(mode)) }
        function stack(zone: string, key: string, action: string, delta: int): string { return api.json(() => api.stack(zone, key, action, delta)) }
        function samples(nid: int): string { return api.json(() => api.samples(nid)) }
        function setAuto(kind: string, on: bool): string { return api.json(() => api.setAuto(kind, on)) }
        function reply(nid: int, text: string): string { return api.json(() => api.reply(nid, text)) }
        function validate(text: string): string { return api.json(() => api.validate(text)) }
        function simulate(text: string): string { return api.json(() => api.simulate(api.parseReq(text))) }
        function preview(text: string): string { return api.json(() => api.preview(api.parseReq(text))) }
        function endPreview(): string { return api.json(() => api.endPreview()) }
        function history(limit: int): string { return api.json(() => api.history(limit)) }
        function seenApps(): string { return api.json(() => api.seenApps()) }
        function clearHistory(): string { return api.json(() => api.clearHistory()) }
        function dismissAll(): string { return api.json(() => api.dismissAll()) }
        function center(action: string): string { return api.json(() => api.center(action)) }
        function markAllRead(): string { return api.json(() => api.markRead(null)) }
        function invoke(nid: int, key: string): string { return api.json(() => api.invoke(nid, key)) }
    }
}
