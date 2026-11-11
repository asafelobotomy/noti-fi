// The Notifications page against a fake ctx and the stub org.uq.settings module:
//   QT_QPA_PLATFORM=offscreen qmltestrunner -import tests/uq/stub -input tests/uq
// Changes are made the way a person makes them: a switch flips (its `toggled` signal), a box is chosen
// (`activated`), a button is pressed (`clicked`). The fake ctx logs every request in `fake.calls`.
import QtQuick
import QtTest

Item {
    id: root
    width: 600
    height: 400

    readonly property url pageUrl: Qt.resolvedUrl("../../uq/pages/noti-fi/Page.qml")

    // `notify status --json` with noti-fi running. `latest.summary` is notification text: the page must never show it.
    function status(extra) {
        const daemon = Object.assign({
            dnd: false, muted: ["business"], zones: ["business", "personal"], active: 0, queued: 0, cap: 5,
            history: 42, cfgError: "", focus: "work", unread: 3, sharing: false,
            latest: { id: 17, t: 1, appName: "Chat", appIcon: "", summary: "secret", read: false }
        }, extra || {});
        return { code: 0, output: { owner: null, notifd_owns: true, jobs_owner: null, daemon: daemon, config: null } };
    }

    // the answer of dnd, mute, unmute and focus: one bare Status
    function statusDoc(extra) {
        return { code: 0, output: status(extra).output.daemon };
    }

    function settings(extra) {
        return { code: 0, output: Object.assign({
            dnd: { allowCritical: true },
            history: { limit: 200, persist: false },
            autoDnd: { fullscreen: true, screenShare: true },
            zones: [{ id: "business", label: "Business" }, { id: "personal", label: "Personal" }],
            rules: [{ name: "Quiet chat" }, { name: "Rule 2" }]
        }, extra || {}) };
    }

    function answers() {
        return {
            status: status(),
            settings: settings(),
            "focus-modes": { code: 0, output: { active: "work", modes: { reading: {}, work: { label: "Work" } } } },
            dnd: statusDoc({ dnd: true }),
            focus: statusDoc({ focus: "" }),
            mute: statusDoc({ muted: ["business", "personal"] }),
            unmute: statusDoc({ muted: [] }),
            set: { code: 0, output: "done\n" },
            "history-clear": { code: 0, output: "history cleared\n" }
        };
    }

    FakeContext {
        id: fake
        commands: root.answers()
        confirmed: ["history-clear"]
    }

    Loader {
        id: loader
        anchors.fill: parent
    }

    function walk(item, visit) {
        visit(item);
        for (let i = 0; i < item.children.length; ++i)
            walk(item.children[i], visit);
    }

    /// The first visible item whose `text` (or `label`, `title`, `detail`) is exactly `text`.
    function shown(text) {
        let hit = null;
        walk(loader.item, it => {
            if (!hit && it.visible && (it.text === text || it.label === text || it.title === text || it.detail === text))
                hit = it;
        });
        return hit;
    }

    function named(name) {
        let hit = null;
        walk(loader.item, it => {
            if (!hit && it.objectName === name)
                hit = it;
        });
        return hit;
    }

    /// Every text, label, title and detail the page holds, shown or not.
    function everyText() {
        const all = [];
        walk(loader.item, it => {
            for (const k of ["text", "label", "title", "detail"])
                if (typeof it[k] === "string" && it[k] !== "")
                    all.push(it[k]);
        });
        return all;
    }

    function flip(row) {
        row.toggle.checked = !row.toggle.checked;
        row.toggle.toggled();
    }

    function choose(box, index) {
        box.currentIndex = index;
        box.activated(index);
    }

    function press(button) {
        button.clicked();
    }

    function runs(id) {
        return fake.calls.filter(c => c[0] === "run" && c[1] === id);
    }

    function open() {
        loader.source = "";
        loader.source = root.pageUrl;
        verify(loader.status === Loader.Ready, "the page loads");
        loader.item.ctx = fake.ctx;
    }

    TestCase {
        name: "NotificationsPage"
        when: windowShown

        function init() {
            fake.calls = [];
            fake.watches = ({});
            fake.confirms = true;
            fake.commands = root.answers();
            root.open();
            tryVerify(() => root.shown("Do not disturb") !== null);
        }

        function test_reads_on_open_and_watches() {
            compare(fake.calls.slice(0, 6), [
                ["watch", "status"], ["run", "status", []],
                ["watch", "settings"], ["run", "settings", []],
                ["watch", "focus-modes"], ["run", "focus-modes", []]
            ]);
            tryVerify(() => root.shown("Mute Business") !== null && root.shown("Quiet chat") !== null);
            verify(root.shown("42 notifications are kept.") !== null);
        }

        function test_a_change_reads_again() {
            const row = root.shown("Do not disturb");
            compare(row.toggle.checked, false);
            fake.commands = Object.assign({}, fake.commands, { status: root.status({ dnd: true }) });
            fake.change("status");
            tryCompare(row.toggle, "checked", true);
        }

        function test_dnd_switch() {
            const row = root.shown("Do not disturb");
            compare(row.toggle.checked, false);
            root.flip(row);
            tryCompare(row.toggle, "checked", true);
            compare(root.runs("dnd"), [["run", "dnd", ["on"]]]);
            // and back off
            fake.commands = Object.assign({}, fake.commands, { dnd: root.statusDoc({ dnd: false }) });
            root.flip(row);
            tryCompare(row.toggle, "checked", false);
            compare(root.runs("dnd")[1], ["run", "dnd", ["off"]]);
            verify(root.shown("Working…") === null, "nothing is left running");
        }

        function test_failed_write_restores() {
            fake.commands = Object.assign({}, fake.commands, { dnd: { code: 1, error: "exit code 1", output: { type: "result", ok: false, message: "the reason in the program's words" } } });
            const row = root.shown("Do not disturb");
            root.flip(row);
            tryVerify(() => root.shown("Couldn't change Do not disturb.") !== null);
            compare(row.toggle.checked, false, "the switch is back on the last value read");
            // the program's words are under Advanced details, not shown at first
            verify(root.shown("the reason in the program's words") === null);
            const details = root.shown("Advanced details");
            verify(details !== null);
            root.press(details);
            tryVerify(() => root.shown("the reason in the program's words") !== null);
            // the next change clears the message
            fake.commands = Object.assign({}, fake.commands, { dnd: root.statusDoc({ dnd: true }) });
            root.flip(row);
            tryVerify(() => root.shown("Couldn't change Do not disturb.") === null);
        }

        function test_failed_limit_restores() {
            fake.commands = Object.assign({}, fake.commands, { set: { code: 1, error: "exit code 1: error: refused" } });
            const box = root.named("limitBox");
            tryCompare(box, "currentIndex", 2, 5000, "200 is chosen");
            root.choose(box, 3);
            tryVerify(() => root.shown("Couldn't change how many notifications are kept.") !== null);
            compare(box.currentIndex, 2, "the box shows the last value read");
            compare(root.runs("set"), [["run", "set", ["history.limit", "500"]]]);
        }

        function test_set_limit_reads_settings_again() {
            const box = root.named("limitBox");
            tryCompare(box, "currentIndex", 2);
            compare(JSON.stringify(box.model), JSON.stringify(["50", "100", "200", "500", "1000"]));
            fake.commands = Object.assign({}, fake.commands, { settings: root.settings({ history: { limit: 500, persist: false } }) });
            root.choose(box, 3);
            tryVerify(() => root.runs("settings").length === 2);
            compare(root.runs("set"), [["run", "set", ["history.limit", "500"]]]);
            tryCompare(box, "currentIndex", 3);
        }

        function test_switches_send_the_planned_pairs() {
            const pairs = [
                ["When an app is full screen", ["autoDnd.fullscreen", "off"]],
                ["When you share your screen", ["autoDnd.screenShare", "off"]],
                ["Keep after restart", ["history.persist", "on"]],
                ["Let urgent notifications through Do not disturb", ["dnd.allowCritical", "off"]]
            ];
            for (let i = 0; i < pairs.length; ++i) {
                const row = root.shown(pairs[i][0]);
                verify(row !== null, pairs[i][0]);
                tryVerify(() => row.enabled);
                root.flip(row);
                tryVerify(() => root.runs("set").length === i + 1);
                compare(root.runs("set")[i], ["run", "set", pairs[i][1]]);
                tryVerify(() => root.shown("Working…") === null);
            }
        }

        function test_mute_and_unmute() {
            const personal = root.shown("Mute Personal");
            const business = root.shown("Mute Business");
            verify(personal !== null && business !== null);
            compare(personal.toggle.checked, false);
            compare(business.toggle.checked, true);
            root.flip(personal);
            tryVerify(() => root.runs("mute").length === 1);
            compare(root.runs("mute")[0], ["run", "mute", ["personal"]]);
            tryVerify(() => root.shown("Working…") === null);
            root.flip(root.shown("Mute Business"));
            tryVerify(() => root.runs("unmute").length === 1);
            compare(root.runs("unmute")[0], ["run", "unmute", ["business"]]);
        }

        function test_focus_modes() {
            const box = root.named("focusBox");
            tryCompare(box, "currentIndex", 2, 5000, "the active mode is chosen");
            compare(JSON.stringify(box.model), JSON.stringify(["Off", "reading", "Work"]));
            root.choose(box, 0);
            tryVerify(() => root.runs("focus").length === 1);
            compare(root.runs("focus")[0], ["run", "focus", ["off"]]);
            tryCompare(box, "currentIndex", 0);
            tryVerify(() => root.shown("Working…") === null);
            root.choose(box, 1);
            tryVerify(() => root.runs("focus").length === 2);
            compare(root.runs("focus")[1], ["run", "focus", ["reading"]]);
        }

        function test_clear_history_asks_first() {
            const button = root.shown("Clear history…");
            verify(button !== null);
            // Cancel: nothing changes and nothing is shown
            fake.confirms = false;
            root.press(button);
            tryVerify(() => root.runs("history-clear").length === 1);
            wait(50);
            verify(root.shown("Couldn't clear the history.") === null);
            verify(root.shown("Working…") === null);
            // Confirm: it runs, then the state is read again
            fake.confirms = true;
            fake.commands = Object.assign({}, fake.commands, { status: root.status({ history: 0 }) });
            root.press(button);
            tryVerify(() => root.shown("0 notifications are kept.") !== null);
            compare(root.runs("history-clear").length, 2);
        }

        function test_no_notification_text() {
            const texts = root.everyText();
            verify(texts.length > 10);
            for (const t of texts) {
                verify(t.indexOf("secret") < 0, "shows notification text: " + t);
                verify(t.indexOf("Chat") < 0, "shows an app's name: " + t);
            }
        }

        function test_untrusted_text_is_shown_as_it_is() {
            fake.commands = Object.assign({}, fake.commands, {
                settings: root.settings({ zones: [{ id: "business", label: "<b>x</b>" }, { id: "personal", label: "‮evil" }], rules: [{ name: "<i>y</i> https://example.org" }] })
            });
            fake.change("settings");
            tryVerify(() => root.shown("Mute <b>x</b>") !== null);
            verify(root.shown("Mute ‮evil") !== null);
            verify(root.shown("<i>y</i> https://example.org") !== null);
        }

        function test_not_running() {
            fake.calls = [];
            fake.commands = Object.assign({}, root.answers(), { status: { code: 0, output: { owner: null, notifd_owns: false, jobs_owner: null, daemon: null, config: null } } });
            root.open();
            tryVerify(() => root.shown("noti-fi isn't running. uQ's own notifications are in use.") !== null);
            verify(root.shown("Do not disturb") === null, "no controls while it isn't running");
            verify(root.shown("Clear history…") === null);
            const every = root.everyText();
            for (const t of every)
                verify(t.indexOf("Start") !== 0, "there is no start button: " + t);
            compare(root.runs("dnd").length, 0);
        }

        function test_a_failed_read_shows_the_error() {
            fake.commands = Object.assign({}, root.answers(), { status: { error: "timed out" } });
            root.open();
            tryVerify(() => root.shown("Couldn't read noti-fi's state") !== null);
            verify(root.shown("timed out") !== null);
            verify(root.shown("Do not disturb") === null);
        }

        function test_unreadable_settings_disable_their_rows() {
            fake.commands = Object.assign({}, root.answers(), { settings: { code: 1, error: "exit code 1: error: broken" } });
            root.open();
            tryVerify(() => root.shown("Couldn't read noti-fi's settings.") !== null);
            verify(root.shown("Do not disturb").enabled, "the daemon's own state still works");
            verify(!root.shown("Keep after restart").enabled);
            verify(!root.shown("When an app is full screen").enabled);
            verify(!root.named("limitBox").enabled);
        }

        function test_a_rules_file_error_is_shown() {
            fake.commands = Object.assign({}, fake.commands, { status: root.status({ cfgError: "rule 3: something is wrong" }) });
            fake.change("status");
            tryVerify(() => root.shown("rule 3: something is wrong") !== null);
            verify(root.shown("noti-fi couldn't use your latest rules, so it is using the earlier ones.") !== null);
        }

        function test_open_noti_fi() {
            const button = root.shown("Open noti-fi");
            verify(button !== null);
            root.press(button);
            compare(fake.calls[fake.calls.length - 1], ["openApp", "org.notify.Panel.desktop"]);
            verify(root.shown("Your rules") !== null);
            verify(root.shown("Change your rules in noti-fi's own app.") !== null);
        }

        function test_highlight_scrolls() {
            let flick = null;
            root.walk(loader.item, it => {
                if (!flick && it.contentY !== undefined && it.contentHeight !== undefined)
                    flick = it;
            });
            verify(flick !== null);
            compare(flick.contentY, 0);
            loader.item.highlight = "rules";
            tryVerify(() => flick.contentY > 0);
        }
    }
}
