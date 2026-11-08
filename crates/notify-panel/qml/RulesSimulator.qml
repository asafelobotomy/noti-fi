import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// The rule simulator: describe a notification, and see what the daemon would do with it (the daemon decides; this only shows its answer).
Kirigami.FormLayout {
    id: sim
    property var model          // the panel model: simSet(field, value)
    property var result: ({})   // the latest answer, already in plain words

        Layout.fillWidth: true
        Layout.margins: Kirigami.Units.largeSpacing

        Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }

        Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Simulator: what would happen to a notification like this?" }
        Repeater {
            model: [
                { field: "appName", label: "App name:", hint: "Brave Origin" },
                { field: "origin", label: "Site:", hint: "example-shop.com" },
                { field: "category", label: "Kind (category):", hint: "email.arrived" },
                { field: "desktopEntry", label: "Desktop entry:", hint: "" },
                { field: "summary", label: "Title:", hint: "New message" },
                { field: "body", label: "Text:", hint: "" }
            ]
            delegate: QQC2.TextField {
                id: simField
                required property var modelData
                Kirigami.FormData.label: modelData.label
                Layout.fillWidth: true
                placeholderText: modelData.hint
                onTextEdited: sim.model.simSet(simField.modelData.field, text)
                Accessible.name: "Simulate " + modelData.label
            }
        }
        QQC2.ComboBox {
            Kirigami.FormData.label: "Urgency:"
            model: ["normal", "low", "critical"]
            onActivated: index => sim.model.simSet("urgency", model[index])
            Accessible.name: "Simulated urgency"
        }
        QQC2.CheckBox {
            Kirigami.FormData.label: "Timeout:"
            text: "the app asked for no timeout"
            onToggled: sim.model.simSet("timeout", checked ? "never" : "default")
        }
        QQC2.CheckBox {
            Kirigami.FormData.label: "Do Not Disturb:"
            text: "ignore it, mutes and full screen right now"
            checked: true
            onToggled: sim.model.simSet("ignoreState", checked ? "true" : "false")
        }

        Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }

        Kirigami.Heading { Kirigami.FormData.isSection: true; level: 3; text: "Result" }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing
            QQC2.Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                visible: sim.result.error !== undefined
                color: Kirigami.Theme.negativeTextColor
                text: sim.result.error !== undefined ? "Cannot simulate: " + sim.result.error : ""
            }
            QQC2.Label { visible: sim.result.zone !== undefined; text: "Group: " + sim.result.zone + "   (" + sim.result.zoneBy + ")"; font.bold: true }
            QQC2.Label { visible: sim.result.timeout !== undefined; text: "Timeout: " + sim.result.timeout + "   (" + sim.result.timeoutBy + ")" }
            QQC2.Label { visible: sim.result.outcome !== undefined; text: "Outcome: " + sim.result.outcome }
            QQC2.Label {
                visible: sim.result.look !== undefined && sim.result.look.length > 0
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: sim.result.look ? "Look: " + sim.result.look.join(", ") : ""
            }
            QQC2.Label {
                visible: sim.result.matched !== undefined
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                opacity: 0.7
                text: sim.result.matched && sim.result.matched.length ? "Matching rules: " + sim.result.matched.join(", ") : "No rule matches; the defaults apply."
            }
        }
}
