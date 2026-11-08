import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import "Json.js" as Json

// Colours, sound and the "hold popups while..." behaviour. Edits the same draft as the other editor pages.
Kirigami.Page {
    id: page
    // in a wide window the content keeps a readable width, centred, instead of stretching labels and their controls apart
    leftPadding: Math.max(Kirigami.Units.largeSpacing, (width - Kirigami.Units.gridUnit * 60) / 2)
    rightPadding: leftPadding
    title: "Appearance & behavior"
    property var model
    property var ui
    readonly property bool online: model !== undefined && model !== null && model.connected === true

    readonly property var settings: Json.parse(page.model ? page.model.settingsJson : "", ({}))
    readonly property var vals: page.settings.values !== undefined ? page.settings.values : ({})
    // a colour that is safe to bind while offline or half-typed
    function colour(path) { const v = page.vals[path]; return v !== undefined && /^#[0-9a-fA-F]{3,8}$/.test(v) ? v : "transparent" }
    function flag(path) { return page.vals[path] === true }

    readonly property var uiState: ({
        page: "appearance",
        card: page.vals["theme.card"] !== undefined ? page.vals["theme.card"] : "",
        sound: page.flag("sound.enabled"),
        fullscreenHold: page.flag("autoDnd.fullscreen"),
        screenShareHold: page.flag("autoDnd.screenShare"),
        dirty: page.model ? page.model.dirty : false,
        valid: page.model ? page.model.draftValid : false,
        banner: banners.errorText,
        saveMessage: banners.saveText
    })

    actions: [
        Kirigami.Action {
            text: "Save"; icon.name: "document-save"
            enabled: page.online && page.model.dirty && page.model.draftValid && !page.model.checking
            onTriggered: page.model.rulesSave(false)
        },
        Kirigami.Action {
            text: "Revert"; icon.name: "document-revert"
            enabled: page.online && (page.model.dirty || page.model.externalChanged)
            onTriggered: page.model.rulesReload()
        },
        Kirigami.Action {
            text: "Preview on screen"; icon.name: "preview"
            enabled: page.online && page.model.draftValid
            onTriggered: page.model.rulesPreview()
        }
    ]

    component Toggle: QQC2.CheckBox {
        id: t
        required property string path
        checked: page.flag(path)
        onToggled: {
            page.model.settingSet(path, checked ? "true" : "false")
            checked = Qt.binding(() => page.flag(t.path))     // always follow the draft, not the click
        }
        Accessible.name: text
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        DraftBanners { id: banners; Layout.fillWidth: true; model: page.model }

        // stays in view while editing: a sample popup drawn with the draft theme, with the time-left indicator running
        PopupPreview {
            Layout.margins: Kirigami.Units.largeSpacing
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: Math.min(Kirigami.Units.gridUnit * 24, parent.width - Kirigami.Units.gridUnit * 2)
            base: ({ card: page.vals["theme.card"], border: page.vals["theme.border"], text: page.vals["theme.text"], subtext: page.vals["theme.subtext"],
                     button: page.vals["theme.button"], opacity: page.vals["theme.opacity"], fontFamily: page.vals["theme.fontFamily"],
                     fontSize: page.vals["theme.fontSize"], width: page.vals["width"], timerStyle: page.vals["theme.timerStyle"] })
            timeoutMs: Number(page.vals["defaultTimeout"]) || 0
        }

        Kirigami.Separator { Layout.fillWidth: true }

        QQC2.ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            ColumnLayout {
                width: parent.width
                spacing: Kirigami.Units.largeSpacing

                QQC2.Label {
                    Layout.margins: Kirigami.Units.largeSpacing
                    Layout.bottomMargin: 0
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    opacity: 0.7
                    text: "The default look and behaviour of every popup. Groups, apps and rules can override any of it."
                }

                // a sample card drawn with the draft colours, so changes are visible before anything is saved

                Kirigami.FormLayout {
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }
                    Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Colours" }
                    Repeater {
                        model: [
                            { path: "theme.card", label: "Card background:" },
                            { path: "theme.border", label: "Card border:" },
                            { path: "theme.text", label: "Title text:" },
                            { path: "theme.subtext", label: "Message text:" },
                            { path: "theme.button", label: "Buttons:" },
                            { path: "theme.buttonHover", label: "Buttons when hovered:" }
                        ]
                        delegate: ColorField {
                            required property var modelData
                            Kirigami.FormData.label: modelData.label
                            value: page.vals[modelData.path] !== undefined ? page.vals[modelData.path] : ""
                            hint: modelData.path
                            onEdited: text => page.model.settingSet(modelData.path, text)
                        }
                    }

                    Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }

                    Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Text and transparency" }
                    RowLayout {
                        Kirigami.FormData.label: "Background opacity:"
                        QQC2.Slider {
                            id: opacitySlider
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 10
                            from: 10; to: 100; stepSize: 5
                            Binding on value { when: !opacitySlider.pressed; value: Math.round(Number(page.vals["theme.opacity"] !== undefined ? page.vals["theme.opacity"] : 1) * 100) }
                            onMoved: page.model.settingSet("theme.opacity", String(Math.round(value) / 100))
                            Accessible.name: "Popup background opacity"
                        }
                        QQC2.Label { text: Math.round(opacitySlider.value) + "%"; opacity: 0.8 }
                    }
                    RowLayout {
                        Kirigami.FormData.label: "Show time left:"
                        readonly property string cur: String(page.vals["theme.timerStyle"] !== undefined ? page.vals["theme.timerStyle"] : "none")
                        readonly property var on: cur.split(",").filter(x => x === "line" || x === "circle" || x === "frame")
                        id: timerRow
                        QQC2.Button {
                            text: "Nothing"; checkable: true; checked: timerRow.on.length === 0
                            onToggled: { page.model.settingSet("theme.timerStyle", "none"); checked = Qt.binding(() => timerRow.on.length === 0) }
                        }
                        Repeater {
                            model: [{ id: "line", label: "Footer line" }, { id: "circle", label: "Circle" }, { id: "frame", label: "Frame" }]
                            delegate: QQC2.Button {
                                id: tb
                                required property var modelData
                                text: modelData.label; checkable: true
                                checked: timerRow.on.indexOf(modelData.id) >= 0
                                onToggled: {
                                    const set = ["line", "circle", "frame"].filter(x => x === modelData.id ? checked : timerRow.on.indexOf(x) >= 0)
                                    page.model.settingSet("theme.timerStyle", set.length ? set.join(",") : "none")
                                    checked = Qt.binding(() => timerRow.on.indexOf(tb.modelData.id) >= 0)
                                }
                                Accessible.name: "Time left: " + modelData.label
                            }
                        }
                    }
                    FontPicker {
                        Kirigami.FormData.label: "Font:"
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 16
                        value: page.vals["theme.fontFamily"] !== undefined ? page.vals["theme.fontFamily"] : ""
                        onEdited: text => page.model.settingSet("theme.fontFamily", text)
                    }
                    Segmented {
                        Kirigami.FormData.label: "Text size:"
                        accessibleName: "Text size"
                        readonly property string cur: page.vals["theme.fontSize"] !== undefined ? String(page.vals["theme.fontSize"]) : "12"
                        options: [{ value: "11", label: "Small" }, { value: "12", label: "Normal" }, { value: "14", label: "Large" }, { value: "16", label: "Larger" }, { value: "18", label: "Largest" }]
                        value: cur
                        onPicked: v => page.model.settingSet("theme.fontSize", v)
                    }

                    Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }

                    Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Downloads and transfers" }
                    QQC2.ComboBox {
                        Kirigami.FormData.label: "Graph shown as:"
                        model: ["Line graph", "Bars", "Heatmap"]
                        currentIndex: Math.max(0, ["line", "bars", "heatmap"].indexOf(page.vals["transfer.chart"] !== undefined ? page.vals["transfer.chart"] : "line"))
                        onActivated: index => page.model.settingSet("transfer.chart", ["line", "bars", "heatmap"][index])
                        Accessible.name: "Default graph style"
                    }
                    QQC2.Label {
                        Kirigami.FormData.label: ""
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                        Layout.maximumWidth: Kirigami.Units.gridUnit * 28
                        opacity: 0.7
                        text: "A popup with a progress value (or a speed such as 3.2 MB/s in its text) can be expanded to show its rate over time. Each popup can switch between line, bars and heatmap."
                    }

                    Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }

                    Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Notification list" }
                    QQC2.ComboBox {
                        Kirigami.FormData.label: "Opens in:"
                        readonly property var ids: ["bottom-right", "bottom-center", "bottom-left", "top-right", "top-center", "top-left"]
                        model: ["Bottom right", "Bottom centre", "Bottom left", "Top right", "Top centre", "Top left"]
                        currentIndex: Math.max(0, ids.indexOf(page.vals["center.anchor"] !== undefined ? page.vals["center.anchor"] : "bottom-right"))
                        onActivated: index => page.model.settingSet("center.anchor", ids[index])
                        Accessible.name: "Where the notification list opens"
                    }
                    QQC2.Label {
                        Kirigami.FormData.label: ""
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                        Layout.maximumWidth: Kirigami.Units.gridUnit * 28
                        opacity: 0.7
                        text: "The list of recent notifications that clicking the tray icon opens. Put it in the corner next to your tray. To open it from the keyboard, set a shortcut on the System page, or run notify center."
                    }

                    Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }

                    Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Sound" }
                    Toggle { Kirigami.FormData.label: "Play a sound:"; text: "when a popup appears"; path: "sound.enabled" }
                    SoundPicker {
                        Kirigami.FormData.label: "Sound:"
                        model: page.model
                        value: page.vals["sound.defaultName"] !== undefined ? page.vals["sound.defaultName"] : ""
                        onEdited: text => page.model.settingSet("sound.defaultName", text)
                    }

                    Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }

                    Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Do Not Disturb" }
                    Toggle { Kirigami.FormData.label: "Critical:"; text: "still show critical notifications"; path: "dnd.allowCritical" }
                    Toggle { Kirigami.FormData.label: "Hold popups:"; text: "while a window is full screen"; path: "autoDnd.fullscreen" }
                    Toggle { text: "while the screen is being shared"; path: "autoDnd.screenShare" }

                    Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }

                    Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "History" }
                    Toggle { Kirigami.FormData.label: "Keep:"; text: "history across restarts"; path: "history.persist" }
                    Segmented {
                        Kirigami.FormData.label: "Remember at most:"
                        accessibleName: "History size"
                        readonly property string cur: page.vals["history.limit"] !== undefined ? String(page.vals["history.limit"]) : "200"
                        options: [{ value: "50", label: "50" }, { value: "100", label: "100" }, { value: "200", label: "200" }, { value: "500", label: "500" }, { value: "1000", label: "1000" }]
                        value: cur
                        onPicked: v => page.model.settingSet("history.limit", v)
                    }
                }
            }
        }
    }
}
