import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Everything about one app beyond zone and mute: how long its popups stay, the Flatpak permission, its look, and its notification types.
ColumnLayout {
    id: root
    property var host
    property var app          // the app as the list rows hold it (name, matchField, timeout, kinds, ...)
    property bool kindsOpen: root.host.openAll
    spacing: Kirigami.Units.largeSpacing

    // where its popups appear
    RowLayout {
        spacing: Kirigami.Units.largeSpacing
        MiniScreen {
            readonly property var zn: root.host.zoneInfo(app.zone)
            dim: zn === null
            anchorName: zn ? zn.anchor : "top-right"
            fx: zn ? Number(zn.x) : 0.5; fy: zn ? Number(zn.y) : 0.1
            accent: zn && zn.accent ? zn.accent : ""
        }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            opacity: 0.8
            text: app.zone !== "" ? "Popups from " + app.name + " appear in the “" + root.host.zoneLabel(app.zone) + "” zone."
                                  : "No zone set for " + app.name + ": your rules decide, otherwise the default zone."
        }
    }

    GridLayout {
        columns: 4
        Layout.fillWidth: true
        columnSpacing: Kirigami.Units.largeSpacing
        rowSpacing: Kirigami.Units.smallSpacing
        QQC2.Label { text: "Stays on screen:"; Layout.alignment: Qt.AlignTop }
        DurationChips {
            Layout.columnSpan: 3
            Layout.fillWidth: true
            value: app.timeout
            onEdited: text => root.host.model.appSet(app.matchField, app.matchValue, app.name, "timeout", text)
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

    Kirigami.Heading { level: 4; text: "How its popups look and behave" }
    LookEditor {
        Layout.fillWidth: true
        showGroup: true
        inheritFrom: "its zone and the theme"
        levels: ["Theme", app.zone !== "" ? "Zone: " + root.host.zoneLabel(app.zone) : "Its zone", app.name]
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
                        model: root.host.zoneChoiceLabels
                        displayText: kindCard.kind.routing.zone !== "" ? root.host.zoneLabel(kindCard.kind.routing.zone) : root.host.zoneChoiceLabels[0]
                        currentIndex: kindCard.kind.routing.zone === "" ? 0 : Math.max(0, root.host.zoneChoices.indexOf(kindCard.kind.routing.zone))
                        onActivated: i => root.host.model.appSetKind(JSON.stringify(kindCard.kind.matches), app.name + ": " + (kindCard.kind.category || "general") + " " + kindCard.kind.channel, "zone", i === 0 ? "" : root.host.zoneChoices[i])
                        Accessible.name: "Zone for this kind"
                    }
                    Item { Layout.fillWidth: true }
                }
                LookEditor {
                    Layout.fillWidth: true
                    showGroup: true
                    inheritFrom: "the app's settings"
                    levels: ["Theme", "Zone", app.name, kindCard.kind.category !== "" ? kindCard.kind.category : "General"]
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
