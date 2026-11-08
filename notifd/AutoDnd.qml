import QtQuick
import Quickshell
import Quickshell.Io

// Automatic Do Not Disturb: watches for a full-screen window and for screen sharing, and sets shell.auto through shell.setAuto().
// On Plasma: a KWin script (full screen) and KWin's capture stream in PipeWire (sharing). On Hyprland: its own events
// (HyprlandWatch.qml); the PipeWire watch runs there too.
Scope {
    id: autoDnd
    required property var shell          // cfg, auto, setAuto(), screens.hypr
    readonly property bool watching: shell.cfg.autoDnd.watch !== false      // off in tests, which drive the state by hand
    readonly property var hypr: shell.screens.hypr
    property bool pwShare: false
    readonly property bool shareNow: pwShare || (hypr !== null && hypr.screencast === true)
    readonly property bool hyprFull: hypr !== null && hypr.fullscreen === true
    onShareNowChanged: if (watching && shell.cfg.autoDnd.screenShare) shell.setAuto("share", shareNow)
    onHyprFullChanged: if (watching && shell.cfg.autoDnd.fullscreen) shell.setAuto("fullscreen", hyprFull)

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
    // Screen sharing: a PipeWire video stream produced by KWin means something is capturing the screen. `pw-dump --monitor`
    // prints the whole graph, then an array of changes whenever something changes (a removed object has "info": null), so a
    // share is seen the moment its stream appears, not on a poll. Prints 1 or 0 on every change of state (0 if pw-dump fails).
    readonly property string shareScript:
        "pw-dump --monitor 2>/dev/null | python3 -u -c '" +
        "import json,sys\n" +
        "nodes={}; last=None; buf=[]\n" +
        "def hit():\n" +
        "  for p in nodes.values():\n" +
        "    who=\" \".join(str(p.get(k,\"\")) for k in (\"application.name\",\"application.process.binary\",\"node.name\",\"media.name\")).lower()\n" +
        "    c=str(p.get(\"media.class\",\"\"))\n" +
        "    if \"Video\" in c and c.startswith(\"Stream/Output\") and \"kwin\" in who: return 1\n" +
        "  return 0\n" +
        "for line in sys.stdin:\n" +
        "  buf.append(line)\n" +
        "  if line.rstrip() != \"]\": continue\n" +
        "  try: d=json.loads(\"\".join(buf))\n" +
        "  except Exception: d=[]\n" +
        "  buf=[]\n" +
        "  for o in d:\n" +
        "    if o.get(\"info\") is None: nodes.pop(o.get(\"id\"),None)\n" +
        "    elif o.get(\"type\")==\"PipeWire:Interface:Node\": nodes[o.get(\"id\")]=(o[\"info\"].get(\"props\") or nodes.get(o.get(\"id\")) or {})\n" +
        "  h=hit()\n" +
        "  if h!=last: last=h; print(h)\n" +
        "if last is None: print(0)'"
    Process {
        id: shareWatch
        command: ["sh", "-c", autoDnd.shareScript]
        stdout: SplitParser { onRead: line => autoDnd.pwShare = line.trim() === "1" }
    }
    Timer {
        // keeps the watchers alive (respawns journalctl or pw-dump if one ever exits)
        interval: 4000; running: true; repeat: true; triggeredOnStart: true
        onTriggered: {
            const s = autoDnd.shell
            if (s.cfg.autoDnd.watch === false) return      // watchers off (tests drive the state by hand)
            if (s.cfg.autoDnd.fullscreen && !kwinWatch.running && autoDnd.hypr === null) kwinWatch.running = true
            if (s.cfg.autoDnd.screenShare && !shareWatch.running) shareWatch.running = true
            if (!s.cfg.autoDnd.screenShare && shareWatch.running) shareWatch.running = false
            if (!s.cfg.autoDnd.fullscreen && s.auto.fullscreen) s.setAuto("fullscreen", false)
            if (!s.cfg.autoDnd.screenShare && s.auto.share) s.setAuto("share", false)
        }
    }
}
