import QtQuick
import Quickshell
import Quickshell.Io

// Automatic Do Not Disturb: watches for a full-screen window and for screen sharing, and sets shell.auto through shell.setAuto().
Scope {
    id: autoDnd
    required property var shell          // cfg, auto, setAuto()

    // Full-screen state comes from a small KWin script (installed by `notify autodnd`) that logs
    // "notifd-autodnd: fullscreen=1|0". We follow that line in the journal. journald stamps the
    // sending unit itself, so other apps cannot fake it.
    Process {
        id: kwinWatch
        command: ["journalctl", "--user", "-b", "-u", "plasma-kwin_wayland.service", "-n", "1", "-f", "-o", "cat", "--grep", "notifd-autodnd: fullscreen="]
        stdout: SplitParser {
            onRead: line => {
                const m = /notifd-autodnd: fullscreen=([01])/.exec(line)
                if (m) autoDnd.shell.setAuto("fullscreen", m[1] === "1")
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
        "  who=\" \".join(str(p.get(k,\"\")) for k in (\"application.name\",\"application.process.binary\",\"node.name\",\"media.name\")).lower()\n" +
        "  if \"Video\" in str(p.get(\"media.class\",\"\")) and p.get(\"media.class\",\"\").startswith(\"Stream/Output\") and \"kwin\" in who: hit=1\n" +
        "print(hit)'"
    Process {
        id: shareProbe
        command: ["sh", "-c", autoDnd.shareScript]
        stdout: StdioCollector { onStreamFinished: autoDnd.shell.setAuto("share", text.trim() === "1") }
    }
    Timer {
        // keeps the watchers alive (respawns journalctl if it ever exits) and polls for screen sharing
        interval: 4000; running: true; repeat: true; triggeredOnStart: true
        onTriggered: {
            const s = autoDnd.shell
            if (s.cfg.autoDnd.watch === false) return      // watchers off (tests drive the state by hand)
            if (s.cfg.autoDnd.fullscreen && !kwinWatch.running) kwinWatch.running = true
            if (s.cfg.autoDnd.screenShare && !shareProbe.running) shareProbe.running = true
            if (!s.cfg.autoDnd.fullscreen && s.auto.fullscreen) s.setAuto("fullscreen", false)
            if (!s.cfg.autoDnd.screenShare && s.auto.share) s.setAuto("share", false)
        }
    }
}
