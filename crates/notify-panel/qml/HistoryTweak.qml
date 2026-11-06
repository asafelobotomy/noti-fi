import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Under an expanded History entry: change how notifications like this one are shown, saved as a rule of their own.
// "Like this" is chosen with switches (from this app, from this website, of this type); the first change creates the rule at the
// top of the list (so it wins over broader rules), later changes edit that same rule. Nothing reaches notifd until Save.
ColumnLayout {
    id: root
    property var entry: ({})
    property var model
    property var recent: []
    signal openRules()
    spacing: Kirigami.Units.largeSpacing

    // ---- what "like this" means
    property bool useApp: true
    property bool useSite: (root.entry.origin || "") !== ""
    property bool useType: (root.entry.category || "") !== ""
    function esc(t) { let o = ""; for (const c of String(t)) o += (".^$*+?()[]{}|\\".indexOf(c) >= 0 ? "\\" : "") + c; return o }
    readonly property string appField: (root.entry.desktopEntry || "") !== "" ? "desktopEntry" : "appName"
    readonly property var match: {
        const m = { appName: "", desktopEntry: "", origin: "", summary: "", body: "", urgency: "", category: "", channel: "" }
        if (root.useApp) m[root.appField] = "^" + root.esc(root.appField === "desktopEntry" ? root.entry.desktopEntry : root.entry.app) + "$"
        if (root.useSite && root.entry.origin) m.origin = "\\b" + root.esc(root.entry.origin) + "\\b"
        if (root.useType && root.entry.category) m.category = "^" + root.esc(root.entry.category)
        return m
    }
    readonly property bool anything: Object.keys(root.match).some(k => root.match[k] !== "")
    readonly property string ruleName: "From History: " + [root.useApp ? root.entry.app : "", root.useSite ? root.entry.origin : "", root.useType ? root.entry.category : ""].filter(x => x).join(" · ")

    // ---- the rule for exactly this "like this", if there is one already
    readonly property var rules: { try { return JSON.parse(root.model ? root.model.rulesJson : "[]") } catch (e) { return [] } }
    readonly property int ruleIndex: root.rules.findIndex(r => Object.keys(root.match).every(k => (r.match[k] || "") === root.match[k]))
    readonly property var rule: root.ruleIndex >= 0 ? root.rules[root.ruleIndex] : null
    function ensureRule() {
        if (root.ruleIndex >= 0) return root.ruleIndex
        let i = root.model.ruleAdd()
        root.model.ruleSetName(i, root.ruleName)
        for (const k in root.match) if (root.match[k] !== "") root.model.ruleSetMatch(i, k, root.match[k])
        while (i > 0) i = root.model.ruleMove(i, -1)          // to the top: the specific rule must win over broader ones
        return i
    }

    // ---- zones
    readonly property var zoneObjs: { try { return JSON.parse(root.model ? root.model.settingsJson : "{}").zones || [] } catch (e) { return [] } }
    function zoneInfo(id) { return root.zoneObjs.find(z => z.id === id) || null }
    readonly property var zoneIds: [""].concat(root.zoneObjs.map(z => z.id))
    readonly property string zoneNow: root.rule ? (root.rule.zone || "") : ""

    Kirigami.Heading { level: 4; text: "Change how notifications like this are shown" }

    // like this = …
    RowLayout {
        Layout.fillWidth: true
        QQC2.Label { text: "Like this means:"; opacity: 0.8 }
        Flow {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing
            QQC2.Button { text: "From " + (root.entry.app || "this app"); checkable: true; checked: root.useApp; onToggled: root.useApp = checked }
            QQC2.Button { visible: (root.entry.origin || "") !== ""; text: "From " + root.entry.origin; icon.name: "internet-web-browser"; checkable: true; checked: root.useSite; onToggled: root.useSite = checked }
            QQC2.Button { visible: (root.entry.category || "") !== ""; text: "Of type " + root.entry.category; icon.name: "tag"; checkable: true; checked: root.useType; onToggled: root.useType = checked }
        }
    }
    RulePreview {
        Layout.fillWidth: true
        recent: root.recent
        rule: ({ mAppName: root.match.appName, mDesktopEntry: root.match.desktopEntry, mOrigin: root.match.origin, mSummary: "", mBody: "",
                 mUrgency: "", mCategory: root.match.category, mChannel: "" })
    }

    ColumnLayout {
        enabled: root.anything
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing
        QQC2.Label { text: "Show in zone"; font.bold: true }
        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            MiniScreen {
                readonly property var zn: root.zoneInfo(root.zoneNow || root.entry.zone)   // where they go now, until a zone is chosen
                dim: zn === null
                anchorName: zn ? zn.anchor : "top-right"
                fx: zn ? Number(zn.x) : 0.5; fy: zn ? Number(zn.y) : 0.1
                accent: zn && zn.accent ? zn.accent : ""
            }
            QQC2.ComboBox {
                Layout.fillWidth: true
                model: root.zoneIds.map(id => id === "" ? "(as now: " + (root.entry.zone || "default") + ")" : ((root.zoneInfo(id) || {}).label || id))
                currentIndex: Math.max(0, root.zoneIds.indexOf(root.zoneNow))
                displayText: root.zoneNow === "" ? "(as now: " + (root.entry.zone || "default") + ")" : ((root.zoneInfo(root.zoneNow) || {}).label || root.zoneNow)
                onActivated: index => root.model.ruleSetZone(root.ensureRule(), root.zoneIds[index])
                Accessible.name: "Show in zone"
            }
        }
        QQC2.Label { Layout.topMargin: Kirigami.Units.smallSpacing; text: "Stays on screen"; font.bold: true }
        DurationChips {
            Layout.fillWidth: true
            unsetLabel: "As now"
            unsetTip: "Keep whatever decides it today"
            value: root.rule ? (root.rule.timeout || "") : ""
            onEdited: text => root.model.ruleSetTimeout(root.ensureRule(), text)
        }
        QQC2.CheckBox {
            text: "Never show these, keep them in History only"
            checked: root.rule ? root.rule.silent === true : false
            onToggled: { root.model.ruleSetSilent(root.ensureRule(), checked); checked = Qt.binding(() => root.rule ? root.rule.silent === true : false) }
        }
        Kirigami.Heading { Layout.topMargin: Kirigami.Units.smallSpacing; level: 4; text: "How they look and behave" }
        LookEditor {
            Layout.fillWidth: true
            model: root.model
            inheritFrom: "the zone and the theme"
            levels: ["Theme", "Zone", "This rule"]
            accent: { const z = root.zoneInfo(root.zoneNow || root.entry.zone); return z && z.accent ? z.accent : "" }
            timeoutMs: root.rule && root.rule.timeout !== "" && root.rule.timeout !== undefined ? Number(root.rule.timeout) : -1
            values: root.rule ? root.rule.style : ({})
            onEdited: (key, text) => root.model.ruleSetStyle(root.ensureRule(), key, text)
        }
    }

    // ---- where it stands, and saving
    RowLayout {
        Layout.fillWidth: true
        Kirigami.Icon { source: root.rule ? "dialog-ok" : "documentinfo"; Layout.preferredWidth: Kirigami.Units.iconSizes.small; Layout.preferredHeight: Layout.preferredWidth }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: !root.anything ? "Pick at least one of the switches above."
                : root.rule ? "Rule “" + root.rule.title + "” (number " + (root.ruleIndex + 1) + " of " + root.rules.length + ")" + (root.model && root.model.dirty ? ", not saved yet" : "")
                : "Any change here creates a rule just for these notifications."
        }
        QQC2.Button { visible: root.rule !== null; text: "Open in Rules"; icon.name: "view-filter"; onClicked: root.openRules() }
        QQC2.Button { text: "Revert"; icon.name: "document-revert"; enabled: root.model && root.model.dirty; onClicked: root.model.rulesReload() }
        QQC2.Button { text: "Save"; icon.name: "document-save"; highlighted: true; enabled: root.model && root.model.dirty && root.model.draftValid && !root.model.checking; onClicked: root.model.rulesSave(false) }
    }
}
