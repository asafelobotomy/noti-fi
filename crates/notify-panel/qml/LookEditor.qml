import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import "Json.js" as Json

// How popups look and behave at one level (a zone, an app, a type of notification, a rule). Pick a ready-made style, then adjust it
// with choices beside a live preview. The breadcrumb shows the levels a popup's look passes through; anything not changed here is
// inherited from the level before, and "Reset this level" clears every change made here. Edits are emitted, never applied here.
ColumnLayout {
    id: root
    property var values: ({})                // this level's settings ("" = inherit)
    property string inheritFrom: "the theme"
    property var levels: ["Theme"]           // the breadcrumb, ending with this level
    property bool showGroup: true
    property var groups: []                  // group names already in use, offered in the group picker
    property var model                       // the panel model (for the theme)
    property string accent: ""
    property int timeoutMs: -1               // -1 = the default timeout
    signal edited(string key, string text)
    spacing: Kirigami.Units.largeSpacing

    readonly property var settingsValues: Json.parse(root.model ? root.model.settingsJson : "", ({})).values || ({})
    readonly property var base: ({
        card: settingsValues["theme.card"], border: settingsValues["theme.border"], text: settingsValues["theme.text"], subtext: settingsValues["theme.subtext"],
        button: settingsValues["theme.button"], opacity: settingsValues["theme.opacity"], fontFamily: settingsValues["theme.fontFamily"],
        fontSize: settingsValues["theme.fontSize"], width: settingsValues["width"], timerStyle: settingsValues["theme.timerStyle"]
    })
    function val(k) { return root.values && root.values[k] !== undefined && root.values[k] !== null ? String(root.values[k]) : "" }

    // ---- ready-made styles: each sets the look keys it names and clears the others
    readonly property var lookKeys: ["opacity", "width", "fontSize", "timerStyle", "media", "priority", "clickThrough"]
    readonly property var presets: [
        { id: "theme", label: "As inherited", tip: "No changes here", set: {} },
        { id: "compact", label: "Compact", tip: "Small text, narrow, a footer line", set: { fontSize: "11", width: "320", timerStyle: "line", media: "false" } },
        { id: "glass", label: "Glass", tip: "See-through, clicks pass to the window below", set: { opacity: "0.8", clickThrough: "true", timerStyle: "line" } },
        { id: "loud", label: "Loud", tip: "Large, wide, framed, marked important", set: { fontSize: "15", width: "460", timerStyle: "frame", priority: "high" } },
        { id: "minimal", label: "Minimal", tip: "No indicator, no pictures", set: { timerStyle: "none", media: "false", width: "340" } }
    ]
    readonly property string currentPreset: { for (const p of root.presets) if (root.lookKeys.every(k => (p.set[k] || "") === root.val(k))) return p.id; return "" }
    function applyPreset(p) { for (const k of root.lookKeys) if ((p.set[k] || "") !== root.val(k)) root.edited(k, p.set[k] || "") }

    readonly property var names: ({ opacity: "background", clickThrough: "clicks", noFocus: "focus", width: "width", fontFamily: "font", fontSize: "text size",
                                    group: "group", priority: "priority", media: "pictures", stack: "stacking", timerStyle: "time left" })
    readonly property var changedHere: Object.keys(root.names).filter(k => root.val(k) !== "")

    // ---- styles
    Flow {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing
        Repeater {
            model: root.presets
            delegate: QQC2.Button {
                id: pb
                required property var modelData
                checkable: true
                checked: root.currentPreset === modelData.id
                onToggled: { root.applyPreset(modelData); checked = Qt.binding(() => root.currentPreset === pb.modelData.id) }
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: modelData.tip
                Accessible.name: "Style: " + modelData.label
                implicitWidth: Kirigami.Units.gridUnit * 5.5
                contentItem: ColumnLayout {
                    spacing: 4
                    // a thumbnail of the style, in a fixed-height slot so the names line up
                    Item {
                      Layout.fillWidth: true
                      implicitHeight: Kirigami.Units.gridUnit * 1.9
                      Rectangle {
                        anchors.centerIn: parent
                        readonly property real w: Number(pb.modelData.set.width || 380)
                        implicitWidth: Kirigami.Units.gridUnit * 3.6 * w / 380
                        implicitHeight: Kirigami.Units.gridUnit * (pb.modelData.set.fontSize === "15" ? 1.8 : pb.modelData.set.fontSize === "11" ? 1.2 : 1.5)
                        radius: 3
                        color: root.base.card || "#1d1f27"
                        opacity: Number(pb.modelData.set.opacity || 1)
                        border.width: (pb.modelData.set.timerStyle || "").indexOf("frame") >= 0 ? 2 : 1
                        border.color: (pb.modelData.set.timerStyle || "").indexOf("frame") >= 0 ? (root.accent || Kirigami.Theme.highlightColor) : (root.base.border || "#2c2f3a")
                        Rectangle { x: 4; y: 4; width: parent.width * 0.6; height: 3; color: root.base.text || "#f2f3f7" }
                        Rectangle { x: 4; y: 10; width: parent.width * 0.8; height: 2; color: root.base.subtext || "#b9bccb"; opacity: 0.8 }
                        Rectangle { visible: (pb.modelData.set.timerStyle || "").indexOf("line") >= 0; x: 4; anchors.bottom: parent.bottom; anchors.bottomMargin: 3; width: parent.width * 0.5; height: 2; color: root.accent || Kirigami.Theme.highlightColor }
                    }
                    }
                    QQC2.Label { Layout.fillWidth: true; horizontalAlignment: Text.AlignHCenter; elide: Text.ElideRight; text: pb.modelData.label; font: Kirigami.Theme.smallFont }
                }
            }
        }
    }

    // ---- where the look comes from
    RowLayout {
        Layout.fillWidth: true
        spacing: 4
        Repeater {
            model: root.levels
            delegate: QQC2.Label {
                required property string modelData
                required property int index
                text: (index > 0 ? "›  " : "") + modelData
                font.bold: index === root.levels.length - 1
                opacity: index === root.levels.length - 1 ? 1 : 0.65
            }
        }
        Item { Layout.fillWidth: true }
        QQC2.Label {
            opacity: 0.7
            font: Kirigami.Theme.smallFont
            text: root.changedHere.length ? "Changed here: " + root.changedHere.map(k => root.names[k]).join(", ") : "Nothing changed here"
        }
        QQC2.ToolButton {
            visible: root.changedHere.length > 0
            icon.name: "edit-undo"
            text: "Reset this level"
            onClicked: { const keys = root.changedHere.slice(); for (const k of keys) root.edited(k, "") }
        }
    }

    // ---- the preview beside the choices (stacked when narrow)
    GridLayout {
        Layout.fillWidth: true
        columns: root.width > Kirigami.Units.gridUnit * 40 ? 2 : 1
        columnSpacing: Kirigami.Units.largeSpacing * 2
        rowSpacing: Kirigami.Units.largeSpacing

        ColumnLayout {
            Layout.alignment: Qt.AlignTop
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            spacing: Kirigami.Units.smallSpacing
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: preview.implicitHeight + Kirigami.Units.largeSpacing * 2
                radius: 6
                color: Qt.rgba(0.17, 0.18, 0.22, 1)          // a dark desktop behind the sample, as on screen
                PopupPreview {
                    id: preview
                    anchors { left: parent.left; right: parent.right; top: parent.top; margins: Kirigami.Units.largeSpacing }
                    base: root.base
                    over: compare.pressed ? ({}) : root.values
                    accent: root.accent
                    interactive: true
                    onDark: true
                    timeoutMs: root.timeoutMs >= 0 ? root.timeoutMs : (Number(root.settingsValues["defaultTimeout"]) || 0)
                    onPartClicked: part => {
                        const go = { font: ["look", "fontSize"], width: ["look", "width"], timer: ["timing", "timerStyle"], card: ["look", "opacity"] }[part]
                        controls.tab = go[0]; controls.flash = go[1]; flashOff.restart()
                    }
                }
            }
            RowLayout {
                QQC2.Label { Layout.fillWidth: true; wrapMode: Text.Wrap; opacity: 0.7; font: Kirigami.Theme.smallFont; text: compare.pressed ? "Showing it without this level's changes" : "Click a part of the popup to change it" }
                QQC2.Button {
                    id: compare
                    visible: root.changedHere.length > 0
                    text: "Hold to compare"
                    icon.name: "view-visible"
                    Accessible.name: "Hold to see the popup without this level's changes"
                }
            }
        }
        LookControls {
            id: controls
            Layout.alignment: Qt.AlignTop
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            values: root.values
            base: root.base
            inheritFrom: root.inheritFrom
            showGroup: root.showGroup
            groups: root.groups
            onEdited: (k, t) => root.edited(k, t)
            Timer { id: flashOff; interval: 1400; onTriggered: controls.flash = "" }
        }
    }
}
