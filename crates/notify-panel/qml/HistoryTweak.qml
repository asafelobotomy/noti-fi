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
    // words from the notification that it must contain (title words are looked for in the title, text words in the text)
    property var keywords: []
    readonly property var titleWords: root.wordsOf(root.entry.summary)
    readonly property var textWords: root.wordsOf(root.entry.body).filter(w => root.titleWords.indexOf(w) < 0)
    function wordsOf(t) {
        const seen = []
        for (const w of String(t || "").toLowerCase().split(/[^a-z0-9\u00c0-\u024f\u0400-\u04ff@#._-]+/)) {
            const x = w.replace(/^[._-]+|[._-]+$/g, "")
            if (x.length >= 3 && !/^\d+$/.test(x) && seen.indexOf(x) < 0) seen.push(x)
        }
        return seen.slice(0, 12)
    }
    function toggleWord(w) { const k = root.keywords.slice(); const i = k.indexOf(w); if (i >= 0) k.splice(i, 1); else k.push(w); root.keywords = k }
    function esc(t) { let o = ""; for (const c of String(t)) o += (".^$*+?()[]{}|\\".indexOf(c) >= 0 ? "\\" : "") + c; return o }
    readonly property string appField: (root.entry.desktopEntry || "") !== "" ? "desktopEntry" : "appName"
    readonly property var match: {
        const m = { appName: "", desktopEntry: "", origin: "", summary: "", body: "", urgency: "", category: "", channel: "" }
        if (root.useApp) m[root.appField] = "^" + root.esc(root.appField === "desktopEntry" ? root.entry.desktopEntry : root.entry.app) + "$"
        if (root.useSite && root.entry.origin) m.origin = "\\b" + root.esc(root.entry.origin) + "\\b"
        if (root.useType && root.entry.category) m.category = "^" + root.esc(root.entry.category)
        const inTitle = root.keywords.filter(w => root.titleWords.indexOf(w) >= 0), inText = root.keywords.filter(w => root.titleWords.indexOf(w) < 0)
        if (inTitle.length) m.summary = inTitle.map(root.esc).join("|")
        if (inText.length) m.body = inText.map(root.esc).join("|")
        return m
    }
    readonly property bool anything: Object.keys(root.match).some(k => root.match[k] !== "")
    readonly property string ruleName: "From History: " + [root.useApp ? root.entry.app : "", root.useSite ? root.entry.origin : "", root.useType ? root.entry.category : ""].concat(root.keywords.map(w => "“" + w + "”")).filter(x => x).join(" · ")

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

    Kirigami.Heading { level: 4; text: "Put notifications like this in a group, or change how they're shown" }

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
    RowLayout {
        visible: root.titleWords.length + root.textWords.length > 0
        Layout.fillWidth: true
        QQC2.Label { text: "And mentions:"; opacity: 0.8; Layout.alignment: Qt.AlignTop }
        Flow {
            Layout.fillWidth: true
            spacing: 4
            Repeater {
                model: root.titleWords.concat(root.textWords)
                delegate: QQC2.Button {
                    required property string modelData
                    text: modelData

                    checkable: true
                    checked: root.keywords.indexOf(modelData) >= 0
                    onToggled: { root.toggleWord(modelData); checked = Qt.binding(() => root.keywords.indexOf(modelData) >= 0) }
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: root.titleWords.indexOf(modelData) >= 0 ? "Only notifications with this word in the title" : "Only notifications with this word in the text"
                }
            }
        }
    }
    RulePreview {
        Layout.fillWidth: true
        recent: root.recent
        rule: ({ mAppName: root.match.appName, mDesktopEntry: root.match.desktopEntry, mOrigin: root.match.origin, mSummary: root.match.summary, mBody: root.match.body, mGroup: "",
                 mUrgency: "", mCategory: root.match.category, mChannel: "" })
    }

    ColumnLayout {
        enabled: root.anything
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing
        QQC2.Label { text: "Group"; font.bold: true }
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
            Segmented {
                Layout.fillWidth: true
                accessibleName: "Group"
                options: root.zoneIds.map(id => ({ value: id, label: id === "" ? "As now (" + ((root.zoneInfo(root.entry.zone) || {}).label || root.entry.zone || "default") + ")" : ((root.zoneInfo(id) || {}).label || id) }))
                value: root.zoneNow
                onPicked: v => root.model.ruleSetZone(root.ensureRule(), v)
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
            inheritFrom: "the group and the theme"
            levels: ["Theme", "Group", "This rule"]
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
