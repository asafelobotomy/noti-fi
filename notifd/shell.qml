pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Services.Notifications

ShellRoot {
    id: root

    // =====================================================================
    // Configuration: defaults + rules.json (hot-reloaded, validated).
    // A file that fails validation is rejected whole; the last good config
    // keeps running and an error pill is shown. Notifications are never
    // dropped because of a bad config.
    // =====================================================================
    readonly property var defaults: ({
        screen: "", defaultZone: "personal", defaultTimeout: 6000,
        width: 380, margin: 16, spacing: 10, maxVisible: 5, debug: false,
        dnd: { allowCritical: true },
        history: { limit: 200, persist: false },
        sound: { enabled: true, defaultName: "" },
        autoDnd: { fullscreen: true, screenShare: true },
        theme: { card: "#1d1f27", border: "#2c2f3a", text: "#f2f3f7", subtext: "#b9bccb",
                 button: "#272a36", buttonHover: "#323646", opacity: 1, fontFamily: "", fontSize: 12, timerStyle: "none" },
        // Focus modes (like Apple's): while one is on, only notifications at or above `minPriority`, or matching an `allow` entry, are shown.
        focus: { modes: { urgent: { label: "Urgent only", minPriority: "urgent" } } },
        // how the graph of a download / transfer popup is drawn by default: bars, line or heatmap
        transfer: { chart: "line" },
        zones: { personal: { anchor: "top-right", accent: "#60a5fa", label: "Personal" } },
        rules: []
    })
    // How a popup looks and behaves. The same keys can be set on a rule (for matching notifications), on a zone, and (opacity, fontFamily,
    // fontSize) on the theme. Per key: the first matching rule that sets it wins, then the zone, then the theme / global width.
    // Patterns run on text that apps and web pages control, on the thread that draws every popup: see Validate.qml (pattern length) and maxSubject (text length).
    readonly property int maxSubject: 2000
    readonly property var styleKeys: ["opacity", "clickThrough", "noFocus", "width", "fontFamily", "fontSize", "group", "priority", "media", "stack", "timerStyle"]
    // the registry of apps is written a few seconds after its last change; make sure a stop does not lose that
    Component.onDestruction: if (seenTimer.running) saveNow()
    Validate { id: checks }       // configuration checks and the lists they use (places, match fields, priorities, charts)

    property var cfg: defaults
    property var compiled: []          // [{ r: rule, tests: [[field, RegExp], ...] }]
    property string cfgError: ""

    function mergeConfig(user) {
        const c = Object.assign({}, defaults, user)
        for (const k of ["dnd", "history", "sound", "autoDnd", "theme", "focus", "transfer"])
            c[k] = Object.assign({}, defaults[k], (user && typeof user[k] === "object" && user[k]) || {})
        return c
    }

    function compile(c) {
        return c.rules.map(r => ({
            r: r,
            tests: Object.keys(r.match || {}).map(k => [k, new RegExp(r.match[k], "i")])
        }))
    }

    function reject(msg) {
        cfgError = msg
        console.warn("notifd: rules.json rejected, keeping previous config:", msg)
        emit("config", { ok: false, error: msg, rules: cfg.rules.length, zones: Object.keys(cfg.zones) })
        flash("rules.json: " + msg.slice(0, 80), "#f87171", 5000)
    }

    // parse + merge + validate a config text WITHOUT applying it: { cfg } or { error }
    function checkConfig(text) {
        let user
        try { user = JSON.parse(text) } catch (e) { return { error: "not valid JSON" } }
        if (user === null || typeof user !== "object" || Array.isArray(user)) return { error: "top level must be an object" }
        const merged = mergeConfig(user)
        const err = checks.validate(merged)
        return err !== "" ? { error: err } : { cfg: merged }
    }

    function applyConfig(text) {
        const r = checkConfig(text)
        if (r.error) { reject(r.error); return }
        cfg = r.cfg
        compiled = compile(r.cfg)
        cfgError = ""
        touch()
        emit("config", { ok: true, error: "", rules: cfg.rules.length, zones: Object.keys(cfg.zones) })
        console.log("notifd: loaded", r.cfg.rules.length, "rules,", Object.keys(r.cfg.zones).length, "zones")
    }

    FileView {
        path: Qt.resolvedUrl("rules.json")
        watchChanges: true
        onFileChanged: reload()
        onLoaded: { root.rawConfig = text(); root.applyConfig(text()) }
    }

    // =====================================================================
    // Runtime state: DND, muted zones, history
    // =====================================================================
    property var ctl: ({ dnd: false, muted: [], focus: "" })
    property var history: []
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
        if (c.autoDnd.fullscreen && st.auto.fullscreen) return "fullscreen"
        if (c.autoDnd.screenShare && st.auto.share) return "screen sharing"
        return ""
    }
    function liveState() { return { dnd: ctl.dnd, muted: ctl.muted, focus: ctl.focus, auto: auto } }
    function holdReason() {
        const h = holdFor(cfg, liveState())
        return h !== "" ? h : (ctl.focus !== "" && ctl.focus in cfg.focus.modes ? "focus" : "")
    }
    function setAuto(kind, on) {
        if (kind === "fullscreen") auto = { fullscreen: on, share: auto.share }
        else if (kind === "share") auto = { fullscreen: auto.fullscreen, share: on }
    }

    // Full-screen state comes from a small KWin script (installed by `notify autodnd`) that logs
    // "notifd-autodnd: fullscreen=1|0". We follow that line in the journal. journald stamps the
    // sending unit itself, so other apps cannot fake it.
    Process {
        id: kwinWatch
        command: ["journalctl", "--user", "-b", "-u", "plasma-kwin_wayland.service", "-n", "1", "-f", "-o", "cat", "--grep", "notifd-autodnd: fullscreen="]
        stdout: SplitParser {
            onRead: line => {
                const m = /notifd-autodnd: fullscreen=([01])/.exec(line)
                if (m) root.setAuto("fullscreen", m[1] === "1")
            }
        }
    }
    // Screen sharing: a PipeWire video stream produced by KWin means something is capturing the screen.
    readonly property string shareScript:
        "pw-dump 2>/dev/null | python3 -c '" +
        "import json,sys\n" +
        "try: d=json.load(sys.stdin)\n" +
        "except Exception: print(0); sys.exit()\n" +
        "hit=0\n" +
        "for o in d:\n" +
        "  if o.get(\"type\")!=\"PipeWire:Interface:Node\": continue\n" +
        "  p=o.get(\"info\",{}).get(\"props\",{})\n" +
        "  who=(str(p.get(\"application.name\",\"\"))+str(p.get(\"application.process.binary\",\"\"))).lower()\n" +
        "  if \"Video\" in str(p.get(\"media.class\",\"\")) and p.get(\"media.class\",\"\").startswith(\"Stream/Output\") and \"kwin\" in who: hit=1\n" +
        "print(hit)'"
    Process {
        id: shareProbe
        command: ["sh", "-c", root.shareScript]
        stdout: StdioCollector { onStreamFinished: root.setAuto("share", text.trim() === "1") }
    }
    Timer {
        // keeps the watchers alive (respawns journalctl if it ever exits) and polls for screen sharing
        interval: 4000; running: true; repeat: true; triggeredOnStart: true
        onTriggered: {
            if (root.cfg.autoDnd.watch === false) return      // watchers off (tests drive the state by hand)
            if (root.cfg.autoDnd.fullscreen && !kwinWatch.running) kwinWatch.running = true
            if (root.cfg.autoDnd.screenShare && !shareProbe.running) shareProbe.running = true
            if (!root.cfg.autoDnd.fullscreen && root.auto.fullscreen) root.setAuto("fullscreen", false)
            if (!root.cfg.autoDnd.screenShare && root.auto.share) root.setAuto("share", false)
        }
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
            t: Date.now(), id: n.id, app: f.appName, summary: f.summary, body: (f.body || "").slice(0, 300),
            zone: r.zone, origin: f.origin, urgency: f.urgency, shown: why === "", why: why
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
        const prev = seen[key]
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
        const i = history.findIndex(x => x.id === n.id)
        if (i < 0) return
        const h = history.slice()
        h[i] = Object.assign({}, h[i], { t: Date.now(), summary: f.summary, body: (f.body || "").slice(0, 300),
                                          zone: r.zone, updates: (h[i].updates || 0) + 1 })
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
    //   st   { dnd, muted[], auto: { fullscreen, share } }
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
    function holdWhy(c, st, f, zone, silent, priority) {
        const passCritical = priority === "urgent" && c.dnd.allowCritical !== false
        if (silent) return "silent rule"
        const hold = holdFor(c, st)
        if (hold !== "" && !passCritical) return hold
        const fh = focusHold(c, st, f, priority)
        if (fh !== "") return fh
        if (st.muted.indexOf(zone) >= 0 && !passCritical) return "muted " + zone
        return ""
    }
    function decide(c, comp, f, exp, st) {
        const out = { zone: undefined, timeout: undefined, silent: undefined }
        const style = {}
        const by = { zone: null, timeout: null, silent: null }
        const matched = []
        const matchedIdx = []
        const byIdx = { zone: null, timeout: null, silent: null }
        for (const sk of styleKeys) { by[sk] = null; byIdx[sk] = null }
        for (let i = 0; i < comp.length; i++) {
            const k = comp[i]
            let ok = true
            try {
                for (const t of k.tests) if (!t[1].test(String(f[t[0]] ?? "").slice(0, maxSubject))) { ok = false; break }
            } catch (e) { ok = false }
            if (!ok) continue
            const name = k.r.name || ("#" + (i + 1))
            matched.push(name)
            matchedIdx.push(i)
            if (out.zone === undefined && k.r.zone !== undefined) { out.zone = k.r.zone; by.zone = name; byIdx.zone = i }
            if (out.timeout === undefined && k.r.timeout !== undefined) { out.timeout = k.r.timeout; by.timeout = name; byIdx.timeout = i }
            if (out.silent === undefined && k.r.silent !== undefined) { out.silent = k.r.silent; by.silent = name; byIdx.silent = i }
            for (const sk of styleKeys)
                if (style[sk] === undefined && k.r[sk] !== undefined) { style[sk] = k.r[sk]; by[sk] = name; byIdx[sk] = i }
        }
        // then the zone's own style, then the theme and global width
        const z = c.zones[(out.zone !== undefined && out.zone in c.zones) ? out.zone : c.defaultZone] || {}
        for (const sk of styleKeys)
            if (style[sk] === undefined && z[sk] !== undefined && sk !== "group") style[sk] = z[sk]
        if (style.opacity === undefined) style.opacity = c.theme.opacity ?? 1
        if (style.fontFamily === undefined) style.fontFamily = c.theme.fontFamily ?? ""
        if (style.fontSize === undefined) style.fontSize = c.theme.fontSize ?? 12
        if (style.timerStyle === undefined) style.timerStyle = c.theme.timerStyle ?? "none"
        if (style.width === undefined) style.width = c.width
        if (style.priority === undefined) style.priority = f.urgency === "critical" ? "urgent" : (f.urgency === "low" ? "low" : "normal")
        // media: pictures are shown unless a rule says false; audio/video players only when a rule (or zone) says true, because a notification
        // can name any local file and the player would open it
        style.players = style.media === true
        style.media = style.media !== false
        style.stack = style.stack === true
        // notifications that share a stack key (same group, or same site/app and kind) in one zone are stacked together when `stack` is on
        style.stackKey = !style.stack ? "" : ((style.group ?? "") !== "" ? "g:" + style.group : "a:" + String(f.origin || f.appName || f.desktopEntry).toLowerCase() + "|" + (f.category || ""))
        style.clickThrough = style.clickThrough === true
        style.noFocus = style.noFocus === true
        style.group = style.group ?? ""
        if (out.zone === undefined || !(out.zone in c.zones)) { out.zone = c.defaultZone; by.zone = null }
        if (out.timeout === undefined)           // spec: -1 = server default, 0 = never expire, >0 = milliseconds
            out.timeout = exp === 0 ? 0 : (exp > 0 ? exp : c.defaultTimeout)
        out.silent = out.silent === true
        out.style = style
        out.why = holdWhy(c, st, f, out.zone, out.silent, style.priority)
        out.matched = matched
        out.matchedIdx = matchedIdx          // positions in the rules list (names can repeat or be missing)
        out.decidedIdx = byIdx
        out.decidedBy = by
        return out
    }

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

    // One row per notification on screen. Style comes from decide(); `pinned` and `userWidth` are changed on the popup itself.
    function rowFor(nid, zone, timeout, st) {
        return { nid: nid, zone: zone, timeout: timeout, leaving: false, gen: 0, pinned: false, userWidth: 0,
                 cardWidth: st.width, bgOpacity: st.opacity, clickThrough: st.clickThrough, noFocus: st.noFocus,
                 fontFamily: st.fontFamily, fontSize: st.fontSize, group: st.group, timerStyle: st.timerStyle ?? "none",
                 priority: st.priority, media: st.media, players: st.players === true, stack: st.stack, stackKey: st.stackKey ?? "" }
    }
    function applyStyle(i, st) {
        rows.setProperty(i, "cardWidth", st.width)
        rows.setProperty(i, "bgOpacity", st.opacity)
        rows.setProperty(i, "clickThrough", st.clickThrough)
        rows.setProperty(i, "noFocus", st.noFocus)
        rows.setProperty(i, "fontFamily", st.fontFamily)
        rows.setProperty(i, "fontSize", st.fontSize)
        rows.setProperty(i, "timerStyle", st.timerStyle ?? "none")
        rows.setProperty(i, "group", st.group)
        rows.setProperty(i, "priority", st.priority)
        rows.setProperty(i, "media", st.media)
        rows.setProperty(i, "players", st.players === true)
        rows.setProperty(i, "stack", st.stack)
        rows.setProperty(i, "stackKey", st.stackKey ?? "")
    }
    // Newest first. A notification that belongs to a group goes to the top of that group (groups stay together); otherwise to the top.
    // More important notifications stay above less important ones (a new one goes above older ones of the same importance).
    function addRow(row) {
        let at = -1
        if (row.group !== "")
            for (let i = 0; i < rows.count; i++) {
                const x = rows.get(i)
                if (x.zone === row.zone && x.group === row.group && !x.leaving) { at = i; break }
            }
        if (at < 0) {
            const mine = checks.priorities.indexOf(row.priority)
            at = rows.count
            for (let i = 0; i < rows.count; i++) {
                const x = rows.get(i)
                if (x.zone === row.zone && !x.leaving && checks.priorities.indexOf(x.priority) <= mine) { at = i; break }
            }
        }
        rows.insert(at, row)
    }
    // the first live row of its zone+group is the one that shows the group's heading
    function isGroupHead(nid, zone, group) {
        if (group === "") return false
        for (let i = 0; i < rows.count; i++) {
            const x = rows.get(i)
            if (x.zone === zone && x.group === group && !x.leaving) return x.nid === nid
        }
        return false
    }
    function setPinned(nid, on) {
        const i = rowIndex(nid)
        if (i >= 0) { rows.setProperty(i, "pinned", on === true); touch() }
    }
    function setUserWidth(nid, w) {
        const i = rowIndex(nid)
        if (i >= 0) { rows.setProperty(i, "userWidth", Math.max(0, Math.round(w))); touch() }
    }
    // width of a zone's window: the widest popup it is showing
    function zoneWidth(zone) {
        let w = 0
        for (let i = 0; i < rows.count; i++) {
            const x = rows.get(i)
            if (x.zone !== zone || x.leaving) continue
            w = Math.max(w, x.userWidth > 0 ? x.userWidth : x.cardWidth)
        }
        return Math.max(w, 1)
    }

    function rowIndex(nid) {
        for (let i = 0; i < rows.count; i++) if (rows.get(i).nid === nid) return i
        return -1
    }
    function cap() { return cfg.maxVisible > 0 ? cfg.maxVisible : 1000000 }
    // A "unit" is what takes one place in a zone: a single notification, or a whole stack of them.
    function unitOf(x) { return x.stackKey !== "" ? "s:" + x.stackKey : "n:" + x.nid }
    // position of a notification's unit among the live units of its zone (0 = the newest or most important)
    function rankOf(nid, zone) {
        const seenUnits = {}
        let r = 0
        let mine = ""
        for (let i = 0; i < rows.count; i++) {
            const x = rows.get(i)
            if (x.nid === nid) mine = unitOf(x)
        }
        if (mine === "") return 0
        for (let i = 0; i < rows.count; i++) {
            const x = rows.get(i)
            if (x.zone !== zone || x.leaving) continue
            const u = unitOf(x)
            if (u === mine) return r
            if (!seenUnits[u]) { seenUnits[u] = true; r++ }
        }
        return r
    }

    // Stacks. `expandedStacks` and `stackOffsets` are keyed by zone + stack key; both change only through the functions below.
    property var expandedStacks: ({})
    property var stackOffsets: ({})
    function listMax() { return cap() }                  // an expanded stack shows at most as many notifications as a zone does; the rest scroll
    function stackInfo(nid) {
        let me = null
        for (let i = 0; i < rows.count; i++) if (rows.get(i).nid === nid) { me = rows.get(i); break }
        if (!me || me.stackKey === "") return { idx: 0, count: 1, expanded: false, offset: 0, listMax: listMax(), key: "" }
        let idx = 0, count = 0, found = -1
        for (let i = 0; i < rows.count; i++) {
            const x = rows.get(i)
            if (x.zone !== me.zone || x.stackKey !== me.stackKey || x.leaving) continue
            if (x.nid === nid) found = count
            count++
        }
        idx = found < 0 ? 0 : found
        const k = me.zone + "|" + me.stackKey
        const exp = expandedStacks[k] === true && count > 1
        const maxList = listMax()
        const off = Math.max(0, Math.min(stackOffsets[k] || 0, Math.max(0, count - maxList)))
        return { idx: idx, count: count, expanded: exp, offset: off, listMax: maxList, key: k }
    }
    // forget the expanded / scrolled state of stacks that no longer exist
    function pruneStacks() {
        const live = {}
        for (let i = 0; i < rows.count; i++) {
            const x = rows.get(i)
            if (x.stackKey !== "" && !x.leaving) { const k = x.zone + "|" + x.stackKey; live[k] = (live[k] || 0) + 1 }
        }
        for (const bag of ["expandedStacks", "stackOffsets"]) {
            const cur = root[bag]
            const stale = Object.keys(cur).filter(k => (live[k] || 0) < 2)
            if (stale.length === 0) continue
            const next = Object.assign({}, cur)
            for (const k of stale) delete next[k]
            root[bag] = next
        }
    }
    function toggleStack(zone, stackKey) {
        const k = zone + "|" + stackKey
        const e = Object.assign({}, expandedStacks)
        if (e[k]) delete e[k]; else e[k] = true
        expandedStacks = e
        const o = Object.assign({}, stackOffsets); delete o[k]; stackOffsets = o
        touch()
    }
    function scrollStack(zone, stackKey, delta) {
        const k = zone + "|" + stackKey
        let n = 0
        for (let i = 0; i < rows.count; i++) { const x = rows.get(i); if (x.zone === zone && x.stackKey === stackKey && !x.leaving) n++ }
        const next = Math.max(0, Math.min((stackOffsets[k] || 0) + delta, Math.max(0, n - listMax())))
        if (next === (stackOffsets[k] || 0)) return
        const o = Object.assign({}, stackOffsets); o[k] = next; stackOffsets = o
        touch()
    }
    function dismissStack(zone, stackKey) {
        const ids = []
        for (let i = 0; i < rows.count; i++) { const x = rows.get(i); if (x.zone === zone && x.stackKey === stackKey && !x.leaving) ids.push(x.nid) }
        for (const id of ids) { const n = byId[id]; if (n) { try { n.dismiss() } catch (e) {} } }
    }
    function stackList() {
        const out = {}
        for (let i = 0; i < rows.count; i++) {
            const x = rows.get(i)
            if (x.stackKey === "" || x.leaving) continue
            const k = x.zone + "|" + x.stackKey
            out[k] = out[k] || { zone: x.zone, key: x.stackKey, count: 0, expanded: false }
            out[k].count++
        }
        return Object.keys(out).map(k => Object.assign(out[k], { expanded: expandedStacks[k] === true && out[k].count > 1 })).filter(x => x.count > 1)
    }
    function zoneWantsKeys(zone) {
        for (let i = 0; i < rows.count; i++) {
            const x = rows.get(i)
            if (x.zone !== zone || x.leaving || x.noFocus) continue
            const n = byId[x.nid]
            if (n && n.hasInlineReply) return true
        }
        return false
    }
    function waiting(zone) {
        const units = {}
        let n = 0
        for (let i = 0; i < rows.count; i++) {
            const x = rows.get(i)
            if (x.zone !== zone || x.leaving) continue
            const u = unitOf(x)
            if (!units[u]) { units[u] = true; n++ }
        }
        return Math.max(0, n - cap())
    }

    function show(n) {
        const f = fields(n)
        const r = decide(cfg, compiled, f, n.expireTimeout, liveState())
        if (cfg.debug)
            console.log("notifd: [" + f.appName + "] '" + f.summary + "' -> " + r.zone + " timeout=" + r.timeout + (r.why ? " held: " + r.why : ""))
        n.tracked = true                       // keep it alive until we drop it

        if (rowIndex(n.id) >= 0) return        // already on screen (replacements arrive as property changes, see watchUpdates)

        if (!n.transient) record(f, n, r, r.why)   // transient notifications are not kept
        emit("notification", { id: n.id, app: f.appName, summary: f.summary, body: (f.body || "").slice(0, 300), zone: r.zone,
                                origin: f.origin, urgency: f.urgency, shown: r.why === "", why: r.why, transient: !!n.transient })
        if (r.why !== "") { n.expire(); return }

        byId[n.id] = n
        tx.note(n, f)
        addRow(rowFor(n.id, r.zone, r.timeout, r.style))
        n.closed.connect(() => root.drop(n.id))
        watchUpdates(n)
        touch()
        playSound(n)
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
        const i = rowIndex(n.id)
        if (i < 0 || rows.get(i).leaving) return
        const f = fields(n)
        const r = decide(cfg, compiled, f, n.expireTimeout, liveState())
        rows.setProperty(i, "timeout", r.timeout)
        rows.setProperty(i, "zone", r.zone)
        tx.note(n, f)
        applyStyle(i, r.style)
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
            if (rowIndex(n.id) >= 0) return
            byId[n.id] = n
            addRow(rowFor(n.id, cfg.defaultZone, cfg.defaultTimeout, { width: cfg.width, opacity: 1, clickThrough: false, noFocus: false, fontFamily: "", fontSize: 12, group: "", timerStyle: "none", priority: "normal", media: true, players: false, stack: false, stackKey: "" }))
            n.closed.connect(() => root.drop(n.id))
            watchUpdates(n)
            touch()
        } catch (e) { console.warn("notifd: could not show notification at all:", e) }
    }

    // fade out first, remove once the animation is done
    property var reapQueue: []
    function drop(nid) {
        const i = rowIndex(nid)
        if (i < 0) return
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
                const i = root.rowIndex(nid)
                if (i >= 0 && rows.get(i).leaving) { rows.remove(i); tx.drop(nid); const o = root.byId[nid]; delete root.byId[nid]; if (nid < 0 && o) o.destroy() }
            }
            root.reapQueue = []
            root.pruneStacks()
            root.touch()
        }
    }

    // =====================================================================
    // Preview: show REAL popups using a draft config (or the current one) for a few seconds.
    // Stand-in notification objects feed the same Popup, so what you see is what you get.
    // While a preview runs the draft is the active config; it is restored afterwards.
    // =====================================================================
    property int previewSeq: 0
    property var previewIds: []
    property var previewSaved: null            // { cfg, compiled } to restore when the preview ends
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
            function dismiss() { root.drop(nid) }
            function expire() { root.drop(nid) }
            function sendInlineReply(text) { root.drop(nid) }
        }
    }
    Timer { id: previewTimer; onTriggered: root.endPreview() }

    function previewSamples(c, req) {
        if (Array.isArray(req.samples) && req.samples.length) return req.samples
        const zs = Object.keys(c.zones)
        const out = zs.map(z => ({ appName: c.zones[z].label || z, summary: "Preview: " + (c.zones[z].label || z) + " zone",
                                    body: "This is how notifications in this zone look.", zone: z }))
        out.push({ appName: "Chat app", summary: "Sam", body: "hey, are you free later today?", reply: true, actions: ["Open"], zone: zs[0] })
        out.push({ appName: "Downloads", summary: "Downloading file", body: "55% done", progress: 55, zone: zs[0] })
        return out
    }

    function startPreview(req) {
        endPreview()
        let c = cfg, comp = compiled
        if (req.config !== undefined) {
            const r = checkConfig(JSON.stringify(req.config))
            if (r.error) return { error: "config: " + r.error }
            c = r.cfg
            comp = compile(c)
        }
        const secs = Math.max(1, Math.min(30, Number(req.seconds) || 6))
        const calm = { dnd: false, muted: [], focus: "", auto: { fullscreen: false, share: false } }   // previews ignore Do Not Disturb
        previewSaved = { cfg: cfg, compiled: compiled }
        cfg = c
        compiled = comp
        const ids = []
        for (const s of previewSamples(c, req)) {
            const f = { appName: String(s.appName ?? "Preview"), desktopEntry: String(s.desktopEntry ?? ""), origin: String(s.origin ?? ""),
                        category: String(s.category ?? ""), channel: String(s.channel ?? (s.origin ? "web" : "app")),
                        summary: String(s.summary ?? ""), body: String(s.body ?? ""),
                        urgency: ["low", "normal", "critical"].indexOf(s.urgency) >= 0 ? s.urgency : "normal" }
            const d = decide(c, comp, f, -1, calm)
            const zone = (s.zone && (s.zone in c.zones)) ? s.zone : d.zone
            const nid = -(++previewSeq)
            byId[nid] = fakeComp.createObject(root, {
                nid: nid, appName: f.appName, desktopEntry: f.desktopEntry, summary: f.summary, body: f.body,
                hints: typeof s.progress === "number" ? { value: s.progress } : {},
                actions: (s.actions || []).map(t => ({ text: String(t), invoke: () => {} })),
                hasInlineReply: s.reply === true, inlineReplyPlaceholder: s.reply === true ? "Reply…" : ""
            })
            if (typeof s.progress === "number") tx.seed(nid, s.progress)
            addRow(rowFor(nid, zone, secs * 1000, d.style))
            ids.push(nid)
        }
        previewIds = ids
        previewTimer.interval = secs * 1000 + 400
        previewTimer.restart()
        touch()
        return { ok: true, shown: ids.length, seconds: secs }
    }

    function endPreview() {
        const was = previewIds.length > 0 || previewSaved !== null
        previewTimer.stop()
        for (const id of previewIds) drop(id)
        previewIds = []
        if (previewSaved) { cfg = previewSaved.cfg; compiled = previewSaved.compiled; previewSaved = null }
        touch()
        if (was) emit("preview", { state: "ended" })
    }

    // =====================================================================
    // Sound: honours the sound-file / sound-name / suppress-sound hints.
    // =====================================================================
    readonly property string soundScript:
        'k=$1; v=$2; ' +
        'if [ "$k" = file ]; then ' +
        '  command -v pw-play >/dev/null 2>&1 && exec pw-play "$v"; ' +
        '  command -v paplay >/dev/null 2>&1 && exec paplay "$v"; ' +
        'else ' +
        '  command -v canberra-gtk-play >/dev/null 2>&1 && exec canberra-gtk-play -i "$v"; ' +
        'fi; exit 0'
    Component { id: soundComp; Process { onExited: destroy() } }

    function playSound(n) {
        if (cfg.sound.enabled === false) return
        const h = n.hints || {}
        if (h["suppress-sound"] === true) return
        let kind = "", val = ""
        if (h["sound-file"]) {
            val = String(h["sound-file"]).replace(/^file:\/\//, "")
            if (val.startsWith("/")) kind = "file"
        } else if (h["sound-name"]) {
            val = String(h["sound-name"])
            if (/^[A-Za-z0-9._-]+$/.test(val)) kind = "name"
        } else if (cfg.sound.defaultName && /^[A-Za-z0-9._-]+$/.test(cfg.sound.defaultName)) {
            kind = "name"; val = cfg.sound.defaultName
        }
        if (kind === "") return
        soundComp.createObject(root, { command: ["sh", "-c", soundScript, "notifd-sound", kind, val], running: true })
    }

    // =====================================================================
    // Confirmation pill (DND / mute / config errors); shown even during DND
    // =====================================================================
    property string osdText: ""
    property color osdColor: "#34d399"
    property bool osdShown: false
    function flash(text, color, ms) {
        osdText = text
        osdColor = color
        osdShown = true
        osdTimer.interval = ms || 1600
        osdTimer.restart()
    }
    Timer { id: osdTimer; interval: 1600; onTriggered: root.osdShown = false }

    // =====================================================================
    // Control API. One implementation (api*), two front doors:
    //   - `qs ipc call notifd <fn>`   (strings in, JSON string out; used by scripts)
    //   - the control socket          (JSON lines, request/response plus pushed events)
    // The api* functions return plain objects and throw Error(message) on failure.
    // =====================================================================
    readonly property string daemonVersion: "0.10.0"
    readonly property int protocolVersion: 1
    property string rawConfig: ""

    function fail(code, message) { const e = new Error(message); e.code = code; throw e }

    function apiStatus() {
        const per = {}                // notifications per zone
        const units = {}              // zone -> [[unit, notifications in it]] in display order
        let onScreen = 0, queued = 0
        for (let i = 0; i < rows.count; i++) {
            const x = rows.get(i)
            if (x.leaving) continue
            per[x.zone] = (per[x.zone] || 0) + 1
            const list = units[x.zone] = units[x.zone] || []
            const u = unitOf(x)
            const hit = list.find(e => e[0] === u)
            if (hit) hit[1]++; else list.push([u, 1])
        }
        // a stack takes one place (or, expanded, up to a list's worth); places beyond the cap wait
        for (const z in units) {
            units[z].forEach((e, i) => {
                if (i >= cap()) { queued++; return }
                const k = z + "|" + e[0].slice(2)
                onScreen += (e[0].startsWith("s:") && expandedStacks[k] === true && e[1] > 1) ? Math.min(e[1], listMax()) : 1
            })
        }
        return { dnd: ctl.dnd, muted: ctl.muted, zones: Object.keys(cfg.zones), active: onScreen, queued: queued,
                 focus: ctl.focus in cfg.focus.modes ? ctl.focus : "", perZone: per, stacks: stackList(), cap: cap(), history: history.length, cfgError: cfgError, auto: auto, holding: holdReason(),
                 previewing: previewIds.length > 0 }
    }
    function apiConfigStatus() {
        return { ok: cfgError === "", error: cfgError, rules: cfg.rules.length, zones: Object.keys(cfg.zones),
                 screenFound: cfg.screen === "" || Quickshell.screens.some(s => s.name === cfg.screen) }
    }
    function apiConfigGet() { return { raw: rawConfig, effective: cfg, error: cfgError } }
    function apiDnd(mode) {
        const on = mode === "on" ? true : mode === "off" ? false : mode === "toggle" ? !ctl.dnd : null
        if (on === null) fail("bad_params", "mode must be on, off or toggle")
        ctl = Object.assign({}, ctl, { dnd: on })
        saveSoon()
        flash(on ? "Do Not Disturb on" : "Do Not Disturb off", on ? "#fbbf24" : "#34d399")
        return apiStatus()
    }
    function apiMute(zone, on) {
        if (!(zone in cfg.zones)) fail("bad_params", "unknown zone '" + zone + "'")
        const m = ctl.muted.filter(z => z !== zone)
        if (on) m.push(zone)
        ctl = Object.assign({}, ctl, { muted: m })
        saveSoon()
        flash((cfg.zones[zone].label || zone) + (on ? " muted" : " unmuted"), cfg.zones[zone].accent || "#60a5fa")
        return apiStatus()
    }
    function apiFocus(mode) {
        const m = String(mode ?? "")
        if (m === "off" || m === "") ctl = Object.assign({}, ctl, { focus: "" })
        else {
            if (!(m in cfg.focus.modes)) fail("bad_params", "unknown focus mode '" + m + "' (have: " + Object.keys(cfg.focus.modes).join(", ") + ")")
            ctl = Object.assign({}, ctl, { focus: m })
        }
        saveSoon()
        flash(ctl.focus === "" ? "Focus off" : "Focus: " + (cfg.focus.modes[ctl.focus].label || ctl.focus), ctl.focus === "" ? "#34d399" : "#a78bfa")
        scheduleState()
        return apiStatus()
    }
    // action: toggle (expand or collapse), clear (dismiss every notification in the stack) or scroll (delta lines)
    function apiStack(zone, key, action, delta) {
        if (!(zone in cfg.zones)) fail("bad_params", "unknown zone '" + zone + "'")
        if (action === "toggle") toggleStack(zone, key)
        else if (action === "clear") dismissStack(zone, key)
        else if (action === "scroll") scrollStack(zone, key, Number(delta) || 0)
        else fail("bad_params", "action must be toggle, clear or scroll")
        return apiStatus()
    }
    function apiSamples(nid) { return tx.copyOf(nid) }
    function apiSetAuto(kind, on) {
        if (kind !== "fullscreen" && kind !== "share") fail("bad_params", "kind must be fullscreen or share")
        setAuto(kind, on)
        return apiStatus()
    }
    function apiReply(nid, text) {
        const n = byId[nid]
        if (!n) fail("not_found", "no such notification on screen")
        if (!n.hasInlineReply) fail("bad_params", "that notification does not accept replies")
        n.sendInlineReply(text)
        return { ok: true }
    }
    function apiValidate(text) {
        const r = checkConfig(text)
        return r.error ? { ok: false, error: r.error } : { ok: true, rules: r.cfg.rules.length, zones: Object.keys(r.cfg.zones) }
    }
    // req: { config?: {...}, notification: {appName, desktopEntry, origin, summary, body, urgency, expireTimeout}, ignoreState?: bool }
    function apiSimulate(req) {
        const n = req.notification || {}
        let c = cfg, comp = compiled
        if (req.config !== undefined) {
            const r = checkConfig(JSON.stringify(req.config))
            if (r.error) fail("bad_params", "config: " + r.error)
            c = r.cfg
            comp = compile(c)
        }
        const f = { appName: String(n.appName ?? ""), desktopEntry: String(n.desktopEntry ?? ""), origin: String(n.origin ?? ""),
                    category: String(n.category ?? ""), channel: String(n.channel ?? (n.origin ? "web" : "app")),
                    summary: String(n.summary ?? ""), body: String(n.body ?? ""),
                    urgency: ["low", "normal", "critical"].indexOf(n.urgency) >= 0 ? n.urgency : "normal" }
        // ignoreState shows the rule outcome alone; a `focus` name in the request tries a focus mode on top of that
        const st = req.ignoreState === true ? { dnd: false, muted: [], focus: String(req.focus ?? ""), auto: { fullscreen: false, share: false } } : liveState()
        const d = decide(c, comp, f, typeof n.expireTimeout === "number" ? n.expireTimeout : -1, st)
        return { ok: true, zone: d.zone, timeout: d.timeout, silent: d.silent, wouldShow: d.why === "",
                 hold: d.why, matched: d.matched, decidedBy: d.decidedBy, style: d.style,
                 matchedIdx: d.matchedIdx, decidedIdx: d.decidedIdx }
    }
    // req: { config?: {...}, samples?: [{appName, summary, body, urgency, zone?, progress?, reply?, actions?}], seconds?: 6 }
    function apiPreview(req) {
        const r = startPreview(req)
        if (r.error) fail("bad_params", r.error)
        emit("preview", { state: "started", shown: r.shown, seconds: r.seconds })
        return r
    }
    function apiEndPreview() { endPreview(); return apiStatus() }
    function apiHistory(limit) { return history.slice(0, Math.max(0, Math.min(1000, Number(limit) || 0))) }
    function apiSeenApps() {
        return Object.keys(seen).map(k => {
            const e = seen[k]
            const types = Object.keys(e.types || {}).map(t => Object.assign({ key: t }, e.types[t])).sort((x, y) => y.count - x.count)
            return { key: k, app: e.app, desktopEntry: e.desktopEntry, count: e.count, last: e.last, types: types }
        }).sort((a, b) => b.last - a.last)
    }
    function apiClearHistory() { history = []; saveSoon(); scheduleState(); return apiStatus() }
    function apiDismissAll() {
        // only notifications that are still live: one that is already fading out is closed and must not be closed again
        const ids = []
        for (let i = 0; i < rows.count; i++) { const x = rows.get(i); if (!x.leaving) ids.push(x.nid) }
        for (const id of ids) { const n = byId[id]; if (n) { try { n.dismiss() } catch (e) {} } }
        return apiStatus()
    }

    function ipcJson(fn) {
        try { return JSON.stringify(fn()) } catch (e) { return JSON.stringify({ error: e.message }) }
    }
    function parseReq(text) {
        try { return JSON.parse(text) } catch (e) { fail("bad_request", "request is not valid JSON") }
    }

    IpcHandler {
        target: "notifd"
        function status(): string { return root.ipcJson(() => root.apiStatus()) }
        function configStatus(): string { return root.ipcJson(() => root.apiConfigStatus()) }
        function dnd(mode: string): string { return root.ipcJson(() => root.apiDnd(mode)) }
        function mute(zone: string, on: bool): string { return root.ipcJson(() => root.apiMute(zone, on)) }
        function focus(mode: string): string { return root.ipcJson(() => root.apiFocus(mode)) }
        function stack(zone: string, key: string, action: string, delta: int): string { return root.ipcJson(() => root.apiStack(zone, key, action, delta)) }
        function samples(nid: int): string { return root.ipcJson(() => root.apiSamples(nid)) }
        function setAuto(kind: string, on: bool): string { return root.ipcJson(() => root.apiSetAuto(kind, on)) }
        function reply(nid: int, text: string): string { return root.ipcJson(() => root.apiReply(nid, text)) }
        function validate(text: string): string { return root.ipcJson(() => root.apiValidate(text)) }
        function simulate(text: string): string { return root.ipcJson(() => root.apiSimulate(root.parseReq(text))) }
        function preview(text: string): string { return root.ipcJson(() => root.apiPreview(root.parseReq(text))) }
        function endPreview(): string { return root.ipcJson(() => root.apiEndPreview()) }
        function history(limit: int): string { return root.ipcJson(() => root.apiHistory(limit)) }
        function seenApps(): string { return root.ipcJson(() => root.apiSeenApps()) }
        function clearHistory(): string { return root.ipcJson(() => root.apiClearHistory()) }
        function dismissAll(): string { return root.ipcJson(() => root.apiDismissAll()) }
    }

    // =====================================================================
    // Control socket: JSON lines over a Unix socket.
    //   server -> client first:  {"event":"hello","protocol":1,"daemon":"...","pid":N,"seq":N}
    //   client -> server:        {"id":1,"method":"status","params":{}}
    //   server -> client:        {"id":1,"result":{...}}  or  {"id":1,"error":{"code":"...","message":"..."}}
    //   after {"method":"subscribe"} the server also pushes events:
    //     state, notification, updated, closed, config, preview    (each with "seq" and "t")
    // The socket lives in $XDG_RUNTIME_DIR (mode 0700), so only your user can reach it.
    // =====================================================================
    // No $XDG_RUNTIME_DIR means no safe per-user place for the socket (a shared /tmp would let other users reach it), so then there is no control socket at all.
    readonly property string sockPath: Quickshell.env("NOTIFD_SOCKET") || (Quickshell.env("XDG_RUNTIME_DIR") ? Quickshell.env("XDG_RUNTIME_DIR") + "/notifd.sock" : "")
    readonly property int maxConnections: 16
    readonly property int maxLine: 1048576
    property var conns: []
    property int eventSeq: 0

    function sendLine(sock, obj) {
        try { sock.write(JSON.stringify(obj) + "\n"); sock.flush() } catch (e) {}
    }
    function emit(event, data) {
        if (conns.length === 0) return
        const msg = JSON.stringify(Object.assign({ event: event, seq: ++eventSeq, t: Date.now() }, data)) + "\n"
        for (const c of conns) {
            if (!c.subscribed) continue
            try { c.write(msg); c.flush() } catch (e) {}
        }
    }
    Timer { id: stateTimer; interval: 100; onTriggered: root.emit("state", root.apiStatus()) }
    function scheduleState() { if (conns.length > 0 && !stateTimer.running) stateTimer.start() }
    onCtlChanged: scheduleState()
    onAutoChanged: scheduleState()

    function dispatch(sock, m, p) {
        switch (m) {
        case "ping": return { pong: true, t: Date.now() }
        case "status": return apiStatus()
        case "configStatus": return apiConfigStatus()
        case "configGet": return apiConfigGet()
        case "dnd": return apiDnd(p.mode)
        case "mute": return apiMute(p.zone, p.on === true)
        case "focus": return apiFocus(p.mode)
        case "samples": return apiSamples(p.id)
        case "stack": return apiStack(p.zone, String(p.key ?? ""), p.action, p.delta)
        case "setAuto": return apiSetAuto(p.kind, p.on === true)
        case "history": return apiHistory(p.limit ?? 200)
        case "seenApps": return apiSeenApps()
        case "clearHistory": return apiClearHistory()
        case "dismissAll": return apiDismissAll()
        case "reply": return apiReply(p.id, String(p.text ?? ""))
        case "validate": return apiValidate(p.config !== undefined ? JSON.stringify(p.config) : String(p.text ?? ""))
        case "simulate": return apiSimulate(p)
        case "preview": return apiPreview(p)
        case "endPreview": return apiEndPreview()
        case "subscribe": sock.subscribed = true; return { status: apiStatus(), seq: eventSeq }
        case "unsubscribe": sock.subscribed = false; return {}
        default: fail("unknown_method", "unknown method '" + m + "'")
        }
    }
    function handleLine(sock, line) {
        if (line.length > maxLine) { sendLine(sock, { id: null, error: { code: "too_large", message: "request too large" } }); return }
        if (line.trim() === "") return
        let req
        try { req = JSON.parse(line) } catch (e) { sendLine(sock, { id: null, error: { code: "bad_request", message: "not valid JSON" } }); return }
        if (req === null || typeof req !== "object" || typeof req.method !== "string") {
            sendLine(sock, { id: (req && req.id !== undefined) ? req.id : null, error: { code: "bad_request", message: "need an object with a string \"method\"" } })
            return
        }
        const id = req.id === undefined ? null : req.id
        try {
            sendLine(sock, { id: id, result: dispatch(sock, req.method, (req.params && typeof req.params === "object") ? req.params : {}) })
        } catch (e) {
            sendLine(sock, { id: id, error: { code: e.code || "failed", message: String(e.message) } })
        }
    }

    Component.onCompleted: if (sockPath === "") console.warn("notifd: XDG_RUNTIME_DIR is not set; the control socket is disabled (notifications still work)")
    SocketServer {
        active: root.sockPath !== ""
        path: root.sockPath
        handler: Socket {
            id: sock
            property bool subscribed: false
            property bool greeted: false
            parser: SplitParser { onRead: line => root.handleLine(sock, line) }
            // greet once the connection is really attached (writing during construction is lost)
            function greet() {
                if (greeted || !connected) return
                greeted = true
                if (root.conns.length >= root.maxConnections) {
                    root.sendLine(sock, { event: "error", code: "too_many_connections", message: "too many control connections" })
                    Qt.callLater(() => { sock.connected = false })
                    return
                }
                root.conns = root.conns.concat([sock])
                root.sendLine(sock, { event: "hello", protocol: root.protocolVersion, daemon: root.daemonVersion,
                                      pid: Quickshell.processId, seq: root.eventSeq })
            }
            Component.onCompleted: Qt.callLater(greet)
            onConnectedChanged: {
                if (connected) greet()
                else root.conns = root.conns.filter(c => c !== sock)
            }
        }
    }

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

    ZoneWindows { root: root }
    Osd { root: root }
}
