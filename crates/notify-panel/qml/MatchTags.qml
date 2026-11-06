import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// One match field of a rule, edited as tags ("Signal", "Slack") with suggestions taken from history and the app list.
// The daemon still stores a regular expression: tags are written as one (kind says how), and a pattern that is not a plain
// list of words (anything with real regex syntax) stays editable as text, so nothing is ever lost.
ColumnLayout {
    id: root
    property string value: ""
    property string kind: "contains"       // exact | choice (exact, bare words accepted) | word | start | contains: how the tags are wrapped into a pattern
    property string placeholder: ""
    property var suggestions: []
    property string label: ""
    signal edited(string text)

    property bool rawMode: false
    spacing: Kirigami.Units.smallSpacing

    readonly property string metas: ".^$*+?()[]{}|\\"
    function esc(t) { let o = ""; for (const c of t) o += (root.metas.indexOf(c) >= 0 ? "\\" : "") + c; return o }
    // split a pattern on "|" that is not escaped, unescaping each piece; null when a piece holds real regex syntax
    function words(inner) {
        const out = []
        let cur = ""
        for (let i = 0; i < inner.length; i++) {
            const c = inner[i]
            if (c === "\\") { if (i + 1 >= inner.length || root.metas.indexOf(inner[i + 1]) < 0) return null; cur += inner[++i] }
            else if (c === "|") { if (cur === "") return null; out.push(cur); cur = "" }
            else if (root.metas.indexOf(c) >= 0) return null
            else cur += c
        }
        if (cur === "") return null
        out.push(cur)
        return out
    }
    // the tags a pattern stands for, or null when it is more than a list of words
    function parse(v, kind) {
        if (v === "") return []
        let inner = null, m
        if (kind === "exact") { m = /^\^\(([^()]*)\)\$$/.exec(v) || /^\^([^()]*)\$$/.exec(v) }
        else if (kind === "choice") { m = /^\^\(([^()]*)\)\$$/.exec(v) || /^\^([^()]*)\$$/.exec(v) || /^([^()^$]*)$/.exec(v) }   // a closed set: a bare word means the same
        else if (kind === "word") { m = /^\\b\(([^()]*)\)\\b$/.exec(v) || /^\\b([^()]*)\\b$/.exec(v) }
        else if (kind === "start") { m = /^\^\(([^()]*)\)$/.exec(v) || /^\^([^()]*)$/.exec(v) }
        else { m = /^([^()^$]*)$/.exec(v) }
        if (!m) return null
        return root.words(m[1])
    }
    function build(tags, kind) {
        if (tags.length === 0) return ""
        const body = tags.map(esc).join("|")
        const many = tags.length > 1
        if (kind === "exact" || kind === "choice") return many ? "^(" + body + ")$" : "^" + body + "$"
        if (kind === "word") return many ? "\\b(" + body + ")\\b" : "\\b" + body + "\\b"
        if (kind === "start") return many ? "^(" + body + ")" : "^" + body
        return body
    }

    readonly property var tags: root.parse(root.value, root.kind)
    readonly property bool asText: root.rawMode || root.tags === null
    function add(t) {
        t = t.trim()
        if (t === "" || root.tags === null || root.tags.some(x => x.toLowerCase() === t.toLowerCase())) return
        root.edited(root.build(root.tags.concat([t]), root.kind))
    }
    function removeAt(i) {
        const n = root.tags.slice(); n.splice(i, 1)
        root.edited(root.build(n, root.kind))
    }

    // ---- tags
    Flow {
        Layout.fillWidth: true
        visible: !root.asText
        spacing: Kirigami.Units.smallSpacing
        Repeater {
            model: root.tags === null ? [] : root.tags
            delegate: Rectangle {
                id: chip
                required property string modelData
                required property int index
                implicitWidth: chipRow.implicitWidth + Kirigami.Units.smallSpacing * 2
                implicitHeight: chipRow.implicitHeight + 2
                radius: height / 2
                color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.22)
                border.width: 1
                border.color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.6)
                RowLayout {
                    id: chipRow
                    anchors.centerIn: parent
                    spacing: 2
                    QQC2.Label { Layout.leftMargin: Kirigami.Units.smallSpacing; text: chip.modelData; textFormat: Text.PlainText }
                    QQC2.ToolButton {
                        icon.name: "window-close"
                        display: QQC2.AbstractButton.IconOnly
                        focusPolicy: Qt.NoFocus
                        implicitWidth: Kirigami.Units.iconSizes.smallMedium; implicitHeight: implicitWidth
                        onClicked: root.removeAt(chip.index)
                        Accessible.name: "Remove " + chip.modelData
                    }
                }
            }
        }
    }

    // ---- add a tag (or edit the pattern as text)
    RowLayout {
        Layout.fillWidth: true
        QQC2.TextField {
            id: input
            Layout.fillWidth: true
            visible: !root.asText
            placeholderText: (root.tags && root.tags.length) ? "Add another…" : (root.placeholder !== "" ? root.placeholder : "Any (type or pick one)")
            onAccepted: { root.add(text); text = "" }
            Keys.onPressed: event => { if (event.key === Qt.Key_Comma) { root.add(text); text = ""; event.accepted = true } }
            Accessible.name: "Add " + root.label

            readonly property var matches: {
                const q = text.trim().toLowerCase(), have = (root.tags || []).map(x => x.toLowerCase())
                return (root.suggestions || []).filter(s => have.indexOf(s.toLowerCase()) < 0 && (q === "" || s.toLowerCase().indexOf(q) >= 0)).slice(0, 8)
            }
            QQC2.Popup {
                id: pop
                y: input.height
                width: input.width
                padding: 1
                focus: false
                closePolicy: QQC2.Popup.CloseOnEscape
                visible: input.activeFocus && input.matches.length > 0
                contentItem: ListView {
                    implicitHeight: Math.min(contentHeight, Kirigami.Units.gridUnit * 10)
                    clip: true
                    model: input.matches
                    delegate: QQC2.ItemDelegate {
                        required property string modelData
                        width: ListView.view.width
                        focusPolicy: Qt.NoFocus
                        text: modelData
                        onClicked: { root.add(modelData); input.text = ""; input.forceActiveFocus() }
                    }
                }
            }
        }
        QQC2.TextField {
            id: rawField
            Layout.fillWidth: true
            visible: root.asText
            font.family: "monospace"
            placeholderText: root.placeholder !== "" ? "e.g. " + root.placeholder : ""
            Binding on text { when: !rawField.activeFocus; value: root.value }
            onTextEdited: root.edited(text)
            Accessible.name: "Pattern for " + root.label
        }
        QQC2.ToolButton {
            // text is always available; tags only when the pattern is a plain list
            visible: root.asText ? root.tags !== null : true
            icon.name: root.asText ? "tag" : "code-context"
            display: QQC2.AbstractButton.IconOnly
            onClicked: root.rawMode = !root.asText
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.text: root.asText ? "Back to tags" : "Edit as a regular expression"
            Accessible.name: root.asText ? "Use tags for " + root.label : "Edit " + root.label + " as a regular expression"
        }
    }
    QQC2.Label {
        Layout.fillWidth: true
        visible: root.tags === null
        wrapMode: Text.Wrap
        opacity: 0.7
        font: Kirigami.Theme.smallFont
        text: "This is a regular expression, so it is edited as text."
    }
}
