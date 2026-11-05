import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Look and behaviour of popups: transparency, click-through, focus, width, font, group. Used for a rule, a zone, an app and one kind of
// notification. `values` holds each setting as text ("" = not set, inherit from the next level); edits are emitted, never applied here.
Kirigami.FormLayout {
    id: root
    property var values: ({})
    property bool showGroup: true
    property string inheritFrom: "the zone and theme"
    signal edited(string key, string text)

    function val(k) { return root.values && root.values[k] !== undefined ? String(root.values[k]) : "" }
    function triIndex(k) { const v = val(k); return v === "true" ? 1 : v === "false" ? 2 : 0 }

    QQC2.ComboBox {
        Kirigami.FormData.label: "Priority:"
        model: ["Inherit (from the app's urgency)", "Low", "Normal", "High (marked)", "Urgent (passes Do Not Disturb and focus)"]
        currentIndex: ["", "low", "normal", "high", "urgent"].indexOf(root.val("priority")) < 0 ? 0 : ["", "low", "normal", "high", "urgent"].indexOf(root.val("priority"))
        onActivated: index => root.edited("priority", ["", "low", "normal", "high", "urgent"][index])
        Accessible.name: "Priority"
    }
    QQC2.ComboBox {
        Kirigami.FormData.label: "Show time left:"
        readonly property var choices: [["", "Inherit (" + root.inheritFrom + ")"], ["none", "No indicator"], ["line", "Footer line"], ["circle", "Circle"], ["frame", "Frame"],
                                        ["line,circle", "Footer line and circle"], ["line,frame", "Footer line and frame"], ["circle,frame", "Circle and frame"], ["line,circle,frame", "All three"]]
        model: choices.map(c => c[1])
        currentIndex: { const v = ["line", "circle", "frame"].filter(t => root.val("timerStyle").indexOf(t) >= 0).join(",") || root.val("timerStyle"); const i = choices.findIndex(c => c[0] === v); return i < 0 ? 0 : i }
        onActivated: index => root.edited("timerStyle", choices[index][0])
        Accessible.name: "Show time left"
    }
    QQC2.ComboBox {
        Kirigami.FormData.label: "Stack notifications:"
        model: ["Inherit (" + root.inheritFrom + ")", "Yes, stack with a count", "No, show each one"]
        currentIndex: root.triIndex("stack")
        onActivated: index => root.edited("stack", ["", "true", "false"][index])
        Accessible.name: "Stack notifications"
    }
    QQC2.ComboBox {
        Kirigami.FormData.label: "Pictures, audio, video:"
        model: ["Inherit (pictures only, no players)", "Pictures and audio/video players", "Nothing"]
        currentIndex: root.triIndex("media")
        onActivated: index => root.edited("media", ["", "true", "false"][index])
        Accessible.name: "Show media"
    }
    RowLayout {
        Kirigami.FormData.label: "Background opacity:"
        QQC2.Slider {
            id: slider
            Layout.preferredWidth: Kirigami.Units.gridUnit * 10
            from: 10; to: 100; stepSize: 5
            Binding on value { when: !slider.pressed; value: root.val("opacity") === "" ? 100 : Math.round(Number(root.val("opacity")) * 100) }
            onMoved: root.edited("opacity", String(Math.round(value) / 100))
            Accessible.name: "Background opacity"
        }
        QQC2.Label { text: root.val("opacity") === "" ? "inherit" : Math.round(Number(root.val("opacity")) * 100) + "%"; opacity: 0.8; Layout.minimumWidth: Kirigami.Units.gridUnit * 3 }
        QQC2.ToolButton { visible: root.val("opacity") !== ""; icon.name: "edit-clear"; text: "Inherit"; onClicked: root.edited("opacity", "") }
    }
    QQC2.ComboBox {
        Kirigami.FormData.label: "Let clicks pass through:"
        model: ["Inherit (" + root.inheritFrom + ")", "Yes, click through", "No, popup takes clicks"]
        currentIndex: root.triIndex("clickThrough")
        onActivated: index => root.edited("clickThrough", ["", "true", "false"][index])
        Accessible.name: "Click through"
    }
    QQC2.ComboBox {
        Kirigami.FormData.label: "Never take focus:"
        model: ["Inherit (" + root.inheritFrom + ")", "Yes, never take focus (no reply box)", "No, may take focus for replies"]
        currentIndex: root.triIndex("noFocus")
        onActivated: index => root.edited("noFocus", ["", "true", "false"][index])
        Accessible.name: "Never take focus"
    }
    RowLayout {
        Kirigami.FormData.label: "Width (px):"
        QQC2.TextField {
            id: widthField
            Layout.preferredWidth: Kirigami.Units.gridUnit * 6
            placeholderText: "inherit"
            inputMethodHints: Qt.ImhDigitsOnly
            validator: IntValidator { bottom: 0; top: 4000 }
            Binding on text { when: !widthField.activeFocus; value: root.val("width") }
            onTextEdited: root.edited("width", text)
            Accessible.name: "Width in pixels"
        }
        QQC2.Label { text: "120 to 4000; you can also drag a popup's inner edge"; opacity: 0.7; font: Kirigami.Theme.smallFont }
    }
    RowLayout {
        Kirigami.FormData.label: "Font:"
        QQC2.TextField {
            id: fontField
            Layout.preferredWidth: Kirigami.Units.gridUnit * 10
            placeholderText: "inherit (e.g. Noto Sans)"
            Binding on text { when: !fontField.activeFocus; value: root.val("fontFamily") }
            onTextEdited: root.edited("fontFamily", text)
            Accessible.name: "Font family"
        }
        QQC2.TextField {
            id: sizeField
            Layout.preferredWidth: Kirigami.Units.gridUnit * 4
            placeholderText: "size"
            inputMethodHints: Qt.ImhDigitsOnly
            validator: IntValidator { bottom: 0; top: 48 }
            Binding on text { when: !sizeField.activeFocus; value: root.val("fontSize") }
            onTextEdited: root.edited("fontSize", text)
            Accessible.name: "Font size in pixels"
        }
        QQC2.Label { text: "px"; opacity: 0.7 }
    }
    QQC2.TextField {
        id: groupField
        visible: root.showGroup
        Kirigami.FormData.label: "Keep together as group:"
        Layout.fillWidth: true
        placeholderText: "none; same name = shown together under a heading"
        Binding on text { when: !groupField.activeFocus; value: root.val("group") }
        onTextEdited: root.edited("group", text)
        Accessible.name: "Group name"
    }
}
