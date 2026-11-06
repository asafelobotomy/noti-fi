import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

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

    readonly property var settings: { try { return JSON.parse(page.model ? page.model.settingsJson : "{}") } catch (e) { return {} } }
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
                    text: "The default look and behaviour of every popup. Zones, apps and rules can override any of it."
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
                    QQC2.ComboBox {
                        Kirigami.FormData.label: "Show time left:"
                        readonly property var choices: [["none", "No indicator"], ["line", "Footer line (shortens)"], ["circle", "Circle (empties)"], ["frame", "Frame (shrinks)"],
                                                        ["line,circle", "Footer line and circle"], ["line,frame", "Footer line and frame"], ["circle,frame", "Circle and frame"], ["line,circle,frame", "All three"]]
                        model: choices.map(c => c[1])
                        currentIndex: { const v = String(page.vals["theme.timerStyle"] !== undefined ? page.vals["theme.timerStyle"] : "none"); const k = ["line", "circle", "frame"].filter(t => v.indexOf(t) >= 0).join(",") || "none"; return Math.max(0, choices.findIndex(c => c[0] === k)) }
                        onActivated: index => page.model.settingSet("theme.timerStyle", choices[index][0])
                        Accessible.name: "Show the time left on popups"
                    }
                    QQC2.TextField {
                        id: fontFamily
                        Kirigami.FormData.label: "Font:"
                        placeholderText: "empty = the system font"
                        Binding on text { when: !fontFamily.activeFocus; value: page.vals["theme.fontFamily"] !== undefined ? page.vals["theme.fontFamily"] : "" }
                        onTextEdited: page.model.settingSet("theme.fontFamily", text)
                        Accessible.name: "Popup font"
                    }
                    RowLayout {
                        Kirigami.FormData.label: "Text size:"
                        QQC2.TextField {
                            id: fontSize
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 4
                            inputMethodHints: Qt.ImhDigitsOnly
                            validator: IntValidator { bottom: 0; top: 48 }
                            Binding on text { when: !fontSize.activeFocus; value: page.vals["theme.fontSize"] !== undefined ? String(page.vals["theme.fontSize"]) : "" }
                            onTextEdited: page.model.settingSet("theme.fontSize", text)
                            Accessible.name: "Popup text size"
                        }
                        QQC2.Label { text: "px (titles are 2 larger)"; opacity: 0.7 }
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
                        opacity: 0.7
                        text: "A popup with a progress value (or a speed such as 3.2 MB/s in its text) can be expanded to show its rate over time. Each popup can switch between line, bars and heatmap."
                    }

                    Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }

                    Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Sound" }
                    Toggle { Kirigami.FormData.label: "Play a sound:"; text: "when a popup appears"; path: "sound.enabled" }
                    QQC2.TextField {
                        id: soundName
                        Kirigami.FormData.label: "Sound name:"
                        placeholderText: "empty = the system default"
                        Binding on text { when: !soundName.activeFocus; value: page.vals["sound.defaultName"] !== undefined ? page.vals["sound.defaultName"] : "" }
                        onTextEdited: page.model.settingSet("sound.defaultName", text)
                        Accessible.name: "Sound name"
                    }

                    Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }

                    Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Do Not Disturb" }
                    Toggle { Kirigami.FormData.label: "Critical:"; text: "still show critical notifications"; path: "dnd.allowCritical" }
                    Toggle { Kirigami.FormData.label: "Hold popups:"; text: "while a window is full screen"; path: "autoDnd.fullscreen" }
                    Toggle { text: "while the screen is being shared"; path: "autoDnd.screenShare" }

                    Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }

                    Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "History" }
                    Toggle { Kirigami.FormData.label: "Keep:"; text: "history across restarts"; path: "history.persist" }
                    RowLayout {
                        Kirigami.FormData.label: "Remember at most:"
                        QQC2.TextField {
                            id: histLimit
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                            inputMethodHints: Qt.ImhDigitsOnly
                            validator: IntValidator { bottom: 0 }
                            Binding on text { when: !histLimit.activeFocus; value: page.vals["history.limit"] !== undefined ? String(page.vals["history.limit"]) : "" }
                            onTextEdited: page.model.settingSet("history.limit", text)
                            Accessible.name: "History limit"
                        }
                        QQC2.Label { text: "notifications"; opacity: 0.7 }
                    }
                }
            }
        }
    }
}
