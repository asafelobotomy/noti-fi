import QtQuick

// decide(): the ONE place that turns "a notification" into "what happens to it" (real notifications, `notify simulate` and preview
// all use it, so what the simulator says is exactly what will happen).
//   c    config            comp  compiled rules for c
//   f    fields (see fields())
//   exp  the app's requested timeout (-1 default, 0 never, >0 ms)
//   st   { dnd, muted[], auto: { fullscreen, share }, focus, inhibited }
//
// Groups (the config's "zones"): every notification ends up in exactly one. Two passes over the rules, in order:
//   1. the group: the first matching rule that names one (rules that test the group itself take no part);
//   2. everything else (timeout, silent, look): the first matching rule that sets it, where a `group` test is checked against the
//      group from pass 1. So a rule can apply to a whole group, or to a single app, website or kind.
// The system's own notifications (the kernel, systemd, power, disks, network, polkit...) are always in the System group, are never
// silenced by a rule, a muted group or a focus mode, and only Do Not Disturb holds them (critical ones still pass): they are how the
// system reaches the user.
QtObject {
    id: route
    property var shell           // holdFor(), focusHold(), styleKeys, maxSubject

    readonly property string systemGroup: "system"
    readonly property var coreApps: /^(kernel|linux( kernel)?|systemd(-[a-z0-9]+)?|udisks2?|upower|polkit(-[a-z0-9-]+)?|networkmanager|nm-applet|powerdevil|org_kde_powerdevil|earlyoom|smartd|kded[0-9]*|kwin(_wayland|_x11)?)$/i
    readonly property var coreEntries: /^(org\.kde\.(powerdevil|kded[0-9]*|kwin)|org\.freedesktop\.(udisks2|upower|networkmanager|systemd1|policykit1)|nm-applet)$/i
    function isCore(f) { return route.coreApps.test(String(f.appName || "")) || route.coreEntries.test(String(f.desktopEntry || "")) }

    function holdWhy(c, st, f, zone, silent, priority, core) {
        const passCritical = priority === "urgent" && c.dnd.allowCritical !== false
        if (silent && !core) return "silent rule"
        const hold = route.shell.holdFor(c, st)
        if (hold !== "" && !passCritical) return hold
        if (core) return ""
        const fh = route.shell.focusHold(c, st, Object.assign({}, f, { group: zone }), priority)   // a focus mode can allow a whole group
        if (fh !== "") return fh
        if (st.muted.indexOf(zone) >= 0 && !passCritical) return "muted " + zone
        return ""
    }

    function decide(c, comp, f, exp, st) {
        const sk_ = route.shell.styleKeys, max = route.shell.maxSubject
        const out = { zone: undefined, timeout: undefined, silent: undefined }
        const style = {}
        const by = { zone: null, timeout: null, silent: null }
        const matched = [], matchedIdx = []
        const byIdx = { zone: null, timeout: null, silent: null }
        for (const sk of sk_) { by[sk] = null; byIdx[sk] = null }
        const core = route.isCore(f) && route.systemGroup in c.zones
        // does rule k match? `group` is a test on the group decided in pass 1 (undefined during pass 1: such rules are skipped there)
        function matches(k, group) {
            try {
                for (const t of k.tests) {
                    if (t[0] === "group") { if (group === undefined || !t[1].test(group)) return false }
                    else if (!t[1].test(String(f[t[0]] ?? "").slice(0, max))) return false
                }
                return true
            } catch (e) { return false }
        }
        // pass 1: the group
        if (core) { out.zone = route.systemGroup; by.zone = "System (built in)" }
        else for (let i = 0; i < comp.length; i++) {
            const k = comp[i]
            if (k.r.zone === undefined || k.tests.some(t => t[0] === "group") || !matches(k, undefined)) continue
            out.zone = k.r.zone; by.zone = k.r.name || ("#" + (i + 1)); byIdx.zone = i
            break
        }
        if (out.zone === undefined || !(out.zone in c.zones)) { out.zone = c.defaultZone; by.zone = null; byIdx.zone = null }
        // pass 2: everything else, with the group known
        for (let i = 0; i < comp.length; i++) {
            const k = comp[i]
            if (!matches(k, out.zone)) continue
            const name = k.r.name || ("#" + (i + 1))
            matched.push(name)
            matchedIdx.push(i)
            if (out.timeout === undefined && k.r.timeout !== undefined) { out.timeout = k.r.timeout; by.timeout = name; byIdx.timeout = i }
            if (out.silent === undefined && k.r.silent !== undefined) { out.silent = k.r.silent; by.silent = name; byIdx.silent = i }
            for (const sk of sk_)
                if (style[sk] === undefined && k.r[sk] !== undefined) { style[sk] = k.r[sk]; by[sk] = name; byIdx[sk] = i }
        }
        // then the group's own style, then the theme and global width
        const z = c.zones[out.zone] || {}
        for (const sk of sk_)
            if (style[sk] === undefined && z[sk] !== undefined && sk !== "group") style[sk] = z[sk]
        if (style.opacity === undefined) style.opacity = c.theme.opacity ?? 1
        if (style.fontFamily === undefined) style.fontFamily = c.theme.fontFamily ?? ""
        if (style.fontSize === undefined) style.fontSize = c.theme.fontSize ?? 12
        if (style.timerStyle === undefined) style.timerStyle = c.theme.timerStyle ?? "none"
        if (style.width === undefined) style.width = c.width
        if (style.priority === undefined) style.priority = f.urgency === "critical" ? "urgent" : (f.urgency === "low" ? "low" : "normal")
        // media: pictures are shown unless a rule says false; audio/video players only when a rule (or group) says true, because a
        // notification can name any local file and the player would open it
        style.players = style.media === true
        style.media = style.media !== false
        style.stack = style.stack === true
        // notifications that share a stack key (same heading, or same site/app and kind) in one group are stacked together when `stack` is on
        style.stackKey = !style.stack ? "" : ((style.group ?? "") !== "" ? "g:" + style.group : "a:" + String(f.origin || f.appName || f.desktopEntry).toLowerCase() + "|" + (f.category || ""))
        style.clickThrough = style.clickThrough === true
        style.noFocus = style.noFocus === true
        style.group = style.group ?? ""
        if (out.timeout === undefined)           // spec: -1 = server default, 0 = never expire, >0 = milliseconds
            out.timeout = exp === 0 ? 0 : (exp > 0 ? exp : c.defaultTimeout)
        out.silent = out.silent === true && !core
        out.style = style
        out.why = route.holdWhy(c, st, f, out.zone, out.silent, style.priority, core)
        out.matched = matched
        out.matchedIdx = matchedIdx          // positions in the rules list (names can repeat or be missing)
        out.decidedIdx = byIdx
        out.decidedBy = by
        out.core = core
        return out
    }
}
