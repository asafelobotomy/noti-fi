import QtQuick
import Quickshell
import Quickshell.Io

// Control socket: JSON lines over a Unix socket (protocol: docs/protocol.md).
//   server -> client first:  {"event":"hello","protocol":1,"daemon":"...","pid":N,"seq":N}
//   client -> server:        {"id":1,"method":"status","params":{}}
//   server -> client:        {"id":1,"result":{...}}  or  {"id":1,"error":{"code":"...","message":"..."}}
//   after {"method":"subscribe"} the server also pushes events:
//     state, notification, updated, closed, config, preview    (each with "seq" and "t")
// The socket lives in $XDG_RUNTIME_DIR (mode 0700), so only your user can reach it.
Scope {
    id: control
    required property var shell          // api, daemonVersion, protocolVersion

    // No $XDG_RUNTIME_DIR means no safe per-user place for the socket (a shared /tmp would let other users reach it), so then there is no control socket at all.
    readonly property string sockPath: Quickshell.env("NOTIFD_SOCKET") || (Quickshell.env("XDG_RUNTIME_DIR") ? Quickshell.env("XDG_RUNTIME_DIR") + "/notifd.sock" : "")
    readonly property int maxConnections: 16
    readonly property int maxLine: 1048576
    property var conns: []
    property int eventSeq: 0

    function sendLine(sock, obj) {
        try { sock.write(JSON.stringify(obj) + "\n"); sock.flush() } catch (e) {}
    }
    function emit(event, data) {
        if (conns.length === 0) return
        const msg = JSON.stringify(Object.assign({ event: event, seq: ++eventSeq, t: Date.now() }, data)) + "\n"
        for (const c of conns) {
            if (!c.subscribed) continue
            try { c.write(msg); c.flush() } catch (e) {}
        }
    }
    // state events are coalesced to at most one per 100 ms
    Timer { id: stateTimer; interval: 100; onTriggered: control.emit("state", control.shell.api.status()) }
    function scheduleState() { if (conns.length > 0 && !stateTimer.running) stateTimer.start() }

    function dispatch(sock, m, p) {
        const a = control.shell.api
        switch (m) {
        case "ping": return { pong: true, t: Date.now() }
        case "status": return a.status()
        case "configStatus": return a.configStatus()
        case "configGet": return a.configGet()
        case "dnd": return a.dnd(p.mode)
        case "mute": return a.mute(p.zone, p.on === true)
        case "focus": return a.focus(p.mode)
        case "samples": return a.samples(p.id)
        case "stack": return a.stack(p.zone, String(p.key ?? ""), p.action, p.delta)
        case "setAuto": return a.setAuto(p.kind, p.on === true)
        case "history": return a.history(p.limit ?? 200)
        case "seenApps": return a.seenApps()
        case "clearHistory": return a.clearHistory()
        case "dismissAll": return a.dismissAll()
        case "center": return a.center(p.action)
        case "markRead": return a.markRead(Array.isArray(p.ids) ? p.ids : null)
        case "removeHistory": return a.removeHistory(p.id, p.t)
        case "reply": return a.reply(p.id, String(p.text ?? ""))
        case "invoke": return a.invoke(p.id, String(p.key ?? ""))
        case "validate": return a.validate(p.config !== undefined ? JSON.stringify(p.config) : String(p.text ?? ""))
        case "simulate": return a.simulate(p)
        case "preview": return a.preview(p)
        case "endPreview": return a.endPreview()
        case "subscribe": sock.subscribed = true; return { status: a.status(), seq: eventSeq }
        case "unsubscribe": sock.subscribed = false; return {}
        default: a.fail("unknown_method", "unknown method '" + m + "'")
        }
    }
    function handleLine(sock, line) {
        if (line.length > maxLine) { sendLine(sock, { id: null, error: { code: "too_large", message: "request too large" } }); return }
        if (line.trim() === "") return
        let req
        try { req = JSON.parse(line) } catch (e) { sendLine(sock, { id: null, error: { code: "bad_request", message: "not valid JSON" } }); return }
        if (req === null || typeof req !== "object" || typeof req.method !== "string") {
            sendLine(sock, { id: (req && req.id !== undefined) ? req.id : null, error: { code: "bad_request", message: "need an object with a string \"method\"" } })
            return
        }
        const id = req.id === undefined ? null : req.id
        try {
            sendLine(sock, { id: id, result: dispatch(sock, req.method, (req.params && typeof req.params === "object") ? req.params : {}) })
        } catch (e) {
            sendLine(sock, { id: id, error: { code: e.code || "failed", message: String(e.message) } })
        }
    }

    Component.onCompleted: if (sockPath === "") console.warn("notifd: XDG_RUNTIME_DIR is not set; the control socket is disabled (notifications still work)")
    SocketServer {
        active: control.sockPath !== ""
        path: control.sockPath
        handler: Socket {
            id: sock
            property bool subscribed: false
            property bool greeted: false
            parser: SplitParser { onRead: line => control.handleLine(sock, line) }
            // greet once the connection is really attached (writing during construction is lost)
            function greet() {
                if (greeted || !connected) return
                greeted = true
                if (control.conns.length >= control.maxConnections) {
                    control.sendLine(sock, { event: "error", code: "too_many_connections", message: "too many control connections" })
                    Qt.callLater(() => { sock.connected = false })
                    return
                }
                control.conns = control.conns.concat([sock])
                control.sendLine(sock, { event: "hello", protocol: control.shell.protocolVersion, daemon: control.shell.daemonVersion,
                                         pid: Quickshell.processId, seq: control.eventSeq })
            }
            Component.onCompleted: Qt.callLater(greet)
            onConnectedChanged: {
                if (connected) greet()
                else control.conns = control.conns.filter(c => c !== sock)
            }
        }
    }
}
