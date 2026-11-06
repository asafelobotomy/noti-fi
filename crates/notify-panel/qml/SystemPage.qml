import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Health checks (what `notify doctor` prints) and the install actions. Every action shows its plan first and only runs after a second, explicit click.
Kirigami.ScrollablePage {
    id: page
    // in a wide window the content keeps a readable width, centred, instead of stretching labels and their controls apart
    leftPadding: Math.max(Kirigami.Units.largeSpacing, (width - Kirigami.Units.gridUnit * 56) / 2)
    rightPadding: leftPadding
    title: "System"
    property var model
    property var ui

    readonly property var sys: { try { return JSON.parse(page.model ? page.model.systemJson : "{}") } catch (e) { return ({}) } }
    readonly property var checks: sys.checks ?? []
    readonly property string busy: sys.busy ?? ""
    property string planned: ""            // the action whose plan is on screen; its Apply button is enabled
    readonly property var actions: [
        { id: "takeover", label: "Make notifd the notification owner", apply: "Take over now", warn: "This restarts the Plasma shell briefly. Notifications are handed back with Restore." },
        { id: "restore", label: "Hand notifications back to Plasma", apply: "Restore", warn: "Plasma shows notifications again; notifd keeps its files." },
        { id: "adapt", label: "Make Brave deliver notifications", apply: "Apply to Brave", warn: "Brave must be closed. The original settings are backed up first." },
        { id: "update", label: "Install changed daemon files", apply: "Update daemon", warn: "The new files are smoke-tested on a private bus first and rolled back if the daemon will not start." }
    ]

    readonly property var uiState: ({
        page: "system",
        checks: checks.length,
        problems: checks.filter(c => c.level === "warn" || c.level === "fail" || c.level === "error").length,
        busy: busy,
        planned: planned
    })

    Component.onCompleted: if (page.model) page.model.systemDoctor()

    function levelColor(l) {
        return l === "ok" ? Kirigami.Theme.positiveTextColor
             : l === "info" ? Kirigami.Theme.disabledTextColor
             : l === "warn" ? Kirigami.Theme.neutralTextColor : Kirigami.Theme.negativeTextColor
    }
    function levelMark(l) { return l === "ok" ? "OK" : l === "info" ? "info" : l === "warn" ? "check" : "fix" }

    Kirigami.PromptDialog {
        id: confirm
        property var action: ({})
        title: "Confirm: " + (action.label ?? "")
        subtitle: action.warn ?? ""
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: { page.model.systemRun(action.id, true); page.planned = "" }
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        RowLayout {
            Layout.fillWidth: true
            Kirigami.Heading { level: 3; text: "Health checks"; Layout.fillWidth: true }
            QQC2.Button {
                text: page.busy === "doctor" ? "Checking..." : "Check again"
                icon.name: "view-refresh"
                enabled: page.busy === ""
                onClicked: page.model.systemDoctor()
                Accessible.name: "Check again"
            }
        }
        QQC2.Label {
            visible: (page.sys.error ?? "") !== ""
            text: page.sys.error ?? ""
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        Repeater {
            model: page.checks
            delegate: RowLayout {
                required property var modelData
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing
                QQC2.Label {
                    text: page.levelMark(modelData.level)
                    color: page.levelColor(modelData.level)
                    font.bold: true
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 3
                }
                QQC2.Label {
                    text: modelData.group + ": " + modelData.message
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                    Accessible.name: modelData.level + ", " + modelData.group + ": " + modelData.message
                }
            }
        }

        Kirigami.Separator { Layout.fillWidth: true }
        Kirigami.Heading { level: 3; text: "Install actions" }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            opacity: 0.7
            text: "Each action shows its plan first. Nothing changes until you press the second button and confirm."
        }
        Repeater {
            model: page.actions
            delegate: RowLayout {
                id: row
                required property var modelData
                Layout.fillWidth: true
                QQC2.Label { text: row.modelData.label; Layout.fillWidth: true; wrapMode: Text.Wrap }
                QQC2.Button {
                    visible: row.modelData.id !== "update"
                    text: "Show plan"
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                    enabled: page.busy === ""
                    onClicked: { page.planned = row.modelData.id; page.model.systemRun(row.modelData.id, false) }
                    Accessible.name: "Show plan: " + row.modelData.label
                }
                QQC2.Button {
                    text: row.modelData.apply
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 8
                    highlighted: true
                    // the daemon update has no separate plan; every other action must have had its plan shown first
                    enabled: page.busy === "" && (row.modelData.id === "update" || page.planned === row.modelData.id)
                    onClicked: { confirm.action = row.modelData; confirm.open() }
                    Accessible.name: row.modelData.apply
                }
            }
        }
        QQC2.BusyIndicator { running: page.busy !== "" && page.busy !== "doctor"; visible: running }
        QQC2.Label {
            visible: (page.sys.output ?? "") !== ""
            text: (page.sys.applied ? (page.sys.ok ? "Done: " : "Failed: ") : "Plan: ") + (page.sys.action ?? "")
            font.bold: true
        }
        QQC2.TextArea {
            visible: (page.sys.output ?? "") !== ""
            Layout.fillWidth: true
            Layout.preferredHeight: Kirigami.Units.gridUnit * 12
            readOnly: true
            wrapMode: Text.Wrap
            font.family: "monospace"
            text: page.sys.output ?? ""
            Accessible.name: "Command output"
        }
    }
}
