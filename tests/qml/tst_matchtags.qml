import QtQuick
import QtTest
import "../../crates/notify-panel/qml"

// The rule editor's tags: how a list of words becomes a pattern and back, and that real regular expressions are left alone.
Item {
    MatchTags { id: t }
    TestCase {
        name: "MatchTags"
        function test_exact_roundtrip() {
            compare(t.build(["Steam"], "exact"), "^Steam$")
            compare(t.build(["Steam", "Slack"], "exact"), "^(Steam|Slack)$")
            compare(JSON.stringify(t.parse("^(Steam|Slack)$", "exact")), '["Steam","Slack"]')
            compare(JSON.stringify(t.parse("^Steam$", "exact")), '["Steam"]')
        }
        function test_word_and_start_and_contains() {
            compare(JSON.stringify(t.parse("\\b(onlyfans|fansly)\\b", "word")), '["onlyfans","fansly"]')
            compare(t.build(["a", "b"], "word"), "\\b(a|b)\\b")
            compare(t.build(["email"], "start"), "^email")
            compare(JSON.stringify(t.parse("^(email|chat)", "start")), '["email","chat"]')
            compare(t.build(["hello", "bye"], "contains"), "hello|bye")
            compare(JSON.stringify(t.parse("hello|bye", "contains")), '["hello","bye"]')
        }
        function test_special_characters_are_escaped_and_restored() {
            compare(t.build(["example.com"], "word"), "\\bexample\\.com\\b")
            compare(JSON.stringify(t.parse("\\bexample\\.com\\b", "word")), '["example.com"]')
        }
        function test_real_regex_stays_text() {
            compare(t.parse("^(kernel|linux.*)$", "exact"), null)
            compare(t.parse("^steam.*$", "exact"), null)
            compare(t.parse("a(b", "contains"), null)
            compare(t.parse("^Steam$", "contains"), null)   // an anchored pattern is not a plain "contains" list
        }
        function test_a_closed_set_accepts_a_bare_word() {
            compare(JSON.stringify(t.parse("critical", "choice")), '["critical"]')
            compare(JSON.stringify(t.parse("^(low|critical)$", "choice")), '["low","critical"]')
            compare(t.build(["critical"], "choice"), "^critical$")
            compare(t.parse("critical", "exact"), null)
        }
        function test_empty_means_no_tags() {
            compare(JSON.stringify(t.parse("", "exact")), "[]")
            compare(t.build([], "exact"), "")
        }
    }
}
