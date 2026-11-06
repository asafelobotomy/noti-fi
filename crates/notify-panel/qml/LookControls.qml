import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// The three tabs of the look editor: Look (size, width, font, opacity), Timing (the time-left indicator) and Behaviour (priority,
// stacking, media, clicks, focus, group). Every setting is a choice; a setting not changed at this level shows what it inherits,
// and a changed one has a ↺ to go back. `flash` briefly highlights one setting (after a click on that part of the preview).
ColumnLayout {
    id: root
    property var values: ({})
    property var base: ({})              // the theme's values, to say what is inherited
    property string inheritFrom: "the theme"
    property bool showGroup: true
    property var groups: []
    property string tab: "look"          // look | timing | behaviour
    property string flash: ""
    signal edited(string key, string text)
    spacing: Kirigami.Units.smallSpacing

    function val(k) { return root.values && root.values[k] !== undefined && root.values[k] !== null ? String(root.values[k]) : "" }
    function baseVal(k, fallback) { const b = root.base ? root.base[k] : undefined; return b !== undefined && b !== null && b !== "" ? String(b) : fallback }

    // one setting: its name, the control, what it inherits when unchanged, and ↺ when changed
    component Setting: ColumnLayout {
        id: setting
        property string label
        property string key
        property string inherited: ""
        default property alias content: holder.data
        Layout.fillWidth: true
        spacing: 2
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: head.implicitHeight + 4
            radius: 3
            color: root.flash === setting.key ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.25) : "transparent"
            Behavior on color { ColorAnimation { duration: 300 } }
            RowLayout {
                id: head
                anchors { left: parent.left; right: parent.right; verticalCenter: parent.verticalCenter }
                QQC2.Label { text: setting.label; font.bold: true }
                QQC2.Label {
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    opacity: 0.6
                    font: Kirigami.Theme.smallFont
                    text: root.val(setting.key) === "" ? (setting.inherited !== "" ? "from " + root.inheritFrom + ": " + setting.inherited : "from " + root.inheritFrom) : "changed here"
                }
                QQC2.ToolButton {
                    visible: root.val(setting.key) !== ""
                    icon.name: "edit-undo"
                    display: QQC2.AbstractButton.IconOnly
                    onClicked: root.edited(setting.key, "")
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: "Back to " + root.inheritFrom
                    Accessible.name: "Reset " + setting.label
                }
            }
        }
        RowLayout { id: holder; Layout.fillWidth: true }
    }

    QQC2.TabBar {
        Layout.fillWidth: true
        currentIndex: ["look", "timing", "behaviour"].indexOf(root.tab)
        onCurrentIndexChanged: root.tab = ["look", "timing", "behaviour"][currentIndex]
        QQC2.TabButton { text: "Look" }
        QQC2.TabButton { text: "Timing" }
        QQC2.TabButton { text: "Behaviour" }
    }

    // ---------------- Look ----------------
    ColumnLayout {
        visible: root.tab === "look"
        Layout.fillWidth: true
        spacing: Kirigami.Units.largeSpacing
        Setting {
            label: "Text size"; key: "fontSize"; inherited: root.baseVal("fontSize", "12") + " px"
            Segmented {
                Layout.fillWidth: true; accessibleName: "Text size"
                options: [{ value: "", label: "Inherit" }, { value: "11", label: "Small" }, { value: "13", label: "Normal" }, { value: "15", label: "Large" }, { value: "18", label: "Larger" }]
                value: ["", "11", "13", "15", "18"].indexOf(root.val("fontSize")) >= 0 ? root.val("fontSize") : "custom"
                onPicked: v => root.edited("fontSize", v)
            }
        }
        Setting {
            label: "Width"; key: "width"; inherited: root.baseVal("width", "380") + " px"
            Segmented {
                Layout.fillWidth: true; accessibleName: "Width"
                options: [{ value: "", label: "Inherit" }, { value: "320", label: "Narrow" }, { value: "380", label: "Normal" }, { value: "460", label: "Wide" }, { value: "560", label: "Wider" }]
                value: ["", "320", "380", "460", "560"].indexOf(root.val("width")) >= 0 ? root.val("width") : "custom"
                onPicked: v => root.edited("width", v)
            }
        }
        Setting {
            label: "Font"; key: "fontFamily"; inherited: root.baseVal("fontFamily", "system font")
            FontPicker { Layout.fillWidth: true; unsetLabel: "Inherit"; value: root.val("fontFamily"); onEdited: t => root.edited("fontFamily", t) }
        }
        Setting {
            label: "Background"; key: "opacity"; inherited: Math.round(Number(root.baseVal("opacity", "1")) * 100) + "% solid"
            QQC2.Slider {
                id: slider
                Layout.fillWidth: true
                from: 40; to: 100; stepSize: 5
                Binding on value { when: !slider.pressed; value: root.val("opacity") === "" ? Math.round(Number(root.baseVal("opacity", "1")) * 100) : Math.round(Number(root.val("opacity")) * 100) }
                onMoved: root.edited("opacity", String(Math.round(value) / 100))
                Accessible.name: "Background opacity"
            }
            QQC2.Label { text: Math.round(slider.value) + "%"; Layout.minimumWidth: Kirigami.Units.gridUnit * 2.5 }
        }
    }

    // ---------------- Timing ----------------
    ColumnLayout {
        visible: root.tab === "timing"
        Layout.fillWidth: true
        spacing: Kirigami.Units.largeSpacing
        Setting {
            label: "Show the time left"; key: "timerStyle"; inherited: root.baseVal("timerStyle", "none")
            readonly property string cur: root.val("timerStyle")
            Segmented {
                accessibleName: "Time left"
                options: [{ value: "", label: "Inherit" }, { value: "none", label: "Nothing" }]
                value: root.val("timerStyle") === "" || root.val("timerStyle") === "none" ? root.val("timerStyle") : "parts"
                onPicked: v => root.edited("timerStyle", v)
            }
            // any mix of the three indicators
            Repeater {
                model: [{ id: "line", label: "Footer line" }, { id: "circle", label: "Circle" }, { id: "frame", label: "Frame" }]
                delegate: QQC2.Button {
                    id: part
                    required property var modelData
                    readonly property var on: root.val("timerStyle").split(",").filter(x => x === "line" || x === "circle" || x === "frame")
                    text: modelData.label
                    checkable: true
                    checked: on.indexOf(modelData.id) >= 0
                    onToggled: {
                        const set = ["line", "circle", "frame"].filter(x => x === modelData.id ? checked : on.indexOf(x) >= 0)
                        root.edited("timerStyle", set.length ? set.join(",") : "none")
                        checked = Qt.binding(() => part.on.indexOf(part.modelData.id) >= 0)
                    }
                    Accessible.name: "Time left: " + modelData.label
                }
            }
        }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            opacity: 0.7
            font: Kirigami.Theme.smallFont
            text: "The preview's indicator runs at the real speed. How long a popup stays is set where the group, app or rule is chosen."
        }
    }

    // ---------------- Behaviour ----------------
    ColumnLayout {
        visible: root.tab === "behaviour"
        Layout.fillWidth: true
        spacing: Kirigami.Units.largeSpacing
        Setting {
            label: "Priority"; key: "priority"; inherited: "the app's urgency"
            Segmented {
                Layout.fillWidth: true; accessibleName: "Priority"
                options: [{ value: "", label: "Inherit" }, { value: "low", label: "Low" }, { value: "normal", label: "Normal" }, { value: "high", label: "High", tip: "Marked as important" },
                          { value: "urgent", label: "Urgent", tip: "Gets through Do Not Disturb and focus modes" }]
                value: root.val("priority")
                onPicked: v => root.edited("priority", v)
            }
        }
        Setting {
            label: "Similar notifications"; key: "stack"
            Segmented {
                Layout.fillWidth: true; accessibleName: "Similar notifications"
                options: [{ value: "", label: "Inherit" }, { value: "true", label: "Stack with a count" }, { value: "false", label: "Show each one" }]
                value: root.val("stack"); onPicked: v => root.edited("stack", v)
            }
        }
        Setting {
            label: "Pictures and players"; key: "media"; inherited: "pictures only"
            Segmented {
                Layout.fillWidth: true; accessibleName: "Pictures and players"
                options: [{ value: "", label: "Inherit" }, { value: "true", label: "Pictures, audio and video" }, { value: "false", label: "Neither" }]
                value: root.val("media"); onPicked: v => root.edited("media", v)
            }
        }
        Setting {
            label: "Clicks"; key: "clickThrough"
            Segmented {
                Layout.fillWidth: true; accessibleName: "Clicks"
                options: [{ value: "", label: "Inherit" }, { value: "false", label: "Popup takes clicks" }, { value: "true", label: "Pass through to the window below" }]
                value: root.val("clickThrough"); onPicked: v => root.edited("clickThrough", v)
            }
        }
        Setting {
            label: "Keyboard focus"; key: "noFocus"
            Segmented {
                Layout.fillWidth: true; accessibleName: "Keyboard focus"
                options: [{ value: "", label: "Inherit" }, { value: "false", label: "May take focus to reply" }, { value: "true", label: "Never (no reply box)" }]
                value: root.val("noFocus"); onPicked: v => root.edited("noFocus", v)
            }
        }
        Setting {
            visible: root.showGroup
            label: "Show together under a heading"; key: "group"; inherited: "no group"
            QQC2.ComboBox {
                id: groupBox
                Layout.fillWidth: true
                editable: true
                readonly property var choices: ["No group"].concat(root.groups.filter(g => g !== ""), root.val("group") !== "" && root.groups.indexOf(root.val("group")) < 0 ? [root.val("group")] : [])
                model: choices
                currentIndex: root.val("group") === "" ? 0 : Math.max(0, choices.indexOf(root.val("group")))
                onActivated: index => root.edited("group", index === 0 ? "" : choices[index])
                onAccepted: root.edited("group", editText === "No group" ? "" : editText.trim())
                Accessible.name: "Group"
            }
        }
    }
}
