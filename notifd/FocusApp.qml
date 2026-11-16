import QtQuick
import Quickshell.Io

// Brings an app's window forward after one of its notification actions ran (View, Open, Activate ...). The app asks the
// compositor to raise it with an xdg-activation token, but the notification server never passes one along, so Hyprland
// ignores the request or only marks the window urgent. So find the window and focus it. Hyprland only; a no-op elsewhere or
// when no window matches.
//  - Candidates for the window's class: the notification's desktop entry, app name and icon name, plus the desktop file's
//    StartupWMClass and the command it runs (read from the file), so an app whose class differs from its name still matches.
//  - The focus is sent as the Lua dispatcher (`hl.dsp.focus`) that a Lua config (uQ's hyprland.lua) understands; the older
//    `focuswindow` dispatcher is tried when that is refused.
Item {
    id: focus
    visible: false
    property var names: []          // the lowercased candidates of the request being served

    function norm(list) {
        return list.map(s => String(s || "").toLowerCase().replace(/\.desktop$/, "").trim()).filter(s => s !== "")
    }

    // `notif` is a Notification (desktopEntry, appName, appIcon).
    function raise(notif) {
        if (!notif || meta.running || probe.running) return
        const cands = focus.norm([notif.desktopEntry, notif.appName, notif.appIcon])
        if (cands.length === 0) return
        focus.names = cands
        const entry = String(notif.desktopEntry || "").replace(/\.desktop$/, "")
        if (/^[A-Za-z0-9._-]+$/.test(entry)) { meta.command = ["sh", "-c", focus.metaScript, "sh", entry]; meta.running = true }
        else probe.running = true
    }

    // StartupWMClass and the executable's name from the entry's .desktop file; nothing when there is none.
    readonly property string metaScript: 'for d in "$HOME/.local/share/applications" /usr/local/share/applications /usr/share/applications ' +
        '"$HOME/.local/share/flatpak/exports/share/applications" /var/lib/flatpak/exports/share/applications; do ' +
        'f="$d/$1.desktop"; if [ -f "$f" ]; then sed -n "s/^StartupWMClass=//p" "$f" | head -n 1; ' +
        'sed -n "s/^Exec=\\([^ ]*\\).*/\\1/p" "$f" | head -n 1 | sed "s|.*/||"; break; fi; done'

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
        id: meta
        stdout: StdioCollector {
            onStreamFinished: {
                focus.names = focus.names.concat(focus.norm(text.split("\n")))
                probe.running = true
            }
        }
    }
    Process {
        id: probe
        command: ["sh", "-c", "[ -n \"$HYPRLAND_INSTANCE_SIGNATURE\" ] && exec hyprctl clients -j"]
        stdout: StdioCollector {
            onStreamFinished: {
                let list = []
                try { list = JSON.parse(text) } catch (e) { return }
                const addr = focus.pick(list, focus.names)
                // the address comes from Hyprland; refuse anything that isn't 0x and hex before it goes into a command
                if (!/^0x[0-9a-f]+$/i.test(addr)) return
                go.command = ["sh", "-c", 'hyprctl dispatch "hl.dsp.focus({ window = \\"address:$1\\" })" | grep -qi "^ok" || hyprctl dispatch focuswindow "address:$1"', "sh", addr]
                go.running = true
            }
        }
    }
    Process { id: go }
}
