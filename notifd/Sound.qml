import QtQuick
import Quickshell
import Quickshell.Io

// Sound: honours the sound-file / sound-name / suppress-sound hints, and the config's default sound.
Scope {
    id: sound
    required property var shell          // cfg

    readonly property string script:
        'k=$1; v=$2; ' +
        'if [ "$k" = file ]; then ' +
        '  command -v pw-play >/dev/null 2>&1 && exec pw-play "$v"; ' +
        '  command -v paplay >/dev/null 2>&1 && exec paplay "$v"; ' +
        'else ' +
        '  command -v canberra-gtk-play >/dev/null 2>&1 && exec canberra-gtk-play -i "$v"; ' +
        'fi; exit 0'
    Component { id: soundComp; Process { onExited: destroy() } }

    function play(n) {
        const c = sound.shell.cfg
        if (c.sound.enabled === false) return
        const h = n.hints || {}
        if (h["suppress-sound"] === true) return
        let kind = "", val = ""
        if (h["sound-file"]) {
            val = String(h["sound-file"]).replace(/^file:\/\//, "")
            if (val.startsWith("/")) kind = "file"
        } else if (h["sound-name"]) {
            val = String(h["sound-name"])
            if (/^[A-Za-z0-9._-]+$/.test(val)) kind = "name"
        } else if (c.sound.defaultName && /^[A-Za-z0-9._-]+$/.test(c.sound.defaultName)) {
            kind = "name"; val = c.sound.defaultName
        }
        if (kind === "") return
        soundComp.createObject(sound, { command: ["sh", "-c", script, "notifd-sound", kind, val], running: true })
    }
}
