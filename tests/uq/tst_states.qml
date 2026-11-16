// What the page shows while loading, when noti-fi isn't running, and when a read fails.
import QtQuick
import QtTest

PageHarness {
    id: root

    TestCase {
        id: tc
        name: "PageStates"
        when: windowShown

        function init() {
            root.reset();
        }

        // ---------------------------------------------------------------- states

        function test_no_notification_text() {
            const texts = root.everyText();
            verify(texts.length > 10);
            for (const t of texts) {
                verify(t.indexOf("secret") < 0, "shows notification text: " + t);
                verify(t.indexOf("Quarterly") < 0, "shows notification text: " + t);
            }
        }

        function test_untrusted_text_is_shown_as_it_is() {
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, {
                settings: root.settings({ zones: [{ id: "business", label: "<b>x</b>" }, { id: "personal", label: "‮evil" }], rules: [{ name: "<i>y</i> https://example.org" }] })
            });
            root.fakeCtx.change("settings");
            root.tab("Quiet");
            tryVerify(() => root.shown("<b>x</b>") !== null);
            verify(root.shown("‮evil") !== null);
            root.tab("Rules");
            verify(root.shown("<i>y</i> https://example.org") !== null);
        }

        function test_not_running() {
            root.fakeCtx.calls = [];
            root.fakeCtx.commands = Object.assign({}, root.answers(), { status: { code: 0, output: { owner: null, notifd_owns: false, jobs_owner: null, daemon: null, config: null } } });
            root.open();
            tryVerify(() => root.shown("noti-fi isn't running") !== null);
            verify(root.shown("noti-fi isn't running. uQ's own notifications are in use.") !== null);
            verify(!root.shown("Do not disturb").enabled, "no controls while it isn't running");
            verify(!root.shown("Send a test notification").enabled);
            for (const t of root.everyText())
                verify(t.indexOf("Start") !== 0, "there is no start button: " + t);
            compare(root.runs("dnd").length, 0);
            // the preferences are the file's, so they can still be read
            root.tab("Preferences");
            compare(root.named("field-width").text, "380");
        }

        function test_a_failed_read_shows_the_error() {
            root.fakeCtx.commands = Object.assign({}, root.answers(), { status: { error: "timed out" } });
            root.open();
            tryVerify(() => root.shown("Couldn't read noti-fi's state") !== null);
            verify(root.shown("timed out") !== null);
            verify(!root.shown("Do not disturb").enabled);
        }

        function test_unreadable_settings_disable_their_rows() {
            root.fakeCtx.commands = Object.assign({}, root.answers(), { settings: { code: 1, error: "exit code 1: error: broken" }, config: { code: 1, error: "exit code 1: error: broken" } });
            root.open();
            tryVerify(() => root.shown("Couldn't read noti-fi's settings.") !== null);
            verify(root.shown("Do not disturb").enabled, "the daemon's own state still works");
            root.tab("History");
            verify(!root.shown("Keep after restart").enabled);
            verify(!root.named("limitBox").enabled);
            root.tab("Quiet");
            verify(!root.shown("When an app is full screen").enabled);
        }

        function test_settings_are_read_again_when_noti_fi_comes_up() {
            const down = { code: 1, error: "exit code 1: error: notifd is not running" };
            root.fakeCtx.commands = Object.assign({}, root.answers(), {
                status: { code: 0, output: { owner: null, notifd_owns: false, jobs_owner: null, daemon: null, config: null } },
                settings: down,
                "focus-modes": down
            });
            root.fakeCtx.calls = [];
            root.fakeCtx.watches = ({});
            root.open();
            tryVerify(() => root.shown("noti-fi isn't running") !== null);
            tryVerify(() => root.runs("settings").length === 1 && root.runs("focus-modes").length === 1);
            wait(50);
            root.fakeCtx.commands = root.answers();
            root.fakeCtx.change("status");
            tryVerify(() => root.shown("Do not disturb") !== null && root.shown("Do not disturb").enabled);
            tryCompare(root.named("focusBox"), "currentIndex", 2);
            verify(root.shown("Couldn't read noti-fi's settings.") === null);
        }

        function test_a_rules_file_error_is_shown() {
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { status: root.status({ cfgError: "rule 3: something is wrong" }) });
            root.fakeCtx.change("status");
            tryVerify(() => root.shown("rule 3: something is wrong") !== null);
            verify(root.shown("noti-fi couldn't use your latest rules, so it is using the earlier ones.") !== null);
        }

        function test_a_skeleton_shows_while_loading() {
            root.fakeCtx.commands = Object.assign({}, root.answers(), { status: function () { return undefined; } });
            let bar = null;
            root.walk(root.pageLoader.item, it => {
                if (!bar && it.indeterminate === true && it.visible)
                    bar = it;
            });
            // the first answer has arrived by now; a page that has not heard back yet shows an indeterminate bar
            root.pageLoader.source = "";
            root.fakeCtx.calls = [];
            root.fakeCtx.commands = ({});
            root.pageLoader.source = root.pageUrl;
            root.pageLoader.item.ctx = root.fakeCtx.ctx;
            tryVerify(() => root.inside(root.pageLoader.item, it => it.indeterminate === true) !== null);
        }
    }
    Component.onCompleted: root.tc = tc
}
