// The Quiet, History and Rules tabs.
import QtQuick
import QtTest

PageHarness {
    id: root

    TestCase {
        id: tc
        name: "QuietHistoryRulesTabs"
        when: windowShown

        function init() {
            root.reset();
        }

        // ---------------------------------------------------------------- Quiet

        function test_mute_and_unmute() {
            root.tab("Quiet");
            const personal = root.shown("Personal");
            const business = root.shown("business");
            verify(personal !== null && business !== null);
            compare(personal.toggle.checked, false);
            compare(business.toggle.checked, true);
            root.flip(personal);
            tryVerify(() => root.runs("mute").length === 1);
            compare(root.runs("mute")[0], ["run", "mute", ["personal"]]);
            tryVerify(() => root.shown("Personal").enabled);
            root.flip(root.shown("business"));
            tryVerify(() => root.runs("unmute").length === 1);
            compare(root.runs("unmute")[0], ["run", "unmute", ["business"]]);
        }

        function test_switches_send_the_planned_pairs() {
            const pairs = [
                [["Quiet", "When an app is full screen"], ["autoDnd.fullscreen", "off"]],
                [["Quiet", "When you share your screen"], ["autoDnd.screenShare", "off"]],
                [["Quiet", "Let urgent notifications through Do not disturb"], ["dnd.allowCritical", "off"]],
                [["History", "Keep after restart"], ["history.persist", "on"]]
            ];
            for (let i = 0; i < pairs.length; ++i) {
                root.tab(pairs[i][0][0]);
                const row = root.shown(pairs[i][0][1]);
                verify(row !== null, pairs[i][0][1]);
                tryVerify(() => row.enabled);
                root.flip(row);
                tryVerify(() => root.runs("config-set").length === i + 1);
                compare(root.runs("config-set")[i], ["run", "config-set", pairs[i][1]]);
                tryVerify(() => root.shown(pairs[i][0][1]).enabled);
            }
        }

        function test_a_refused_switch_shows_why_under_its_row() {
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { "config-set": { code: 1, error: "exit code 1: error: notifd refused this change (nope); nothing was changed" } });
            root.tab("Quiet");
            const row = root.shown("When an app is full screen");
            root.flip(row);
            tryVerify(() => root.shown("Couldn't change Do not disturb for full screen.") !== null);
            compare(row.toggle.checked, true, "the switch is back on the file's value");
        }

        // ---------------------------------------------------------------- History

        function test_limit_box() {
            root.tab("History");
            const box = root.named("limitBox");
            tryCompare(box, "currentIndex", 2, 5000, "200 is chosen");
            compare(JSON.stringify(box.model), JSON.stringify(["50", "100", "200", "500", "1000"]));
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { settings: root.settings({ history: { limit: 500, persist: false } }) });
            root.choose(box, 3);
            tryVerify(() => root.runs("config-set").length === 1);
            compare(root.runs("config-set"), [["run", "config-set", ["history.limit", "500"]]]);
            tryVerify(() => root.runs("config").length >= 1);
        }

        function test_failed_limit_restores() {
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { "config-set": { code: 1, error: "exit code 1: error: refused" } });
            root.tab("History");
            const box = root.named("limitBox");
            tryCompare(box, "currentIndex", 2);
            root.choose(box, 3);
            tryVerify(() => root.shown("Couldn't change how many notifications are kept.") !== null);
            compare(box.currentIndex, 2, "the box shows the last value read");
        }

        function test_recent_notifications_show_who_sent_them_and_never_what_they_said() {
            root.tab("History");
            tryVerify(() => root.shown("Chat") !== null && root.shown("Mail") !== null);
            verify(root.shown("personal · Do not disturb") !== null, "a held-back one says why");
            for (const t of root.everyText())
                verify(t.indexOf("SECRET") < 0 && t.indexOf("secret") < 0, "shows notification text: " + t);
            root.choose(root.named("filterBox"), 1);
            tryVerify(() => root.runs("history-held").length === 1);
            tryVerify(() => root.shown("Chat") === null && root.shown("Mail") !== null);
            root.choose(root.named("filterBox"), 0);
            tryVerify(() => root.runs("history").length === 2);
        }

        function test_the_history_follows_the_count() {
            root.tab("History");
            tryVerify(() => root.runs("history").length === 1);
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { status: root.status({ history: 43 }) });
            root.fakeCtx.change("status");
            tryVerify(() => root.runs("history").length === 2);
            root.fakeCtx.change("status");
            wait(50);
            compare(root.runs("history").length, 2, "the same count reads nothing again");
        }

        function test_clear_history_asks_first() {
            root.tab("History");
            const button = root.shown("Clear history…");
            verify(button !== null);
            // Cancel: nothing changes and nothing is shown
            root.fakeCtx.confirms = false;
            root.press(button);
            tryVerify(() => root.runs("history-clear").length === 1);
            wait(50);
            verify(root.shown("Couldn't clear the history.") === null);
            // Confirm: it runs, then the state and the list are read again
            root.fakeCtx.confirms = true;
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { status: root.status({ history: 0 }), history: { code: 0, output: [] } });
            root.press(root.shown("Clear history…"));
            tryVerify(() => root.shown("No notifications yet.") !== null);
            compare(root.runs("history-clear").length, 2);
            tryVerify(() => !root.shown("Clear history…").enabled, 5000, "nothing left to clear");
        }

        // ---------------------------------------------------------------- Rules

        function test_rules_and_their_check() {
            root.tab("Rules");
            verify(root.shown("Quiet chat") !== null);
            verify(root.shown("Rule 2") !== null);
            verify(root.shown("Change your rules in noti-fi's own app.") !== null);
            verify(root.shown("rule 2 never matches anything") !== null);
            verify(root.shown("No problems found.") === null, "a warning is not 'no problems'");
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { rules: { code: 1, error: "exit code 1", output: { file: "x", errors: ["rule 3: not a valid pattern"], warnings: [], daemon: null } } });
            root.fakeCtx.change("rules");
            tryVerify(() => root.shown("A rule has an error") !== null);
            verify(root.shown("rule 3: not a valid pattern") !== null);
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { rules: { code: 0, output: { file: "x", errors: [], warnings: [], daemon: null } } });
            root.fakeCtx.change("rules");
            tryVerify(() => root.shown("No problems found.") !== null);
        }
    }
    Component.onCompleted: root.tc = tc
}
