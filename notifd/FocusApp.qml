import QtQuick
import Quickshell.Io

// Brings an app's window forward after one of its notification actions ran (View, Open, Activate ...). The app asks the
// compositor to raise it with an xdg-activation token, but the notification server never passes one along, so Hyprland
// ignores the request or only marks the window urgent. So find the window by the notification's desktop entry or app name
// and focus it. Hyprland only; a no-op elsewhere or when no window matches.
Item {
    id: focus
    visible: false
    property var names: []          // the lowercased candidates of the request being served

    // `notif` is a Notification (desktopEntry, appName).
    function raise(notif) {
        if (!notif || probe.running) return
        const cands = [notif.desktopEntry, notif.appName]
            .map(s => String(s || "").toLowerCase().replace(/\.desktop$/, "").trim())
            .filter(s => s !== "")
        if (cands.length === 0) return
        focus.names = cands
        probe.running = true
    }

    // The address of the first window whose class or initial class equals a candidate (case-insensitive; a reverse-DNS
    // class also matches by its last part), or "".
    function pick(clients, cands) {
        const last = s => s.split(".").pop()
        for (const c of clients) {
            const ids = [c.class, c.initialClass].map(s => String(s || "").toLowerCase()).filter(s => s !== "")
            for (const n of cands)
                for (const id of ids)
                    if (id === n || last(id) === n || id === last(n)) return String(c.address || "")
        }
        return ""
    }

    Process {
        id: probe
        command: ["sh", "-c", "[ -n \"$HYPRLAND_INSTANCE_SIGNATURE\" ] && exec hyprctl clients -j"]
        stdout: StdioCollector {
            onStreamFinished: {
                let list = []
                try { list = JSON.parse(text) } catch (e) { return }
                const addr = focus.pick(list, focus.names)
                if (addr !== "") { go.command = ["hyprctl", "dispatch", "focuswindow", "address:" + addr]; go.running = true }
            }
        }
    }
    Process { id: go }
}
