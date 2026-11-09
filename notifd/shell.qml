pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Services.Notifications

ShellRoot {
    id: root

    // The parts of the daemon, each in its own file. They reach the shared state below through `shell`.
    readonly property Config config: Config { shell: root }        // defaults + rules.json, validated and hot-reloaded
    readonly property Tokens tokens: Tokens { shell: root }        // a desktop's design tokens under the theme (uQ's tokens.json)
    function reapply() { if (rawConfig !== "") config.apply(rawConfig) }
    readonly property Preview preview: Preview { shell: root }     // real sample popups from a draft config
    readonly property Api api: Api { shell: root }                 // the control API and `qs ipc call notifd ...`
    readonly property Control control: Control { shell: root }     // the control socket and its events
    readonly property Sound sound: Sound { shell: root }
    readonly property AutoDnd autoDnd: AutoDnd { shell: root }                                         // full-screen and screen-sharing watchers
    readonly property var rowList: rows                             // the on-screen rows, for the parts above
    readonly property var transfers: tx
    readonly property var validator: checks
    // while the screen is shared no client gets notification text either (a bar or the panel would draw it on the shared screen)
    function emit(event, data) { control.emit(event, sharing && data && "summary" in data ? Object.assign({}, data, { summary: "", body: "", hidden: true }) : data) }
    function scheduleState() { control.scheduleState() }
    onCtlChanged: scheduleState()          // Do Not Disturb, mutes and focus are pushed to subscribers as state events
    onAutoChanged: scheduleState()

    // How a popup looks and behaves. The same keys can be set on a rule (for matching notifications), on a zone, and (opacity, fontFamily,
    // fontSize) on the theme. Per key: the first matching rule that sets it wins, then the zone, then the theme / global width.
    // Patterns run on text that apps and web pages control, on the thread that draws every popup: see Validate.qml (pattern length) and maxSubject (text length).
    readonly property int maxSubject: 2000
    readonly property var styleKeys: ["opacity", "clickThrough", "noFocus", "width", "fontFamily", "fontSize", "group", "priority", "media", "stack", "timerStyle"]
    // the registry of apps is written a few seconds after its last change; make sure a stop does not lose that
    Component.onDestruction: if (seenTimer.running) saveNow()
    Validate { id: checks }       // configuration checks and the lists they use (places, match fields, priorities, charts)

    property var cfg: config.defaults
    property var compiled: []          // [{ r: rule, tests: [[field, RegExp], ...] }]
    property string cfgError: ""


    // =====================================================================
    // Runtime state: DND, muted zones, history
    // =====================================================================
    property var ctl: ({ dnd: false, muted: [], focus: "" })
    property var history: []
    onHistoryChanged: kept.prune()
    // every app that has ever sent us a notification: { key: { app, desktopEntry, count, last } }. Kept even when history is not.
    property var seen: ({})
    readonly property string stateDir: (Quickshell.env("XDG_STATE_HOME") || (Quickshell.env("HOME") + "/.local/state")) + "/notifd"

    FileView {
        id: stateFile
        path: root.stateDir + "/state.json"
        onLoaded: {
            try {
                const d = JSON.parse(text())
                root.ctl = { dnd: !!d.dnd, muted: Array.isArray(d.muted) ? d.muted : [], focus: typeof d.focus === "string" ? d.focus : "" }
                if (Array.isArray(d.history)) root.history = d.history
                if (d.seen && typeof d.seen === "object" && !Array.isArray(d.seen)) root.seen = d.seen
            } catch (e) { console.warn("notifd: ignoring unreadable state.json") }
        }
        onSaveFailed: console.warn("notifd: could not save state (is", root.stateDir, "writable?)")
    }
    Timer { id: saveTimer; interval: 400; onTriggered: root.saveNow() }
    Timer { id: seenTimer; interval: 5000; onTriggered: root.saveNow() }
    function saveSoon() { saveTimer.restart() }
    function saveNow() {
        stateFile.setText(JSON.stringify({ dnd: ctl.dnd, muted: ctl.muted, focus: ctl.focus, history: cfg.history.persist === true ? history : [], seen: seen }))
    }

    // ---- automatic Do Not Disturb: full-screen window, screen sharing ----
    property var auto: ({ fullscreen: false, share: false })
    // why notifications are being held right now ("" = not held); critical ones may still pass
    function holdFor(c, st) {
        if (st.dnd) return "dnd"
        if (st.inhibited) return "inhibited"
        if (c.autoDnd.fullscreen && st.auto.fullscreen) return "fullscreen"
        if (c.autoDnd.screenShare && st.auto.share) return "screen sharing"
        return ""
    }
    function liveState() { return { dnd: ctl.dnd, muted: ctl.muted, focus: ctl.focus, auto: auto, inhibited: inhibitors.length > 0 } }
    // apps that asked for notifications to be held (Inhibit, through `notify serve`): [{app, reason}]
    property var inhibitors: []
    // the screen is being shared: KWin's capture stream (auto.share), or an app's inhibition that says so (KDE's screen-share
    // portal sends "Screen sharing in progress" as the share starts, before the stream shows in PipeWire)
    readonly property bool sharing: auto.share === true || inhibitors.some(i => /screen ?shar|screencast|screen ?record/i.test(i.reason))
    onInhibitorsChanged: scheduleState()
    function holdReason() {
        const h = holdFor(cfg, liveState())
        return h !== "" ? h : (ctl.focus !== "" && ctl.focus in cfg.focus.modes ? "focus" : "")
    }
    function setAuto(kind, on) {
        if (kind === "fullscreen") auto = { fullscreen: on, share: auto.share }
        else if (kind === "share") auto = { fullscreen: auto.fullscreen, share: on }
    }


    // fields rules can match on (also used for history)
    function fields(n) {
        const h = n.hints || {}
        return {
            // portal notifications arrive with no app name, only a desktop entry
            appName: n.appName || n.desktopEntry, desktopEntry: n.desktopEntry, summary: n.summary, body: n.body,
            origin: h["x-kde-origin-name"] ? String(h["x-kde-origin-name"]) : "",
            // the freedesktop "category" hint (email.arrived, im.received, transfer.complete...); "" when the app sets none
            category: h["category"] ? String(h["category"]) : "",
            // how it reached us: a browser page ("web"), a sandboxed app through the portal ("portal"), or a normal app ("app")
            channel: h["x-kde-origin-name"] ? "web" : (!n.appName && n.desktopEntry ? "portal" : "app"),
            urgency: n.urgency === NotificationUrgency.Critical ? "critical" : n.urgency === NotificationUrgency.Low ? "low" : "normal"
        }
    }

    function record(f, n, r, why) {
        const limit = cfg.history.limit || 200
        const h = history.slice(0, limit - 1)
        h.unshift({
            t: Date.now(), id: n.id, app: String(f.appName || "").slice(0, 200), icon: String(n.appIcon || "").slice(0, 500), summary: String(f.summary || "").slice(0, 300), body: (f.body || "").slice(0, 300),
            zone: r.zone, origin: f.origin, urgency: f.urgency, shown: why === "", why: why, desktopEntry: f.desktopEntry || "", category: f.category || "", channel: f.channel || "app",
            read: false
        })
        history = h
        noteSeen(f, n)
        if (cfg.history.persist === true) saveSoon()
    }

    // What a notification can do, as far as this one shows (the union over an app's notifications of one type is kept).
    function capsOf(n, f) {
        const h = n.hints || {}
        const c = []
        if ((n.actions || []).length > 0) c.push("actions")
        if (n.hasInlineReply) c.push("reply")
        if (n.image && String(n.image) !== "") c.push("image")
        else if (n.appIcon && String(n.appIcon) !== "") c.push("icon")
        if (typeof h["value"] === "number") c.push("progress")
        if (n.resident) c.push("resident")
        if (n.transient) c.push("transient")
        if (f.urgency !== "normal") c.push(f.urgency)
        if (h["sound-file"] || h["sound-name"]) c.push("sound")
        return c
    }
    function noteSeen(f, n) {
        const key = String(f.desktopEntry || f.appName || "").toLowerCase().slice(0, 200)
        if (key === "") return
        const now = Date.now()
        const prev = Object.prototype.hasOwnProperty.call(seen, key) ? seen[key] : undefined
        if (!prev && Object.keys(seen).length >= 500) return      // bounded: never grows without limit
        // updated in place: nothing binds to `seen`, and copying up to 500 entries per notification is wasted work
        const types = prev && prev.types ? prev.types : {}
        // a "type" is the notification's category on one channel (app / portal / web); apps without categories have just "general"
        const tkey = (f.category || "general") + "|" + f.channel
        if (types[tkey] !== undefined || Object.keys(types).length < 24) {
            const t = types[tkey] || { category: f.category, channel: f.channel, caps: [], count: 0, last: 0 }
            const caps = t.caps.slice()
            for (const c of capsOf(n, f)) if (caps.indexOf(c) < 0) caps.push(c)
            types[tkey] = { category: f.category, channel: f.channel, caps: caps, count: t.count + 1, last: now }
        }
        seen[key] = { app: String(f.appName || ""), desktopEntry: String(f.desktopEntry || ""), count: (prev ? prev.count : 0) + 1, last: now, types: types }
        seenTimer.restart()           // the registry is written a few seconds after the last change, not once per notification
    }

    // a replaced notification updates its history entry instead of adding a new one
    function recordUpdate(f, n, r) {
        // ids start again at 1 when notifd restarts: only an entry from this run can be this notification's
        const i = history.findIndex(x => x.id === n.id && x.t >= kept.startedAt)
        if (i < 0) return
        const h = history.slice()
        h[i] = Object.assign({}, h[i], { t: Date.now(), summary: String(f.summary || "").slice(0, 300), body: (f.body || "").slice(0, 300),
                                          zone: r.zone, urgency: f.urgency, icon: String(n.appIcon || "").slice(0, 500),
                                          shown: r.why === "", why: r.why, read: false, updates: (h[i].updates || 0) + 1 })
        history = h
        if (cfg.history.persist === true) saveSoon()
    }

    // =====================================================================
    // decide(): the ONE place that turns "a notification" into "what happens to it".
    // Real notifications, the simulator and previews all go through here, so what the
    // simulator says is exactly what will happen.
    //   c    config            comp  compiled rules for c
    //   f    fields (see fields())
    //   exp  the app's requested timeout (-1 default, 0 never, >0 ms)
    //   st   { dnd, muted[], auto: { fullscreen, share }, focus, inhibited }
    // First matching rule wins, per field (zone, timeout, silent).
    // =====================================================================
    // Focus mode: a notification passes when it is important enough, or matches one of the mode's allow entries
    function focusHold(c, st, f, priority) {
        const m = st.focus ? c.focus.modes[st.focus] : undefined
        if (!m) return ""
        if (checks.priorities.indexOf(priority) >= checks.priorities.indexOf(m.minPriority ?? "urgent")) return ""
        for (const a of (m.allow || [])) {
            let ok = true
            for (const k in a) { try { if (!new RegExp(a[k], "i").test(String(f[k] ?? "").slice(0, maxSubject))) { ok = false; break } } catch (e) { ok = false; break } }
            if (ok && Object.keys(a).length > 0) return ""
        }
        return "focus " + st.focus
    }
    Route { id: route; shell: root }      // decide() and the System group: see Route.qml
    function decide(c, comp, f, exp, st) { return route.decide(c, comp, f, exp, st) }

    // =====================================================================
    // On-screen rows. ListModels can't hold QObjects, so rows carry the id
    // and byId maps it to the Notification object.
    // =====================================================================
    property var byId: ({})
    property int rev: 0        // bumps when a notification's content is replaced
    property int topo: 0       // bumps whenever rows change; popups re-rank on it
    ListModel { id: rows }
    function touch() { topo++; scheduleState() }

    Transfers { id: tx }       // samples of downloads and transfers (rate over time), see Transfers.qml

    readonly property Rows layout: Rows { shell: root }    // ranking, stacks and sizes of the on-screen rows, see Rows.qml
    readonly property Kept kept: Kept { shell: root }      // notifications kept alive for the notification center's buttons

    function show(n) {
        const f = fields(n)
        const r = decide(cfg, compiled, f, n.expireTimeout, liveState())
        if (cfg.debug)
            console.log("notifd: [" + f.appName + "] '" + f.summary + "' -> " + r.zone + " timeout=" + r.timeout + (r.why ? " held: " + r.why : ""))
        n.tracked = true                       // keep it alive until we drop it

        if (layout.rowIndex(n.id) >= 0) return        // already on screen (replacements arrive as property changes, see watchUpdates)

        if (!n.transient) record(f, n, r, r.why)   // transient notifications are not kept
        emit("notification", { id: n.id, app: f.appName, summary: f.summary, body: (f.body || "").slice(0, 300), zone: r.zone,
                                origin: f.origin, urgency: f.urgency, shown: r.why === "", why: r.why, transient: !!n.transient })
        // closing it yourself (the X, a click, an action) counts as reading it; timing out does not
        n.closed.connect(reason => { if (reason === NotificationCloseReason.Dismissed) api.markRead([n.id]); root.kept.release(n.id); root.drop(n.id) })
        watchUpdates(n)
        place(n, f, r)
    }
    // On screen, or (held by Do Not Disturb, a mute, a focus mode) kept for the notification center
    function place(n, f, r) {
        if (r.why !== "") { if (!kept.keep(n)) n.expire(); return }
        byId[n.id] = n
        tx.note(n, f)
        layout.addRow(layout.rowFor(n.id, r.zone, r.timeout, r.style))
        touch()
        sound.play(n)
    }
    // Its popup timed out: keep it for the notification center (the app still holds it), or close it
    function timedOut(nid) {
        const n = byId[nid]
        if (!n) return
        if (kept.keep(n)) drop(nid); else n.expire()
    }

    // An app that replaces a notification (same id) does not produce a new notification:
    // the library updates the existing object in place and emits change signals. When that
    // happens we re-resolve the rules, restart the timer, and update the one history entry.
    property var pendingUpdates: ({})
    function watchUpdates(n) {
        const bump = () => root.queueUpdate(n)
        n.summaryChanged.connect(bump)
        n.bodyChanged.connect(bump)
        n.hintsChanged.connect(bump)
        n.actionsChanged.connect(bump)
        n.urgencyChanged.connect(bump)
        n.imageChanged.connect(bump)
        n.appIconChanged.connect(bump)
        n.expireTimeoutChanged.connect(bump)
    }
    function queueUpdate(n) {                  // several signals fire per replacement: coalesce
        if (pendingUpdates[n.id]) return
        pendingUpdates[n.id] = true
        Qt.callLater(() => { delete root.pendingUpdates[n.id]; root.applyUpdate(n) })
    }
    function applyUpdate(n) {
        const i = layout.rowIndex(n.id)
        if ((i < 0 || rows.get(i).leaving) && kept.byId[n.id]) {   // replaced after its popup left: show it again (as a new notification would be)
            if (i >= 0) { rows.remove(i); reapQueue = reapQueue.filter(x => x !== n.id); delete byId[n.id]; tx.drop(n.id) }   // still fading: take it away now
            kept.release(n.id)
            const f = fields(n), r = decide(cfg, compiled, f, n.expireTimeout, liveState())
            recordUpdate(f, n, r)
            emit("updated", { id: n.id, summary: f.summary, body: (f.body || "").slice(0, 300), zone: r.zone })
            place(n, f, r)
            return
        }
        if (i < 0 || rows.get(i).leaving) return
        const f = fields(n)
        const r = decide(cfg, compiled, f, n.expireTimeout, liveState())
        rows.setProperty(i, "timeout", r.timeout)
        rows.setProperty(i, "zone", r.zone)
        tx.note(n, f)
        layout.applyStyle(i, r.style)
        rows.setProperty(i, "gen", rows.get(i).gen + 1)
        root.rev++
        recordUpdate(f, n, r)
        emit("updated", { id: n.id, summary: f.summary, body: (f.body || "").slice(0, 300), zone: r.zone })
        touch()
    }

    // last resort if show() itself fails: show it plainly rather than lose it
    function showPlain(n) {
        try {
            n.tracked = true
            if (layout.rowIndex(n.id) >= 0) return
            byId[n.id] = n
            layout.addRow(layout.rowFor(n.id, cfg.defaultZone, cfg.defaultTimeout, { width: cfg.width, opacity: 1, clickThrough: false, noFocus: false, fontFamily: "", fontSize: 12, group: "", timerStyle: "none", priority: "normal", media: true, players: false, stack: false, stackKey: "" }))
            n.closed.connect(() => root.drop(n.id))
            watchUpdates(n)
            touch()
        } catch (e) { console.warn("notifd: could not show notification at all:", e) }
    }

    // fade out first, remove once the animation is done
    property var reapQueue: []
    function drop(nid) {
        const i = layout.rowIndex(nid)
        if (i < 0 || rows.get(i).leaving) return          // already going (timed out, then closed by its app): one closed event
        rows.setProperty(i, "leaving", true)
        if (nid > 0) emit("closed", { id: nid })
        touch()
        reapQueue.push(nid)
        reaper.restart()
    }
    Timer {
        id: reaper
        interval: 200
        onTriggered: {
            for (const nid of root.reapQueue) {
                const i = root.layout.rowIndex(nid)
                if (i >= 0 && rows.get(i).leaving) { rows.remove(i); tx.drop(nid); const o = root.byId[nid]; delete root.byId[nid]; if (nid < 0 && o) o.destroy() }
            }
            root.reapQueue = []
            root.layout.pruneStacks()
            root.touch()
        }
    }

    // =====================================================================
    // Confirmation pill (DND / mute / config errors) and levels (volume ...) from a desktop shell; shown even during DND
    // =====================================================================
    property string osdText: ""
    property color osdColor: "#34d399"
    property bool osdShown: false
    property var osdLevel: null              // {kind, value, max, muted} while a level shows; null for the pill
    readonly property Screens screens: Screens { shell: root }
    function showLevel(l) {
        osdLevel = l
        osdColor = "#60a5fa"
        osdShown = true
        osdTimer.interval = 1500
        osdTimer.restart()
    }
    function flash(text, color, ms) {
        osdLevel = null
        osdText = text
        osdColor = color
        osdShown = true
        osdTimer.interval = ms || 1600
        osdTimer.restart()
    }
    Timer { id: osdTimer; interval: 1600; onTriggered: root.osdShown = false }

    // Versions the control socket reports (`notify` checks daemonVersion agrees with its own).
    readonly property string daemonVersion: "0.18.0"
    readonly property int protocolVersion: 1
    property string rawConfig: ""          // the text of rules.json as last read (configGet)


    NotificationServer {
        keepOnReload: false
        bodySupported: true
        bodyMarkupSupported: true
        actionsSupported: true
        imageSupported: true
        inlineReplySupported: true
        persistenceSupported: false
        onNotification: n => {
            try { root.show(n) }
            catch (e) { console.warn("notifd: show() failed, showing plainly:", e); root.showPlain(n) }
        }
    }

    property bool centerOpen: false        // the notification center (Center.qml) is on screen
    Center { shell: root }
    ZoneWindows { root: root }
    Osd { root: root }
}
