import QtQuick
import QtTest
import "../../crates/notify-panel/qml"

// Changing how notifications like one in History are shown: the rule it builds, where it goes, and that later changes edit the same rule.
Item {
    width: 900; height: 1400
    // a stand-in for the panel model: just enough of the draft to see what the editor asks for
    QtObject {
        id: fake
        property var list: [{ title: "Broad", match: { appName: "", desktopEntry: "", origin: "", summary: "", body: "", urgency: "", category: "", channel: "" }, zone: "business", timeout: "", silent: false, style: {} }]
        property string rulesJson: JSON.stringify(list)
        property string settingsJson: JSON.stringify({ values: {}, zones: [{ id: "business", label: "Business", anchor: "top-left" }, { id: "personal", label: "Personal", anchor: "top-right" }] })
        property bool dirty: false; property bool draftValid: true; property bool checking: false
        property var calls: []
        function commit() { rulesJson = JSON.stringify(list); dirty = true }
        function ruleAdd() { const l = list.slice(); l.push({ title: "(unnamed rule)", match: { appName: "", desktopEntry: "", origin: "", summary: "", body: "", urgency: "", category: "", channel: "" }, zone: "", timeout: "", silent: false, style: {} }); list = l; commit(); calls.push("add"); return list.length - 1 }
        function ruleSetName(i, n) { list[i].title = n; commit() }
        function ruleSetMatch(i, f, p) { list[i].match[f] = p; commit() }
        function ruleMove(i, d) { const l = list.slice(); const r = l.splice(i, 1)[0]; l.splice(i + d, 0, r); list = l; commit(); return i + d }
        function ruleSetZone(i, z) { list[i].zone = z; commit(); calls.push("zone " + i + " " + z) }
        function ruleSetTimeout(i, t) { list[i].timeout = t; commit(); calls.push("timeout " + i + " " + t) }
        function ruleSetSilent(i, v) { list[i].silent = v; commit() }
        function ruleSetStyle(i, k, t) { list[i].style[k] = t; commit() }
        function rulesReload() {}
        function rulesSave() {}
    }
    HistoryTweak {
        id: tweak
        width: 800
        model: fake
        entry: ({ id: 7, app: "Brave", desktopEntry: "brave-browser", origin: "shop.example", category: "im.received", zone: "personal", summary: "Invoice ready", body: "Your invoice 42 from Acme" })
    }
    TestCase {
        name: "HistoryTweak"
        when: windowShown
        function test_like_this_matches_the_app_site_and_type() {
            compare(tweak.match.desktopEntry, "^brave-browser$")
            compare(tweak.match.origin, "\\bshop\\.example\\b")
            compare(tweak.match.category, "^im\\.received")
            compare(tweak.ruleIndex, -1, "no rule yet")
        }
        function test_the_first_change_creates_one_rule_at_the_top_and_later_ones_edit_it() {
            fake.ruleSetZone(tweak.ensureRule(), "business")
            compare(fake.list.length, 2)
            compare(fake.list[0].title, "From History: Brave · shop.example · im.received")
            compare(fake.list[0].match.origin, "\\bshop\\.example\\b")
            compare(tweak.ruleIndex, 0, "it is at the top, ahead of the broader rule")
            fake.ruleSetTimeout(tweak.ensureRule(), "15000")
            compare(fake.list.length, 2, "the same rule is edited, not a second one made")
            compare(fake.list[0].timeout, "15000")
        }
        function test_keywords_come_from_the_notification_and_land_in_the_right_field() {
            compare(JSON.stringify(tweak.titleWords), '["invoice","ready"]')
            verify(tweak.textWords.indexOf("acme") >= 0 && tweak.textWords.indexOf("invoice") < 0 && tweak.textWords.indexOf("42") < 0, JSON.stringify(tweak.textWords))
            tweak.toggleWord("invoice"); tweak.toggleWord("acme")
            compare(tweak.match.summary, "invoice")
            compare(tweak.match.body, "acme")
            verify(tweak.ruleName.indexOf("“invoice”") >= 0)
            tweak.keywords = []
        }
        function test_switching_a_part_off_matches_less() {
            tweak.useType = false
            compare(tweak.match.category, "")
            verify(tweak.ruleName.indexOf("im.received") < 0)
            tweak.useType = true
        }
    }
}
