import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import "Json.js" as Json

// The apps, websites and kinds of notification in one group, as chips, with "Add app" and "Add website".
// Membership is kept as rules (a rule that puts matching notifications in this group): a rule that tests one thing as a plain list
// shows one chip per entry (removing one edits the list); a rule that tests several things is one chip; any other rule shows as a
// link to the Rules page. The System group also lists the system's own sources, which are always in it and cannot be removed.
ColumnLayout {
    id: root
    property var model
    property string groupId: ""
    property string groupLabel: ""
    signal openRules()
    spacing: Kirigami.Units.smallSpacing

    MatchTags { id: parser; visible: false }          // only for its parse() / build() of plain lists
    RuleData { id: data }
    function kindOf(field) { const f = data.matchFields.find(x => x.field === field); return f ? f.kind : "contains" }
    readonly property var icons: ({ appName: "application-x-executable", desktopEntry: "application-x-executable", origin: "internet-web-browser",
                                    category: "tag", summary: "edit-find", body: "edit-find", urgency: "emblem-important", channel: "network-connect" })

    readonly property var rules: Json.parse(root.model ? root.model.rulesJson : "[]", [])
    readonly property var apps: Json.parse(root.model ? root.model.appsJson : "[]", [])
    function appLabel(field, word) {
        if (field !== "appName" && field !== "desktopEntry") return word
        const a = root.apps.find(x => (field === "desktopEntry" ? x.desktop_entry : x.name).toLowerCase() === word.toLowerCase())
        return a ? a.name : word
    }
    // [{ label, icon, index, field, words, word, kind: "word" | "rule" | "other" }]
    readonly property var members: {
        const out = []
        root.rules.forEach((r, i) => {
            if (r.zone !== root.groupId) return
            const set = Object.keys(r.match).filter(k => r.match[k] !== "")
            if (set.length === 1) {
                const words = parser.parse(r.match[set[0]], root.kindOf(set[0]))
                if (words && words.length) { for (const w of words) out.push({ label: root.appLabel(set[0], w), icon: root.icons[set[0]] || "tag", index: i, field: set[0], words: words, word: w, kind: "word" }); return }
            }
            if (set.length > 1 && set.every(k => parser.parse(r.match[k], root.kindOf(k)) !== null))
                out.push({ label: set.map(k => parser.parse(r.match[k], root.kindOf(k)).join(" or ")).join(" · "), icon: "view-filter", index: i, kind: "rule" })
            else out.push({ label: "Rule: " + r.title, icon: "view-filter", index: i, kind: "other" })
        })
        return out
    }
    function remove(m) {
        if (m.kind === "word" && m.words.length > 1)
            root.model.ruleSetMatch(m.index, m.field, parser.build(m.words.filter(w => w !== m.word), root.kindOf(m.field)))
        else root.model.ruleDelete(m.index)
    }
    function esc(t) { let o = ""; for (const c of String(t)) o += (".^$*+?()[]{}|\\".indexOf(c) >= 0 ? "\\" : "") + c; return o }
    function addWebsite(site) {
        site = site.trim().toLowerCase()
        if (site === "") return
        let i = root.model.ruleAdd()
        root.model.ruleSetName(i, "Website: " + site)
        root.model.ruleSetMatch(i, "origin", "\\b" + root.esc(site) + "\\b")
        root.model.ruleSetZone(i, root.groupId)
        while (i > 0) i = root.model.ruleMove(i, -1)          // ahead of broader rules
    }

    QQC2.Label { text: "Apps and websites in this group"; font.bold: true }
    // the system's own sources: always here
    QQC2.Label {
        visible: root.groupId === "system"
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        opacity: 0.75
        text: "🔒 Always here: the system's own notifications (the kernel, systemd, power, disks, network, polkit, KWin). They cannot be moved, muted or silenced, because they are how the system reaches you."
    }
    Flow {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing
        Repeater {
            model: root.members
            delegate: Rectangle {
                id: chip
                required property var modelData
                implicitWidth: row.implicitWidth + Kirigami.Units.smallSpacing * 2
                implicitHeight: row.implicitHeight + 4
                radius: height / 2
                color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, chip.modelData.kind === "other" ? 0.08 : 0.18)
                border.width: 1
                border.color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.5)
                RowLayout {
                    id: row
                    anchors.centerIn: parent
                    spacing: 4
                    Kirigami.Icon { source: chip.modelData.icon; Layout.preferredWidth: Kirigami.Units.iconSizes.small; Layout.preferredHeight: Layout.preferredWidth; Layout.leftMargin: 6 }
                    QQC2.Label { text: chip.modelData.label; textFormat: Text.PlainText; elide: Text.ElideRight; Layout.maximumWidth: Kirigami.Units.gridUnit * 16 }
                    QQC2.ToolButton {
                        icon.name: chip.modelData.kind === "other" ? "go-next" : "window-close"
                        display: QQC2.AbstractButton.IconOnly
                        implicitWidth: Kirigami.Units.iconSizes.smallMedium; implicitHeight: implicitWidth
                        onClicked: chip.modelData.kind === "other" ? root.openRules() : root.remove(chip.modelData)
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.text: chip.modelData.kind === "other" ? "This rule is more than a list: edit it on the Rules page" : "Take out of " + root.groupLabel
                        Accessible.name: (chip.modelData.kind === "other" ? "Open " : "Remove ") + chip.modelData.label
                    }
                }
            }
        }
        QQC2.Label { visible: root.members.length === 0 && root.groupId !== "system"; opacity: 0.6; text: "Nothing yet. Add apps or websites, or put notifications here from History." }
    }
    RowLayout {
        QQC2.Button { text: "Add app…"; icon.name: "list-add"; onClicked: appPop.open() }
        QQC2.Button { text: "Add website…"; icon.name: "internet-web-browser"; onClicked: sitePop.open() }
    }
    QQC2.Label {
        visible: root.groupId === "system"
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        opacity: 0.6
        font: Kirigami.Theme.smallFont
        text: "To take a stock system app out of System, add it to another group."
    }

    // ---- add an app: every app known, searchable, each moved here in one click
    QQC2.Popup {
        id: appPop
        width: Kirigami.Units.gridUnit * 20; height: Kirigami.Units.gridUnit * 20
        padding: Kirigami.Units.smallSpacing
        onOpened: { appSearch.text = ""; appSearch.forceActiveFocus() }
        contentItem: ColumnLayout {
            Kirigami.SearchField { id: appSearch; Layout.fillWidth: true; placeholderText: "Find an app" }
            ListView {
                Layout.fillWidth: true; Layout.fillHeight: true
                clip: true
                model: appPop.visible ? root.apps.filter(a => appSearch.text === "" || a.name.toLowerCase().indexOf(appSearch.text.toLowerCase()) >= 0) : []
                delegate: QQC2.ItemDelegate {
                    required property var modelData
                    width: ListView.view.width
                    text: modelData.name + (modelData.routing && modelData.routing.zone && modelData.routing.zone !== root.groupId ? "   (now in " + modelData.routing.zone + ")" : "")
                    icon.name: modelData.icon !== "" ? modelData.icon : "application-x-executable"
                    onClicked: { root.model.appSet(modelData.match_field, modelData.match_value, modelData.name, "zone", root.groupId); appPop.close() }
                }
                QQC2.ScrollBar.vertical: QQC2.ScrollBar {}
            }
        }
    }
    // ---- add a website: the ones seen in History first, or type one
    QQC2.Popup {
        id: sitePop
        width: Kirigami.Units.gridUnit * 20; height: Kirigami.Units.gridUnit * 16
        padding: Kirigami.Units.smallSpacing
        readonly property var seen: sitePop.visible ? (Json.parse(root.model.ruleSuggestions(), ({})).origin || []) : []
        onOpened: { siteField.text = ""; siteField.forceActiveFocus() }
        contentItem: ColumnLayout {
            RowLayout {
                QQC2.TextField { id: siteField; Layout.fillWidth: true; placeholderText: "example.com"; onAccepted: { root.addWebsite(text); sitePop.close() } }
                QQC2.Button { text: "Add"; enabled: siteField.text.trim() !== ""; onClicked: { root.addWebsite(siteField.text); sitePop.close() } }
            }
            QQC2.Label { visible: sitePop.seen.length > 0; text: "Seen in your notifications:"; opacity: 0.7 }
            ListView {
                Layout.fillWidth: true; Layout.fillHeight: true
                clip: true
                model: sitePop.seen.filter(s => siteField.text === "" || s.toLowerCase().indexOf(siteField.text.toLowerCase()) >= 0)
                delegate: QQC2.ItemDelegate {
                    required property string modelData
                    width: ListView.view.width
                    text: modelData
                    icon.name: "internet-web-browser"
                    onClicked: { root.addWebsite(modelData); sitePop.close() }
                }
            }
        }
    }
}
