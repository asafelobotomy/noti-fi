import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// How long a popup stays, as named choices in seconds (the draft keeps milliseconds as text; "" = not set here, "0" = until closed).
// "Custom" opens a slider with stops at the usual values.
ColumnLayout {
    id: root
    property string value: ""
    property string unsetLabel: "App's own"      // what "" means at this level
    property string unsetTip: "Use the time the app asks for, or the default"
    property bool allowUnset: true               // false where a value is required (the default for every popup)
    signal edited(string text)
    spacing: Kirigami.Units.smallSpacing

    readonly property var presets: (root.allowUnset ? [{ value: "", label: root.unsetLabel, tip: root.unsetTip }] : []).concat([{ value: "3000", label: "Glance 3 s" }, { value: "6000", label: "Read 6 s" },
                                    { value: "15000", label: "Long 15 s" }, { value: "0", label: "Until closed", tip: "Stays until you close it" }])
    readonly property bool isPreset: root.presets.some(p => p.value === root.value)
    property bool customOpen: !isPreset

    Segmented {
        Layout.fillWidth: true
        accessibleName: "Stays on screen"
        options: root.presets.concat([{ value: "custom", label: root.isPreset ? "Custom…" : "Custom: " + Math.round(Number(root.value) / 1000) + " s" }])
        value: root.isPreset && !root.customOpen ? root.value : "custom"
        onPicked: v => { if (v === "custom") root.customOpen = true; else { root.customOpen = false; root.edited(v) } }
    }
    RowLayout {
        visible: root.customOpen
        QQC2.Slider {
            id: slider
            Layout.preferredWidth: Kirigami.Units.gridUnit * 14
            // the stops: 1–10 s by 1, then 15, 20, 30, 45, 60, 90, 120
            readonly property var stops: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 15, 20, 30, 45, 60, 90, 120]
            from: 0; to: stops.length - 1; stepSize: 1; snapMode: QQC2.Slider.SnapAlways
            Binding on value { when: !slider.pressed; value: { const s = Math.round(Number(root.value) / 1000); let i = slider.stops.findIndex(x => x >= s); return i < 0 ? slider.stops.length - 1 : i } }
            onMoved: root.edited(String(stops[Math.round(value)] * 1000))
            Accessible.name: "Custom time in seconds"
        }
        QQC2.Label { text: slider.stops[Math.round(slider.value)] + " s" }
    }
}
