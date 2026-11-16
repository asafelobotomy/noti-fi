// The Overview tab, and what opening the page reads.
import QtQuick
import QtTest

PageHarness {
    id: root

    TestCase {
        id: tc
        name: "OverviewTab"
        when: windowShown

        function init() {
            root.reset();
        }

        function test_reads_on_open_and_watches() {
            const watched = root.fakeCtx.calls.filter(c => c[0] === "watch").map(c => c[1]);
            compare(watched, ["status", "settings", "focus-modes", "config", "rules"]);
            tryVerify(() => root.runs("history").length === 1);
            // nothing is written or erased just by opening the page
            for (const id of ["dnd", "focus", "mute", "unmute", "config-set", "config-reset", "test", "history-clear", "update"])
                compare(root.runs(id).length, 0, id);
            const tabs = ["Overview", "Quiet", "History", "Rules", "Health", "Preferences"];
            for (const t of tabs)
                verify(root.named("tab-" + t) !== null, t);
        }

        // ---------------------------------------------------------------- Overview

        function test_a_change_reads_again() {
            const row = root.shown("Do not disturb");
            compare(row.toggle.checked, false);
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { status: root.status({ dnd: true }) });
            root.fakeCtx.change("status");
            tryCompare(row.toggle, "checked", true);
        }

        function test_dnd_switch() {
            const row = root.shown("Do not disturb");
            compare(row.toggle.checked, false);
            root.flip(row);
            tryCompare(row.toggle, "checked", true);
            compare(root.runs("dnd"), [["run", "dnd", ["on"]]]);
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { dnd: root.statusDoc({ dnd: false }) });
            root.flip(row);
            tryCompare(row.toggle, "checked", false);
            compare(root.runs("dnd")[1], ["run", "dnd", ["off"]]);
        }

        function test_failed_write_restores_and_keeps_the_programs_words_behind_details() {
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { dnd: { code: 1, error: "exit code 1", output: { type: "result", ok: false, message: "the reason in the program's words" } } });
            const row = root.shown("Do not disturb");
            root.flip(row);
            tryVerify(() => root.shown("Couldn't change Do not disturb.") !== null);
            compare(row.toggle.checked, false, "the switch is back on the last value read");
            verify(root.shownText("the reason in the program's words") === null);
            const details = root.shown("Advanced details");
            verify(details !== null);
            root.press(details);
            tryVerify(() => root.shownText("the reason in the program's words") !== null);
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { dnd: root.statusDoc({ dnd: true }) });
            root.flip(row);
            tryVerify(() => root.shown("Couldn't change Do not disturb.") === null);
        }

        function test_focus_modes() {
            const box = root.named("focusBox");
            tryCompare(box, "currentIndex", 2, 5000, "the active mode is chosen");
            compare(JSON.stringify(box.model), JSON.stringify(["Off", "reading", "Work"]));
            root.choose(box, 0);
            tryVerify(() => root.runs("focus").length === 1);
            compare(root.runs("focus")[0], ["run", "focus", ["off"]]);
            tryCompare(box, "currentIndex", 0);
            tryVerify(() => root.pageLoader.item.idle);
            root.choose(box, 1);
            tryVerify(() => root.runs("focus").length === 2);
            compare(root.runs("focus")[1], ["run", "focus", ["reading"]]);
        }

        function test_right_now_counts_only() {
            verify(root.shown("0 popups on screen") !== null);
            verify(root.shown("3 unread notifications") !== null);
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { status: root.status({ active: 2, queued: 1, unread: 1, holding: "fullscreen" }) });
            root.fakeCtx.change("status");
            tryVerify(() => root.shown("2 popups on screen, 1 waiting") !== null);
            verify(root.shown("1 unread notification") !== null);
            verify(root.shown("A full-screen app is in front.") !== null);
        }

        function test_send_a_test_notification() {
            root.press(root.shown("Send a test notification"));
            tryVerify(() => root.shown("Sent one to each group.") !== null);
            compare(root.runs("test"), [["run", "test", []]]);
            root.fakeCtx.commands = Object.assign({}, root.fakeCtx.commands, { test: { code: 1, error: "exit code 1: error: notify-send is missing" } });
            root.press(root.shown("Send a test notification"));
            tryVerify(() => root.shown("Couldn't send a test notification.") !== null);
            verify(root.shown("Sent one to each group.") === null, "the old answer is gone");
        }

        function test_open_noti_fi() {
            const button = root.shown("Open noti-fi");
            verify(button !== null);
            root.press(button);
            compare(root.fakeCtx.calls[root.fakeCtx.calls.length - 1], ["openApp", "org.notify.Panel.desktop"]);
        }
    }
    Component.onCompleted: root.tc = tc
}
