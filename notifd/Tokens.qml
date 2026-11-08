import QtQuick
import Quickshell
import Quickshell.Io

// A desktop's design tokens as the base of notifd's theme (theme.tokens): "auto" = uQ's tokens.json when it exists, a path = that
// file, "" = none. Settings in rules.json's `theme` still win over the tokens. The file is read again every 2 s rather than
// watched: uQ replaces its whole theme folder at once, which leaves a watch on the old file stale. A file that is missing,
// unreadable or not valid keeps the last good tokens.
Scope {
    id: tokens
    required property var shell          // cfg; reapply() when the tokens change

    property var theme: ({})             // the theme keys taken from the tokens: card, border, text, ... (empty = none)
    property string lastText: ""
    readonly property string setting: { const t = shell.cfg.theme; return t && typeof t.tokens === "string" ? t.tokens : "auto" }
    readonly property string path: {
        if (setting === "") return ""
        if (setting !== "auto") return setting.startsWith("~/") ? (Quickshell.env("HOME") ?? "") + setting.slice(1) : setting
        const state = Quickshell.env("XDG_STATE_HOME") || ((Quickshell.env("HOME") ?? "") + "/.local/state")
        return state + "/uq/theme/tokens.json"
    }
    onPathChanged: { lastText = ""; if (path === "" && Object.keys(theme).length > 0) { theme = ({}); shell.reapply() } else file.reload() }

    // uQ's token names -> notifd's theme keys; anything that is not a plain colour, size or name is ignored
    function map(t) {
        const out = {}
        const hex = v => typeof v === "string" && /^#[0-9a-fA-F]{3,8}$/.test(v)
        const c = t.colors || {}
        for (const [k, from] of [["card", "surface"], ["border", "border"], ["text", "text"], ["subtext", "text_muted"], ["button", "surface_alt"], ["buttonHover", "overlay"]])
            if (hex(c[from])) out[k] = c[from]
        const f = t.fonts || {}
        if (typeof f.ui === "string" && f.ui.length > 0 && f.ui.length < 100) out.fontFamily = f.ui
        if (typeof f.size === "number" && f.size >= 6 && f.size <= 40) out.fontSize = f.size
        const u = t.ui || {}
        if (typeof u.radius === "number" && u.radius >= 0 && u.radius <= 40) out.radius = u.radius
        return out
    }
    function take(text) {
        if (text === lastText) return
        let parsed
        try { parsed = JSON.parse(text) } catch (e) { return }       // half-written or broken: keep the last good tokens
        if (parsed === null || typeof parsed !== "object") return
        lastText = text
        theme = map(parsed)
        shell.reapply()
    }

    FileView {
        id: file
        path: tokens.path
        blockLoading: false
        printErrors: false
        onLoaded: tokens.take(text())
    }
    Timer { interval: 2000; repeat: true; running: tokens.path !== ""; onTriggered: file.reload() }
}
