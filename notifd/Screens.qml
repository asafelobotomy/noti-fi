import QtQuick
import Quickshell

// Which screen things appear on. The OSD and the confirmation pill: on Hyprland the focused monitor, else the configured `screen`,
// else the first. Popups and the notification list: the configured `screen` ("focused" = Hyprland's focused monitor), else the
// first. Bindings over Quickshell.screens, so an unplugged monitor is simply not chosen any more (null only with no screen at all).
Scope {
    id: scr
    required property var shell
    readonly property bool hyprland: (Quickshell.env("HYPRLAND_INSTANCE_SIGNATURE") ?? "") !== ""
    LazyLoader { id: focus; active: scr.hyprland; source: "FocusedMonitor.qml" }
    readonly property string focusedName: focus.item ? focus.item.name : ""
    readonly property var popups: {
        const list = Quickshell.screens, want = scr.shell.cfg.screen
        return (want === "focused" ? list.find(s => s.name === scr.focusedName) : list.find(s => s.name === want)) ?? list[0] ?? null
    }
    readonly property var osd: {
        const list = Quickshell.screens
        return list.find(s => s.name === scr.focusedName) ?? list.find(s => s.name === scr.shell.cfg.screen) ?? list[0] ?? null
    }
}
