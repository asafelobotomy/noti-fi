// What the page tests share: the Notifications page in a Loader, a fake ctx (FakeContext.qml) with an answer for every command
// (from fixtures/, which are what the real `notify` commands print), and helpers that find items by their text and act on
// them the way a person does: a switch flips (its `toggled` signal), a box is chosen (`activated`), a button is pressed
// (`clicked`), a field is typed in (`editingFinished`). The fake ctx logs every request in `fake.calls`.
// A test file is `PageHarness { id: root; TestCase { id: tc; ... } Component.onCompleted: root.tc = tc }`.
import QtQuick
import QtTest

Item {
    id: root
    width: 700
    height: 500

    /// The TestCase of the file that uses this harness (its verify and tryVerify).
    property var tc
    default property alias tests: holder.data
    /// The fake ctx and the Loader that holds the page, for the tests.
    readonly property alias fakeCtx: fake
    readonly property alias pageLoader: loader
    Item {
        id: holder
    }

    readonly property url pageUrl: Qt.resolvedUrl("../../uq/pages/noti-fi/Page.qml")

    function fixture(name) {
        const x = new XMLHttpRequest();
        x.open("GET", Qt.resolvedUrl("fixtures/" + name), false);
        x.send();
        return JSON.parse(x.responseText);
    }

    // `notify status --json` with noti-fi running. `latest.summary` is notification text: the page must never show it.
    function status(extra) {
        const base = fixture("status.json");
        base.daemon = Object.assign(base.daemon, { latest: { id: 17, t: 1, appName: "Chat", appIcon: "", summary: "secret", read: false } }, extra || {});
        return { code: 0, output: base };
    }

    // the answer of dnd, mute, unmute and focus: one bare Status
    function statusDoc(extra) {
        return { code: 0, output: status(extra).output.daemon };
    }

    function settings(extra) {
        return { code: 0, output: Object.assign(fixture("settings.json"), extra || {}) };
    }

    /// `notify config show --json`; `changes` maps a key to its new value (the file then sets it).
    function config(changes) {
        const doc = fixture("config.json");
        for (const row of doc.settings)
            if (changes && changes[row.key] !== undefined) {
                row.value = changes[row.key];
                row.set = true;
            }
        return { code: 0, output: doc };
    }

    function answers() {
        return {
            status: status(),
            settings: settings(),
            "focus-modes": { code: 0, output: fixture("focus-list.json") },
            config: config(),
            rules: { code: 0, output: fixture("rules.json") },
            history: { code: 0, output: fixture("history.json") },
            "history-held": { code: 0, output: fixture("history.json").filter(e => !e.shown) },
            doctor: { code: 1, error: "exit code 1", output: fixture("doctor.json") },
            "update-check": { code: 0, output: fixture("update-check.json") },
            dnd: statusDoc({ dnd: true }),
            focus: statusDoc({ focus: "" }),
            mute: statusDoc({ muted: ["business", "personal"] }),
            unmute: statusDoc({ muted: [] }),
            "config-set": { code: 0, output: "done\n" },
            "config-reset": { code: 0, output: "done\n" },
            test: { code: 0, output: "" },
            "history-clear": { code: 0, output: "history cleared\n" },
            update: { code: 0, output: null }
        };
    }

    FakeContext {
        id: fake
        commands: root.answers()
        confirmed: ["history-clear", "update"]
        events: ({
            update: [
                { type: "plan", name: "upgrade", total: 3, steps: [] },
                { type: "step", n: 1, total: 3, desc: "download notify-0.20.0.tar.gz and check its checksum", status: "running" },
                { type: "step", n: 1, total: 3, desc: "download notify-0.20.0.tar.gz and check its checksum", status: "done" },
                { type: "step", n: 2, total: 3, desc: "replace notify", status: "running" },
                { type: "result", ok: true, message: "updated to noti-fi 0.20.0. Restart the panel (noti-fi) to use it." }
            ]
        })
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

    /// Like shown(), but only an item's own text (a Notice also holds its detail as a property).
    function shownText(text) {
        let hit = null;
        walk(loader.item, it => {
            if (!hit && it.visible && it.text === text)
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

    function tab(title) {
        const button = root.named("tab-" + title);
        root.tc.verify(button !== null, "a tab called " + title);
        button.clicked();
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

    function type(field, text) {
        field.text = text;
        field.editingFinished();
    }

    /// The Switch or field inside the row labelled `label` on the Preferences tab.
    function prefRow(label) {
        let hit = null;
        walk(loader.item, it => {
            if (!hit && it.visible && it.label === label && it.children.length > 0)
                hit = it;
        });
        return hit;
    }

    function inside(item, test) {
        let hit = null;
        walk(item, it => {
            if (!hit && it.visible && test(it))
                hit = it;
        });
        return hit;
    }

    function runs(id) {
        return fake.calls.filter(c => c[0] === "run" && c[1] === id);
    }

    function open() {
        loader.source = "";
        loader.source = root.pageUrl;
        root.tc.verify(loader.status === Loader.Ready, "the page loads");
        loader.item.ctx = fake.ctx;
    }

    /// Start every test from a freshly opened page with all of its first reads answered.
    function reset() {
        fake.calls = [];
        fake.watches = ({});
        fake.confirms = true;
        fake.commands = root.answers();
        root.open();
        root.tc.tryVerify(() => root.shown("Do not disturb") !== null);
        const p = loader.item;
        root.tc.tryVerify(() => p.ready && p.settings.ok && p.focusModes.ok && p.prefs.ok && p.rulesCheck.ok);
        root.tc.tryVerify(() => root.runs("history").length === 1);
    }
}
