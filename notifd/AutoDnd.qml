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
    property var readers: []              // programs reading a KWin screen capture right now ("?" = not yet known)
    // captures by programs in autoDnd.ignoreCaptureBy (a screen-reading assistant, a remote-support tool you trust) do not count
    readonly property var ignored: (shell.cfg.autoDnd.ignoreCaptureBy || []).map(x => String(x).toLowerCase())
    readonly property bool pwShare: readers.some(r => ignored.indexOf(r.toLowerCase()) < 0)
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
    // share is seen the moment its stream appears, not on a poll. On every change it prints, as a JSON list, the programs reading
    // those streams (found through the links from KWin's stream to their own); "?" for a stream nobody reads yet. [] = none.
    readonly property string shareScript:
        "pw-dump --monitor 2>/dev/null | python3 -u -c '" +
        "import json,sys\n" +
        "objs={}; last=None; buf=[]\n" +
        "def props(o): return (o.get(\"info\") or {}).get(\"props\") or {}\n" +
        "def readers():\n" +
        "  out=set()\n" +
        "  for i,o in objs.items():\n" +
        "    p=props(o); c=str(p.get(\"media.class\",\"\"))\n" +
        "    who=\" \".join(str(p.get(k,\"\")) for k in (\"application.name\",\"application.process.binary\",\"node.name\",\"media.name\")).lower()\n" +
        "    if o.get(\"type\")!=\"PipeWire:Interface:Node\" or not c.startswith(\"Stream/Output/Video\") or \"kwin\" not in who: continue\n" +
        "    names=set()\n" +
        "    for l in objs.values():\n" +
        "      li=l.get(\"info\") or {}\n" +
        "      if l.get(\"type\")!=\"PipeWire:Interface:Link\" or li.get(\"output-node-id\")!=i: continue\n" +
        "      n=objs.get(li.get(\"input-node-id\"),{}); cl=objs.get(props(n).get(\"client.id\"),{}); cp=props(cl)\n" +
        "      names.add(str(cp.get(\"application.process.binary\") or cp.get(\"application.name\") or props(n).get(\"node.name\") or \"?\"))\n" +
        "    out|=names or {\"?\"}\n" +
        "  return sorted(out)\n" +
        "for line in sys.stdin:\n" +
        "  buf.append(line)\n" +
        "  if line.rstrip() != \"]\": continue\n" +
        "  try: d=json.loads(\"\".join(buf))\n" +
        "  except Exception: d=[]\n" +
        "  buf=[]\n" +
        "  for o in d:\n" +
        "    if o.get(\"info\") is None: objs.pop(o.get(\"id\"),None)\n" +
        "    else: objs[o.get(\"id\")]=o\n" +
        "  r=readers()\n" +
        "  if r!=last: last=r; print(json.dumps(r))\n" +
        "if last is None: print(\"[]\")'"
    Process {
        id: shareWatch
        command: ["sh", "-c", autoDnd.shareScript]
        stdout: SplitParser {
            onRead: line => {
                let r = []
                try { r = JSON.parse(line) } catch (e) {}
                autoDnd.readers = Array.isArray(r) ? r.map(String) : []
            }
        }
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
