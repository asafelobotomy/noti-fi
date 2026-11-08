import QtQuick

// Notifications kept alive after their popup has left (it timed out, or Do Not Disturb or a mute held it), so their
// buttons still work from the notification center, as in Plasma. The app is told the notification is closed only when
// you remove it from the list, clear the list, invoke one of its buttons, or it falls out of history; or when more than
// `center.keep` are kept (the oldest is closed first). Transient notifications are never kept.
QtObject {
    id: kept
    required property var shell

    property var byId: ({})        // notification id -> Notification
    property int rev: 0            // bumped on every change (byId is changed in place)
    readonly property real startedAt: Date.now()   // ids start again at 1 when the daemon restarts: older history entries are not live
    readonly property int cap: { const c = shell.cfg.center; return c && typeof c.keep === "number" ? Math.max(0, Math.floor(c.keep)) : 50 }
    readonly property int count: { rev; return Object.keys(byId).length }

    // The live notification behind a history entry (kept, or still on screen), or null.
    function live(entry) {
        rev; shell.topo
        if (!entry || !(entry.t >= startedAt)) return null
        return byId[entry.id] || shell.byId[entry.id] || null
    }
    // Keep it; false when it cannot be kept (then the caller closes it as before).
    function keep(n) {
        if (!n || !(n.id > 0) || n.transient || cap <= 0) return false
        byId[n.id] = n
        const ids = Object.keys(byId).map(Number).sort((a, b) => a - b)
        while (ids.length > cap) close(ids.shift(), false)
        rev++
        return true
    }
    // It was closed (by the app, or by us): forget it.
    function release(id) {
        if (!(id in byId)) return
        delete byId[id]
        rev++
    }
    // Close one: dismissed (the user removed it) or expired (pushed out).
    function close(id, dismissed) {
        const n = byId[id]
        if (!n) return
        delete byId[id]
        rev++
        try { if (dismissed) n.dismiss(); else n.expire() } catch (e) {}
    }
    function closeAll(dismissed) { for (const id of Object.keys(byId)) close(Number(id), dismissed) }
    // Close the ones whose history entry is gone (pushed out by newer notifications).
    function prune() {
        const ids = Object.keys(byId)
        if (ids.length === 0) return
        const inHistory = {}
        for (const h of shell.history) if (h.t >= startedAt) inHistory[h.id] = true
        for (const id of ids) if (!inHistory[id]) close(Number(id), false)
    }
}
