import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Focus modes: while one is on, only notifications at or above its minimum priority, or matching its allow list, are shown.
// (Turning a mode on is on the Overview page; this page edits the modes themselves.)
Kirigami.Page {
    id: page
    title: "Focus modes"
    property var model
    property var ui
    readonly property bool online: model !== undefined && model !== null && model.connected === true

    readonly property var modes: { try { return JSON.parse(page.model ? page.model.focusDraftJson : "[]") } catch (e) { return [] } }
    readonly property var levels: ["low", "normal", "high", "urgent"]
    readonly property var levelNames: ["Low and above (everything)", "Normal and above", "High and above", "Urgent only"]

    readonly property var uiState: ({
        page: "focus",
        modes: page.modes.length,
        labels: page.modes.map(m => m.label || m.name).join(","),
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
            text: "Add focus mode"; icon.name: "list-add"
            onTriggered: page.model.focusAdd()
        }
    ]

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        DraftBanners { id: banners; Layout.fillWidth: true; model: page.model }

        QQC2.ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            ColumnLayout {
                width: parent.width
                spacing: Kirigami.Units.largeSpacing

                QQC2.Label {
                    Layout.margins: Kirigami.Units.largeSpacing
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    opacity: 0.7
                    text: "A notification's priority comes from the app's own urgency (critical = urgent) unless a rule sets one. While a focus mode is on, a notification is shown only if its priority is at least the mode's minimum, or it matches an allowed entry. Everything else is kept in History."
                }

                Repeater {
                    model: page.modes
                    delegate: Kirigami.AbstractCard {
                        id: card
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        contentItem: Kirigami.FormLayout {
                            QQC2.TextField {
                                id: labelField
                                Kirigami.FormData.label: "Name:"
                                Layout.fillWidth: true
                                placeholderText: card.modelData.name
                                Binding on text { when: !labelField.activeFocus; value: card.modelData.label }
                                onTextEdited: page.model.focusSet(card.modelData.name, "label", text)
                                Accessible.name: "Focus mode name"
                            }
                            QQC2.ComboBox {
                                Kirigami.FormData.label: "Let through:"
                                model: page.levelNames
                                currentIndex: Math.max(0, page.levels.indexOf(card.modelData.minPriority))
                                onActivated: index => page.model.focusSet(card.modelData.name, "minPriority", page.levels[index])
                                Accessible.name: "Minimum priority"
                            }
                            QQC2.TextArea {
                                id: allowField
                                Kirigami.FormData.label: "Always allow:"
                                Layout.fillWidth: true
                                Layout.preferredHeight: Kirigami.Units.gridUnit * 5
                                placeholderText: "one per line, e.g.\n^Signal$\norigin:calendar and category:^email"
                                font.family: "monospace"
                                Binding on text { when: !allowField.activeFocus; value: card.modelData.allow }
                                onTextChanged: if (activeFocus) page.model.focusSet(card.modelData.name, "allow", text)
                                Accessible.name: "Always allowed notifications"
                            }
                            QQC2.Label {
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                opacity: 0.7
                                font.pixelSize: Kirigami.Theme.smallFont.pixelSize
                                text: "Each line is a pattern for the app name, or field:pattern pairs joined with “ and ” (fields: appName, desktopEntry, origin, summary, body, urgency, category, channel)."
                            }
                            QQC2.Button {
                                icon.name: "edit-delete"; text: "Delete mode"
                                onClicked: page.model.focusDelete(card.modelData.name)
                                Accessible.name: "Delete focus mode"
                            }
                        }
                    }
                }
                Kirigami.PlaceholderMessage {
                    Layout.fillWidth: true
                    visible: page.modes.length === 0
                    text: "No focus modes"
                    explanation: "Add one to be able to switch it on from the Overview page or with `notify focus`."
                    icon.name: "face-glasses"
                }
            }
        }
    }
}
