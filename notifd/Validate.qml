import QtQuick

// Checks a configuration and says what is wrong with it: pure functions and the lists they use, kept apart from the daemon proper.
// (The same rules are enforced by the `notify rules` linter, the JSON schema and the typed config in the Rust crates; keep them in step.)
QtObject {
    // nine fixed places on a 3x3 grid, or "free": a position (x, y as 0..1 of the screen) chosen by the user
    readonly property var anchorNames: ["top-left", "top-center", "top-right", "middle-left", "center", "middle-right",
                                        "bottom-left", "bottom-center", "bottom-right", "free"]
    // keep in sync with MATCH_FIELDS in the `notify` CLI
    readonly property var matchFields: ["appName", "desktopEntry", "origin", "summary", "body", "urgency", "category", "channel", "group"]
    // Patterns run on text that apps and web pages control. A pattern that backtracks badly could stall the thread that draws every popup, so they are kept short.
    readonly property int maxPattern: 500
    // how important a notification is; an app's urgency maps to low / normal / urgent, a rule can set any of the four
    readonly property var priorities: ["low", "normal", "high", "urgent"]
    readonly property var charts: ["bars", "line", "heatmap"]
    readonly property var corners: anchorNames.filter(a => a !== "free")   // where the notification center can open

    // "" when the style keys of a rule/zone are valid, otherwise why not
    function styleError(o, allowGroup) {
        if (o.opacity !== undefined && (typeof o.opacity !== "number" || !(o.opacity >= 0.1 && o.opacity <= 1))) return "opacity must be a number from 0.1 to 1"
        for (const k of ["clickThrough", "noFocus"])
            if (o[k] !== undefined && typeof o[k] !== "boolean") return k + " must be true or false"
        if (o.width !== undefined && (typeof o.width !== "number" || !(o.width >= 120 && o.width <= 4000))) return "width must be a number from 120 to 4000"
        if (o.fontFamily !== undefined && (typeof o.fontFamily !== "string" || o.fontFamily.length > 100)) return "fontFamily must be a font name"
        if (o.timerStyle !== undefined && (typeof o.timerStyle !== "string" || o.timerStyle.split(",").some(t => ["none", "line", "circle", "frame"].indexOf(t.trim()) < 0)))
            return "timerStyle must be none, or any of line, circle, frame separated by commas"
        if (o.fontSize !== undefined && (typeof o.fontSize !== "number" || !(o.fontSize >= 6 && o.fontSize <= 48))) return "fontSize must be a number from 6 to 48"
        if (o.group !== undefined && (typeof o.group !== "string" || o.group.length > 60)) return "group must be a short name"
        if (o.priority !== undefined && priorities.indexOf(o.priority) < 0) return "priority must be " + priorities.join(", ")
        for (const k of ["media", "stack"])
            if (o[k] !== undefined && typeof o[k] !== "boolean") return k + " must be true or false"
        if (!allowGroup && o.group !== undefined) return "group can only be set on a rule"
        return ""
    }

    function focusError(f) {
        if (!f || typeof f.modes !== "object" || f.modes === null || Array.isArray(f.modes)) return "modes must be an object"
        for (const m in f.modes) {
            const o = f.modes[m]
            if (!o || typeof o !== "object") return "mode '" + m + "' must be an object"
            if (!/^[A-Za-z0-9_-]{1,40}$/.test(m)) return "mode '" + m + "': the name may only use letters, digits, - and _"
            if (o.label !== undefined && typeof o.label !== "string") return "mode '" + m + "': label must be text"
            if (o.minPriority !== undefined && priorities.indexOf(o.minPriority) < 0) return "mode '" + m + "': minPriority must be " + priorities.join(", ")
            if (o.allow !== undefined) {
                if (!Array.isArray(o.allow)) return "mode '" + m + "': allow must be a list"
                for (const a of o.allow) {
                    if (!a || typeof a !== "object" || Array.isArray(a)) return "mode '" + m + "': each allow entry must be an object of patterns"
                    for (const k in a) {
                        if (matchFields.indexOf(k) < 0) return "mode '" + m + "': unknown match field '" + k + "'"
                        if (String(a[k]).length > maxPattern) return "mode '" + m + "': the pattern for " + k + " is longer than " + maxPattern + " characters"
                        try { new RegExp(a[k], "i") } catch (e) { return "mode '" + m + "': bad regex for " + k }
                    }
                }
            }
        }
        return ""
    }

    // returns "" when valid, otherwise a short human-readable reason
    function validate(c) {
        const zones = c.zones
        if (!zones || typeof zones !== "object" || Array.isArray(zones) || Object.keys(zones).length === 0)
            return "zones: need at least one zone"
        for (const z in zones) {
            const zz = zones[z]
            if (!zz || anchorNames.indexOf(zz.anchor) < 0)
                return "zone '" + z + "': anchor must be " + anchorNames.join(", ")
            if (zz.anchor === "free")
                for (const k of ["x", "y"])
                    if (typeof zz[k] !== "number" || !(zz[k] >= 0 && zz[k] <= 1))
                        return "zone '" + z + "': a free zone needs " + k + " between 0 and 1"
            const se = styleError(zz, false)
            if (se !== "") return "zone '" + z + "': " + se
            if (zz.accent !== undefined && !/^#[0-9a-fA-F]{3,8}$/.test(String(zz.accent)))
                return "zone '" + z + "': accent must be a #hex colour"
        }
        if (!(c.defaultZone in zones)) return "defaultZone '" + c.defaultZone + "' is not a defined zone"
        for (const k of ["defaultTimeout", "width", "margin", "spacing", "maxVisible"])
            if (typeof c[k] !== "number" || !(c[k] >= 0)) return k + " must be a number >= 0"
        const te = styleError({ opacity: c.theme.opacity, fontFamily: c.theme.fontFamily, fontSize: c.theme.fontSize, timerStyle: c.theme.timerStyle }, false)
        if (te !== "") return "theme: " + te
        if (typeof c.theme.tokens !== "string" || c.theme.tokens.length > 500) return "theme.tokens must be \"auto\", \"\" or a file path"
        if (typeof c.theme.radius !== "number" || !(c.theme.radius >= 0 && c.theme.radius <= 40)) return "theme.radius must be a number from 0 to 40"
        const fe = focusError(c.focus)
        if (fe !== "") return "focus: " + fe
        if (!c.transfer || charts.indexOf(c.transfer.chart) < 0) return "transfer.chart must be " + charts.join(", ")
        if (!c.center || corners.indexOf(c.center.anchor) < 0) return "center.anchor must be " + corners.join(", ")
        if (!Number.isInteger(c.center.keep) || c.center.keep < 0 || c.center.keep > 1000) return "center.keep must be a whole number from 0 to 1000"
        for (const k of ["fullscreen", "screenShare"])
            if (typeof c.autoDnd[k] !== "boolean") return "autoDnd." + k + " must be true or false"
        if (c.autoDnd.watch !== undefined && typeof c.autoDnd.watch !== "boolean") return "autoDnd.watch must be true or false"
        if (!Array.isArray(c.rules)) return "rules must be a list"
        for (let i = 0; i < c.rules.length; i++) {
            const r = c.rules[i]
            const name = (r && r.name) ? r.name : "#" + (i + 1)
            if (!r || typeof r !== "object") return "rule " + name + ": must be an object"
            if (r.zone !== undefined && !(r.zone in zones)) return "rule '" + name + "': unknown zone '" + r.zone + "'"
            if (r.zone !== undefined && r.match && r.match.group !== undefined) return "rule '" + name + "': a rule for a group cannot move it to another group"
            if (r.timeout !== undefined && (typeof r.timeout !== "number" || r.timeout < 0)) return "rule '" + name + "': timeout must be a number >= 0"
            const se = styleError(r, true)
            if (se !== "") return "rule '" + name + "': " + se
            if (r.match !== undefined && (typeof r.match !== "object" || r.match === null)) return "rule '" + name + "': match must be an object"
            for (const f in (r.match || {})) {
                if (matchFields.indexOf(f) < 0) return "rule '" + name + "': unknown match field '" + f + "'"
                if (String(r.match[f]).length > maxPattern) return "rule '" + name + "': the pattern for " + f + " is longer than " + maxPattern + " characters"
                try { new RegExp(r.match[f], "i") }
                catch (e) { return "rule '" + name + "': bad regex for " + f }
            }
        }
        return ""
    }
}
