import QtQuick
import Quickshell.Hyprland

// The monitor Hyprland has focused. Loaded only on Hyprland (see Screens.qml), so other desktops never touch its IPC.
QtObject {
    readonly property string name: Hyprland.focusedMonitor ? Hyprland.focusedMonitor.name : ""
}
