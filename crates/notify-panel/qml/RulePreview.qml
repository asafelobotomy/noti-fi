import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// "Would have matched": the selected rule tried against your recent notifications, so what a condition does is visible before saving.
// History keeps every field a rule can test (older entries lack the desktop entry, kind and channel, which is said).
Kirigami.AbstractCard {
    id: root
    property var rule                 // the selected rule as the Rules page holds it (mAppName, mOrigin, ...), or null
    property var recent: []           // newest first

    readonly property var tests: [
        { role: "mAppName", field: "app", label: "App" }, { role: "mOrigin", field: "origin", label: "Website" },
        { role: "mSummary", field: "summary", label: "Title" }, { role: "mBody", field: "body", label: "Text" }, { role: "mUrgency", field: "urgency", label: "Urgency" },
        { role: "mDesktopEntry", field: "desktopEntry", label: "Desktop entry" }, { role: "mCategory", field: "category", label: "Kind" }, { role: "mChannel", field: "channel", label: "Arrives as" }
    ]
    // notifications recorded before 0.15 did not keep the desktop entry, kind or channel, so they cannot match those conditions
    readonly property var blind: root.rule && root.recent.some(e => e.desktopEntry === undefined || (e.desktopEntry === "" && e.channel === ""))
        ? [["mDesktopEntry", "Desktop entry"], ["mCategory", "Kind"], ["mChannel", "Arrives as"]].filter(b => root.rule[b[0]] !== "").map(b => b[1]) : []
    readonly property var active: root.rule ? root.tests.filter(t => root.rule[t.role] !== "").map(t => ({ field: t.field, re: root.compile(root.rule[t.role]) })) : []
    function compile(p) { try { return new RegExp(p, "i") } catch (e) { return null } }
    readonly property var hits: root.rule ? root.recent.filter(e => root.active.every(a => a.re !== null && a.re.test(String(e[a.field] === undefined ? "" : e[a.field])))) : []
    readonly property bool bad: root.active.some(a => a.re === null)

    visible: root.rule !== null && root.rule !== undefined
    contentItem: ColumnLayout {
        spacing: Kirigami.Units.smallSpacing
        RowLayout {
            Kirigami.Icon { source: root.bad ? "dialog-warning" : "view-preview"; Layout.preferredWidth: Kirigami.Units.iconSizes.small; Layout.preferredHeight: Layout.preferredWidth }
            QQC2.Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                font.bold: true
                text: root.bad ? "One of the patterns is not a valid expression"
                    : root.recent.length === 0 ? "Nothing in History to try this against yet"
                    : root.active.length === 0 ? "No conditions yet: this rule would match every notification"
                    : "Would have matched " + root.hits.length + " of your last " + root.recent.length + " notifications"
            }
        }
        Repeater {
            model: root.bad || root.active.length === 0 ? [] : root.hits.slice(0, 4)
            delegate: RowLayout {
                required property var modelData
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing
                QQC2.Label { text: modelData.app; font.bold: true; elide: Text.ElideRight; Layout.maximumWidth: Kirigami.Units.gridUnit * 8; textFormat: Text.PlainText }
                QQC2.Label { Layout.fillWidth: true; text: modelData.summary; elide: Text.ElideRight; opacity: 0.8; textFormat: Text.PlainText }
                QQC2.Label { text: modelData.shown ? "shown in " + modelData.zone : "kept in history"; opacity: 0.6; font: Kirigami.Theme.smallFont }
            }
        }
        QQC2.Label {
            visible: root.hits.length > 4 && root.active.length > 0 && !root.bad
            opacity: 0.6
            font: Kirigami.Theme.smallFont
            text: "and " + (root.hits.length - 4) + " more"
        }
        QQC2.Label {
            visible: root.blind.length > 0
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            opacity: 0.7
            font: Kirigami.Theme.smallFont
            text: "Older notifications in History did not record the " + root.blind.join(", ").toLowerCase() + ", so they are not counted for " + (root.blind.length === 1 ? "that condition." : "those conditions.")
        }
    }
}
