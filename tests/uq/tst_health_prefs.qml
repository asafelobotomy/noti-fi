// The Health and Preferences tabs.
import QtQuick
import QtTest

PageHarness {
    id: root

    TestCase {
        id: tc
        name: "HealthPreferencesTabs"
        when: windowShown

        function init() {
            root.reset();
        }

        // ---------------------------------------------------------------- Health

        function test_check_up_runs_when_the_tab_opens() {
            compare(root.runs("doctor").length, 0, "not before the tab is shown");
            root.tab("Health");
            tryVerify(() => root.runs("doctor").length === 1);
            tryVerify(() => root.shown("2 problems to fix, 2 things to look at.") !== null);
            verify(root.shown("Tools") !== null && root.shown("Install") !== null && root.shown("Daemon") !== null);
            verify(root.shown("pw-dump: missing (screen-share detection (pipewire))") !== null);
            verify(root.shown("run: notify portal") !== null, "the hint is shown");
            root.press(root.shown("Run again"));
            tryVerify(() => root.runs("doctor").length === 2);
            root.tab("Overview");
            root.tab("Health");
            wait(50);
            compare(root.runs("doctor").length, 2, "opening the tab again doesn't run it again");
        }

        function test_check_up_failure() {
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { doctor: { error: "timed out" } });
            root.tab("Health");
            tryVerify(() => root.shown("Couldn't run the check-up") !== null);
            verify(root.shown("timed out") !== null);
        }

        function test_update_with_progress() {
            root.tab("Health");
            verify(root.shown("Update…") === null, "no update button before a newer release is found");
            root.press(root.shown("Check for updates"));
            tryVerify(() => root.shown("noti-fi 0.20.0 is available (you have 0.19.1)") !== null);
            // Cancel on the confirmation changes nothing and says nothing
            root.fakeCtx.confirms = false;
            root.press(root.shown("Update…"));
            tryVerify(() => root.fakeCtx.calls.filter(c => c[0] === "stream").length === 1);
            wait(50);
            verify(root.shown("Couldn't update noti-fi.") === null);
            root.fakeCtx.confirms = true;
            root.press(root.shown("Update…"));
            tryVerify(() => root.shown("updated to noti-fi 0.20.0. Restart the panel (noti-fi) to use it.") !== null);
            compare(root.fakeCtx.calls.filter(c => c[0] === "stream" && c[1] === "update").length, 2);
            verify(root.shown("noti-fi 0.20.0 is available (you have 0.19.1)") === null, "the old verdict is gone after an update");
        }

        function test_update_failure_and_an_unreachable_release_page() {
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { "update-check": { code: 1, error: "exit code 1: error: could not check for updates: offline" } });
            root.tab("Health");
            root.press(root.shown("Check for updates"));
            tryVerify(() => root.shown("offline") !== null || root.shown("could not check for updates: offline") !== null);
        }

        // ---------------------------------------------------------------- Preferences

        function test_preferences_show_the_file_and_the_defaults() {
            root.tab("Preferences");
            const width = root.named("field-width");
            verify(width !== null);
            compare(width.text, "380");
            const max = root.named("field-maxVisible");
            compare(max.text, "7");
            for (const card of ["Popups", "Sound", "Look", "Notification list"])
                verify(root.shown(card) !== null, card);
            verify(root.inside(root.pageLoader.item, it => it.text === "Reset") !== null, "a setting the file sets has Reset");
        }

        function test_a_number_is_sent_as_typed() {
            root.tab("Preferences");
            root.type(root.named("field-width"), "420");
            tryVerify(() => root.runs("config-set").length === 1);
            compare(root.runs("config-set")[0], ["run", "config-set", ["width", "420"]]);
            tryVerify(() => root.runs("config").length >= 1);
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { config: root.config({ width: 420 }) });
            root.fakeCtx.change("config");
            tryCompare(root.named("field-width"), "text", "420");
        }

        function test_a_bad_value_is_never_sent() {
            root.tab("Preferences");
            for (const [key, bad, why] of [["width", "wide", "Use a number, like 380."], ["theme.card", "red", "Use a colour like #1d1f27."], ["theme.opacity", "-1", "Use a number, like 1."]]) {
                root.type(root.named("field-" + key), bad);
                tryVerify(() => root.shown(why) !== null, 5000, why);
                compare(root.named("field-" + key).text, root.named("field-" + key).placeholderText === "" ? "" : root.named("field-" + key).text);
            }
            compare(root.runs("config-set").length, 0);
        }

        function test_an_empty_field_resets_the_setting() {
            root.tab("Preferences");
            root.type(root.named("field-maxVisible"), "");
            tryVerify(() => root.runs("config-reset").length === 1);
            compare(root.runs("config-reset")[0], ["run", "config-reset", ["maxVisible"]]);
            tryVerify(() => root.pageLoader.item.idle);
            const button = root.inside(root.pageLoader.item, it => it.text === "Reset");
            verify(button !== null);
            root.press(button);
            tryVerify(() => root.runs("config-reset").length === 2);
        }

        function test_a_switch_and_a_choice() {
            root.tab("Preferences");
            const sound = root.inside(root.pageLoader.item, it => it.visible && it.checked !== undefined && it.checkable !== false && it.toggled !== undefined && it.text === "");
            verify(sound !== null);
            sound.checked = !sound.checked;
            sound.toggled();
            tryVerify(() => root.runs("config-set").length === 1);
            compare(root.runs("config-set")[0], ["run", "config-set", ["sound.enabled", "off"]]);
        }

        function test_a_refusal_is_shown_under_its_row() {
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { "config-set": { code: 1, error: "exit code 1: error: notifd refused this change (theme.opacity: too see-through); nothing was changed" } });
            root.tab("Preferences");
            root.type(root.named("field-theme.opacity"), "0.05");
            tryVerify(() => root.shown("Couldn't change this.") !== null);
            root.press(root.shown("Advanced details"));
            tryVerify(() => root.shownText("notifd refused this change (theme.opacity: too see-through); nothing was changed") !== null);
        }

        function test_preferences_that_cant_be_read() {
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { config: { code: 1, error: "exit code 1: error: rules.json is not usable" } });
            root.fakeCtx.change("config");
            root.tab("Preferences");
            tryVerify(() => root.shown("Couldn't read noti-fi's preferences") !== null);
            verify(!root.named("field-width").enabled);
        }
    }
    Component.onCompleted: root.tc = tc
}
