import QtQuick
import Quickshell
import Quickshell.Io

// Configuration: defaults + rules.json (hot-reloaded, validated). A file that fails validation is rejected whole; the last good
// config keeps running and an error pill is shown. Notifications are never dropped because of a bad config.
// The live config is shell.cfg / shell.compiled; this file only computes and applies it.
Scope {
    id: config
    required property var shell          // cfg, compiled, cfgError, rawConfig, validator; emit(), flash(), touch()

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
        // the notification center (the list the tray icon opens) sits in this corner, next to the tray
        center: { anchor: "bottom-right", keep: 50 },
        zones: { personal: { anchor: "top-right", accent: "#60a5fa", label: "Personal" } },
        rules: []
    })

    function merge(user) {
        const c = Object.assign({}, defaults, user)
        if (!c.zones || !("system" in c.zones)) c.zones = Object.assign({}, c.zones, { system: { anchor: "bottom-right", accent: "#fbbf24", label: "System" } })
        for (const k of ["dnd", "history", "sound", "autoDnd", "theme", "focus", "transfer", "center"])
            c[k] = Object.assign({}, defaults[k], (user && typeof user[k] === "object" && user[k]) || {})
        return c
    }

    function compile(c) {
        return c.rules.map(r => ({
            r: r,
            tests: Object.keys(r.match || {}).map(k => [k, new RegExp(r.match[k], "i")])
        }))
    }

    // parse + merge + validate a config text WITHOUT applying it: { cfg } or { error }
    function check(text) {
        let user
        try { user = JSON.parse(text) } catch (e) { return { error: "not valid JSON" } }
        if (user === null || typeof user !== "object" || Array.isArray(user)) return { error: "top level must be an object" }
        const merged = merge(user)
        const err = config.shell.validator.validate(merged)
        return err !== "" ? { error: err } : { cfg: merged }
    }

    function reject(msg) {
        const s = config.shell
        s.cfgError = msg
        console.warn("notifd: rules.json rejected, keeping previous config:", msg)
        s.emit("config", { ok: false, error: msg, rules: s.cfg.rules.length, zones: Object.keys(s.cfg.zones) })
        s.flash("rules.json: " + msg.slice(0, 80), "#f87171", 5000)
    }

    function apply(text) {
        const s = config.shell
        const r = check(text)
        if (r.error) { reject(r.error); return }
        s.cfg = r.cfg
        s.compiled = compile(r.cfg)
        s.cfgError = ""
        s.touch()
        s.emit("config", { ok: true, error: "", rules: s.cfg.rules.length, zones: Object.keys(s.cfg.zones) })
        console.log("notifd: loaded", r.cfg.rules.length, "rules,", Object.keys(r.cfg.zones).length, "zones")
    }

    FileView {
        path: Qt.resolvedUrl("rules.json")
        watchChanges: true
        onFileChanged: reload()
        onLoaded: { config.shell.rawConfig = text(); config.apply(text()) }
    }
}
