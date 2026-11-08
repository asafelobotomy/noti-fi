import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland

// What Hyprland tells notifd. Loaded only on Hyprland (see Screens.qml), so other desktops never touch its IPC.
//  - name: the focused monitor.
//  - screencast: something is capturing the screen. Hyprland sends "screencast>>1,..." and ">>0,..."; a recorder that copies
//    frame by frame (wf-recorder, OBS's wlr capture) flips it every frame, so it only goes off after 1.5 s without a frame.
//  - fullscreen: the focused window is full screen (not just maximised), read from `hyprctl activewindow` whenever focus or
//    full-screen state changes.
Scope {
    id: watch
    readonly property string name: Hyprland.focusedMonitor ? Hyprland.focusedMonitor.name : ""
    property bool screencast: false
    property bool fullscreen: false

    Connections {
        target: Hyprland
        function onRawEvent(event) {
            if (event.name === "screencast") {
                if (String(event.data).startsWith("1")) { watch.screencast = true; castOff.stop() }
                else castOff.restart()
            } else if (event.name === "fullscreen" || event.name === "activewindow" || event.name === "workspace" || event.name === "focusedmon" || event.name === "closewindow") {
                if (!probe.running) probe.running = true; else probe.again = true
            }
        }
    }
    Timer { id: castOff; interval: 1500; onTriggered: watch.screencast = false }
    Process {
        id: probe
        property bool again: false
        command: ["hyprctl", "activewindow", "-j"]
        // fullscreen: 0 none, 1 maximised, 2 full screen (older versions: true/false)
        stdout: StdioCollector {
            onStreamFinished: {
                let w = {}
                try { w = JSON.parse(text) } catch (e) {}
                watch.fullscreen = w.fullscreen === 2 || w.fullscreen === true
            }
        }
        onExited: if (again) { again = false; running = true }
    }
    Component.onCompleted: probe.running = true
}
