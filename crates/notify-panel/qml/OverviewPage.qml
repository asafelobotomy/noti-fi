import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    id: page
    title: "Overview"
    property var model
    readonly property bool online: model !== undefined && model !== null && model.connected === true
    property var ui

    // what the user actually sees (read by the headless tests)
    readonly property var uiState: ({
        page: "overview",
        banner: banner.visible ? banner.text : "",
        subtitle: holdLabel.text,
        switchOn: dndSwitch.checked,
        focus: page.model ? page.model.focusMode : "",
        focusModes: page.focusModes.length,
        zones: zoneRepeater.count
    })

    readonly property var focusModes: {
        try { return JSON.parse(page.model ? page.model.focusModesJson : "[]") } catch (e) { return [] }
    }
    readonly property var zones: {
        try { return JSON.parse(page.model ? page.model.zonesJson : "[]") } catch (e) { return [] }
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Kirigami.InlineMessage {
            id: banner
            Layout.fillWidth: true
            visible: page.model !== undefined && page.model !== null && page.model.state !== "connected" && page.model.state !== "connecting"
            type: page.model && page.model.state === "lost" ? Kirigami.MessageType.Warning : Kirigami.MessageType.Error
            text: page.model ? page.model.stateMessage : ""
            showCloseButton: false
        }

        Kirigami.AbstractCard {
            Layout.fillWidth: true
            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing
                Kirigami.Icon {
                    source: page.model && page.model.dnd ? "notifications-disabled" : "notifications"
                    Layout.preferredWidth: Kirigami.Units.iconSizes.large
                    Layout.preferredHeight: Kirigami.Units.iconSizes.large
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0
                    Kirigami.Heading { level: 2; text: "Do Not Disturb" }
                    QQC2.Label {
                        id: holdLabel
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        opacity: 0.7
                        text: page.model ? page.model.holdText : "Not connected"
                    }
                }
                QQC2.Switch {
                    id: dndSwitch
                    enabled: page.online
                    checked: page.model ? page.model.dnd : false
                    onToggled: {
                        page.model.setDoNotDisturb(checked)
                        checked = Qt.binding(() => page.model ? page.model.dnd : false)   // follow the daemon, not the click
                    }
                    Accessible.name: "Do Not Disturb"
                }
            }
        }

        // Focus mode (like Apple's): while on, only notifications that are important enough (or allowed by name) are shown
        Kirigami.AbstractCard {
            Layout.fillWidth: true
            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing
                Kirigami.Icon {
                    source: page.model && page.model.focusMode !== "" ? "face-glasses" : "view-visible"
                    Layout.preferredWidth: Kirigami.Units.iconSizes.large
                    Layout.preferredHeight: Kirigami.Units.iconSizes.large
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0
                    Kirigami.Heading { level: 2; text: "Focus" }
                    QQC2.Label {
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        opacity: 0.7
                        text: page.model && page.model.focusMode !== ""
                            ? "On: only notifications that are important enough, or allowed by name, are shown. The rest wait in History."
                            : "Off. Choose a mode to let only important notifications through."
                    }
                }
                QQC2.ComboBox {
                    id: focusBox
                    enabled: page.online
                    model: ["Off"].concat(page.focusModes.map(m => m.label))
                    currentIndex: {
                        const cur = page.model ? page.model.focusMode : ""
                        for (let i = 0; i < page.focusModes.length; i++) if (page.focusModes[i].name === cur) return i + 1
                        return 0
                    }
                    onActivated: index => {
                        page.model.setFocus(index === 0 ? "off" : page.focusModes[index - 1].name)
                    }
                    Accessible.name: "Focus mode"
                }
            }
        }

        Kirigami.Heading { level: 3; text: "Zones" }

        Repeater {
            id: zoneRepeater
            model: page.zones
            delegate: Kirigami.AbstractCard {
                required property var modelData
                Layout.fillWidth: true
                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing
                    Rectangle {
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 0.8
                        Layout.preferredHeight: Kirigami.Units.gridUnit * 0.8
                        radius: width / 2
                        color: modelData.accent
                    }
                    QQC2.Label { Layout.fillWidth: true; text: modelData.label; font.bold: true }
                    QQC2.Label {
                        opacity: 0.7
                        text: modelData.count === 1 ? "1 popup" : modelData.count + " popups"
                        visible: modelData.count > 0
                    }
                    QQC2.Label { text: "Muted"; opacity: 0.7 }
                    QQC2.Switch {
                        enabled: page.online
                        checked: modelData.muted
                        onToggled: page.model.setZoneMuted(modelData.name, checked)
                        Accessible.name: "Mute " + modelData.label
                    }
                }
            }
        }

        QQC2.Label {
            Layout.topMargin: Kirigami.Units.largeSpacing
            opacity: 0.7
            visible: page.online
            text: (page.model ? page.model.active : 0) + " on screen" + (page.model && page.model.queued > 0 ? ", " + page.model.queued + " waiting" : "")
                  + "   ·   notifd " + (page.model ? page.model.daemonVersion : "")
        }
    }
}
