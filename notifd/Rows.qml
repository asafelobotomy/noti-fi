import QtQuick
import Quickshell

// The on-screen rows (shell.rowList; ListModels can't hold QObjects, so rows carry the id and shell.byId maps it to the
// Notification object): where a new one goes, how a zone ranks its popups, stacks, and how wide a zone's window is.
Scope {
    id: layout
    required property var shell          // rowList, byId, cfg, validator; touch()
    readonly property var rows: layout.shell.rowList

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
            const mine = layout.shell.validator.priorities.indexOf(row.priority)
            at = rows.count
            for (let i = 0; i < rows.count; i++) {
                const x = rows.get(i)
                if (x.zone === row.zone && !x.leaving && layout.shell.validator.priorities.indexOf(x.priority) <= mine) { at = i; break }
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
        if (i >= 0) { rows.setProperty(i, "pinned", on === true); layout.shell.touch() }
    }
    function setUserWidth(nid, w) {
        const i = rowIndex(nid)
        if (i >= 0) { rows.setProperty(i, "userWidth", Math.max(0, Math.round(w))); layout.shell.touch() }
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
    function cap() { return layout.shell.cfg.maxVisible > 0 ? layout.shell.cfg.maxVisible : 1000000 }
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
            const cur = layout[bag]
            const stale = Object.keys(cur).filter(k => (live[k] || 0) < 2)
            if (stale.length === 0) continue
            const next = Object.assign({}, cur)
            for (const k of stale) delete next[k]
            layout[bag] = next
        }
    }
    function toggleStack(zone, stackKey) {
        const k = zone + "|" + stackKey
        const e = Object.assign({}, expandedStacks)
        if (e[k]) delete e[k]; else e[k] = true
        expandedStacks = e
        const o = Object.assign({}, stackOffsets); delete o[k]; stackOffsets = o
        layout.shell.touch()
    }
    function scrollStack(zone, stackKey, delta) {
        const k = zone + "|" + stackKey
        let n = 0
        for (let i = 0; i < rows.count; i++) { const x = rows.get(i); if (x.zone === zone && x.stackKey === stackKey && !x.leaving) n++ }
        const next = Math.max(0, Math.min((stackOffsets[k] || 0) + delta, Math.max(0, n - listMax())))
        if (next === (stackOffsets[k] || 0)) return
        const o = Object.assign({}, stackOffsets); o[k] = next; stackOffsets = o
        layout.shell.touch()
    }
    function dismissStack(zone, stackKey) {
        const ids = []
        for (let i = 0; i < rows.count; i++) { const x = rows.get(i); if (x.zone === zone && x.stackKey === stackKey && !x.leaving) ids.push(x.nid) }
        for (const id of ids) { const n = layout.shell.byId[id]; if (n) { try { n.dismiss() } catch (e) {} } }
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
            const n = layout.shell.byId[x.nid]
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
}
