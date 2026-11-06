import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Everything about one app beyond zone and mute: how long its popups stay, the Flatpak permission, its look, and its notification types.
ColumnLayout {
    id: root
    property var host
    property var app          // the app as the list rows hold it (name, matchField, timeout, kinds, ...)
    property bool lookOpen: root.host.openAll
    property bool kindsOpen: root.host.openAll
        Layout.fillWidth: true
    spacing: Kirigami.Units.largeSpacing

GridLayout {
        columns: 4
        columnSpacing: Kirigami.Units.largeSpacing
        rowSpacing: Kirigami.Units.smallSpacing
        QQC2.Label { text: "Hide after (ms):" }
        QQC2.TextField {
            id: timeoutField
            Layout.preferredWidth: Kirigami.Units.gridUnit * 6
            placeholderText: "default"
            inputMethodHints: Qt.ImhDigitsOnly
            validator: IntValidator { bottom: 0 }
            Binding on text { when: !timeoutField.activeFocus; value: app.timeout }
            onTextEdited: root.host.model.appSet(app.matchField, app.matchValue, app.name, "timeout", text)
            Accessible.name: "Timeout for " + app.name
        }
        QQC2.Label { visible: app.flatpak; text: "Flatpak permission:" }
        QQC2.ComboBox {
            visible: app.flatpak
            Layout.preferredWidth: Kirigami.Units.gridUnit * 9
            model: ["Not set", "Allowed", "Blocked"]
            currentIndex: app.perm
            onActivated: index => root.host.model.appFlatpak(app.appId, ["reset", "allow", "deny"][index])
            Accessible.name: "Flatpak notification permission for " + app.name
        }
    }

    QQC2.Button {
        Layout.alignment: Qt.AlignLeft
        flat: true
        icon.name: root.lookOpen ? "go-up" : "go-down"
        text: root.lookOpen ? "Hide look and behavior" : "Change how its popups look and behave…"
        onClicked: root.lookOpen = !root.lookOpen
        Accessible.name: "Look and behavior of " + app.name
    }
    StyleFields {
        visible: root.lookOpen
        Layout.fillWidth: true
        showGroup: true
        inheritFrom: "its zone and the theme"
        model: root.host.model
        accent: root.host.zoneAccent(app.zone)
        timeoutMs: app.timeout !== "" ? Number(app.timeout) : -1
        values: app.appStyle
        onEdited: (key, text) => root.host.model.appSet(app.matchField, app.matchValue, app.name, key, text)
    }

    QQC2.Button {
        Layout.alignment: Qt.AlignLeft
        flat: true
        icon.name: root.kindsOpen ? "go-up" : "go-down"
        text: root.kindsOpen ? "Hide notification types" : "Treat its notification types separately (" + app.kindCount + ")…"
        onClicked: root.kindsOpen = !root.kindsOpen
        Accessible.name: "Notification types of " + app.name
    }
    QQC2.Label {
        visible: root.kindsOpen
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        opacity: 0.7
        text: app.kindCount > 0
            ? "Each type can go to its own zone, be muted, be grouped, or look different from the app's other notifications."
            : "None yet. A kind (for example email.arrived or a web page's notification) is listed as soon as the app sends one."
    }
    Repeater {
        model: root.kindsOpen ? app.kindCount : 0
        delegate: Kirigami.AbstractCard {
            id: kindCard
            required property int index
            readonly property var kind: app.kinds[index] !== undefined ? app.kinds[index] : ({ routing: { muted: false, zone: "", style: {} }, caps: [], category: "", channel: "app", count: 0, matches: [], key: "" })
            property bool lookOpen: root.host.openAll
            Layout.fillWidth: true
            contentItem: ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
                RowLayout {
                    QQC2.Label {
                        Layout.fillWidth: true
                        font.bold: true
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        text: (kindCard.kind.category !== "" ? kindCard.kind.category : "General notifications") + "  ·  " + root.host.channelName(kindCard.kind.channel)
                    }
                    QQC2.Switch {
                        text: "Mute"
                        checked: kindCard.kind.routing.muted
                        onToggled: {
                            root.host.model.appSetKind(JSON.stringify(kindCard.kind.matches), app.name + ": " + (kindCard.kind.category || "general") + " " + kindCard.kind.channel, "muted", checked ? "true" : "false")
                            checked = Qt.binding(() => kindCard.kind.routing.muted)
                        }
                        Accessible.name: "Mute this kind"
                    }
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    opacity: 0.7
                    textFormat: Text.PlainText
                    text: "Sent " + kindCard.kind.count + "×. Can do: " + (kindCard.kind.caps.length ? kindCard.kind.caps.join(", ") : "plain text only")
                }
                RowLayout {
                    QQC2.Label { text: "Show in:" }
                    QQC2.ComboBox {
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 9
                        model: root.host.zoneChoices
                        currentIndex: kindCard.kind.routing.zone === "" ? 0 : Math.max(0, root.host.zoneChoices.indexOf(kindCard.kind.routing.zone))
                        onActivated: i => root.host.model.appSetKind(JSON.stringify(kindCard.kind.matches), app.name + ": " + (kindCard.kind.category || "general") + " " + kindCard.kind.channel, "zone", i === 0 ? "" : root.host.zoneChoices[i])
                        Accessible.name: "Zone for this kind"
                    }
                    Item { Layout.fillWidth: true }
                    QQC2.Button {
                        icon.name: kindCard.lookOpen ? "go-up" : "go-down"
                        text: kindCard.lookOpen ? "Hide look" : "Look, group…"
                        onClicked: kindCard.lookOpen = !kindCard.lookOpen
                    }
                }
                StyleFields {
                    visible: kindCard.lookOpen
                    Layout.fillWidth: true
                    showGroup: true
                    inheritFrom: "the app's settings"
                    model: root.host.model
                    accent: root.host.zoneAccent(kindCard.kind.routing.zone || app.zone)
                    timeoutMs: app.timeout !== "" ? Number(app.timeout) : -1
                    values: kindCard.kind.routing.style
                    onEdited: (key, text) => root.host.model.appSetKind(JSON.stringify(kindCard.kind.matches), app.name + ": " + (kindCard.kind.category || "general") + " " + kindCard.kind.channel, key, text)
                }
            }
        }
    }
}
