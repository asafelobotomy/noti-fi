// Node tests for the Notifications page: logic.js, page.toml and what Page.qml may send.
//   node --test 'tests/uq/*.test.mjs'
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { loadLogic } from "./load-logic.mjs";
import { parseToml } from "./toml-lite.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const pageDir = join(here, "../../uq/pages/noti-fi");
const logic = loadLogic(join(pageDir, "logic.js"));
const manifest = parseToml(readFileSync(join(pageDir, "page.toml"), "utf8"));
const pageQml = readFileSync(join(pageDir, "Page.qml"), "utf8");
const allQml = ["Page.qml", "Notice.qml", "RulesSection.qml"].map((f) => readFileSync(join(pageDir, f), "utf8")).join("\n");
const fixture = (name) => JSON.parse(readFileSync(join(here, "fixtures", name), "utf8"));
// logic.js runs in its own context; its arrays and objects have another prototype, so compare plain copies
const plain = (v) => JSON.parse(JSON.stringify(v));
const ok = (output) => ({ code: 0, output });
const command = (id) => manifest.command.find((c) => c.id === id);

test("parses_status: the state is under `daemon`, which is null while noti-fi isn't running", () => {
  const s = plain(logic.parseStatus(ok(fixture("status.json"))));
  assert.equal(s.kind, "ready");
  assert.equal(s.dnd, false);
  assert.deepEqual(s.muted, ["business"]);
  assert.deepEqual(s.zones, ["business", "personal", "system"]);
  assert.equal(s.focus, "work");
  assert.equal(s.history, 42);
  assert.equal(s.cfgError, "");

  const stopped = plain(logic.parseStatus(ok(fixture("status-not-running.json"))));
  assert.equal(stopped.kind, "notRunning");
  assert.equal(stopped.message, "noti-fi isn't running. uQ's own notifications are in use.");

  for (const bad of [null, { error: "timed out" }, ok(null), ok("text"), ok([]), ok({}), ok({ daemon: 3 }), ok({ daemon: [] })]) {
    assert.equal(logic.parseStatus(bad).kind, "error", JSON.stringify(bad));
  }
  assert.equal(logic.parseStatus({ error: "exit code 1: error: boom" }).message, "boom");

  // a document the changers answer with (dnd, mute, unmute, focus) is a bare Status
  const doc = plain(logic.parseStatusDocument(ok(fixture("status.json").daemon)));
  assert.equal(doc.focus, "work");
  assert.equal(logic.parseStatusDocument(ok(fixture("status.json"))), null, "the wrapped report is not a Status");
  assert.equal(logic.parseStatusDocument({ error: "x" }), null);
});

test("parses_settings", () => {
  const s = plain(logic.parseSettings(ok(fixture("settings.json"))));
  assert.equal(s.ok, true);
  assert.equal(s.limit, 200);
  assert.equal(s.persist, false);
  assert.equal(s.allowCritical, true);
  assert.equal(s.fullscreen, true);
  assert.equal(s.screenShare, true);
  assert.deepEqual(s.rules, ["Quiet chat", "Rule 2"]);
  assert.deepEqual(s.zones[1], { id: "work", label: "work" });
  for (const bad of [null, { error: "no" }, ok(null), ok({}), ok({ dnd: {}, history: {}, autoDnd: {} }), ok({ ...fixture("settings.json"), history: { limit: "200", persist: false } })]) {
    const r = logic.parseSettings(bad);
    assert.equal(r.ok, false, JSON.stringify(bad));
    assert.equal(r.message, "Couldn't read noti-fi's settings.");
  }
  assert.equal(logic.parseSettings({ error: "exit code 1: error: not running" }).detail, "not running");
});

test("focus_modes_listed: Off, then each mode by its label (or its name), and only modes the CLI can be given", () => {
  const m = plain(logic.parseFocusModes(ok(fixture("focus-list.json"))));
  assert.equal(m.ok, true);
  assert.equal(m.active, "work");
  assert.deepEqual(m.modes, [
    { id: "reading", label: "reading" },
    { id: "work", label: "Work" }
  ]);
  assert.deepEqual(plain(logic.focusTexts(m.modes)), ["Off", "reading", "Work"]);
  assert.equal(logic.focusIndex(m.modes, "work"), 2);
  assert.equal(logic.focusIndex(m.modes, ""), 0);
  assert.equal(logic.focusIndex(m.modes, "gone"), 0);
  assert.equal(logic.focusArg(m.modes, 0), "off");
  assert.equal(logic.focusArg(m.modes, 1), "reading");
  assert.equal(logic.focusArg(m.modes, 2), "work");
  assert.equal(logic.focusArg(m.modes, 3), "off", "an index out of range turns it off, never names a mode");

  const odd = logic.parseFocusModes(ok({ active: "", modes: { "-x": {}, "Two Words": {}, off: {}, list: {}, Work: {}, fine_1: { label: "Fine" }, "": {} } }));
  assert.deepEqual(plain(odd.modes), [{ id: "Work", label: "Work" }, { id: "fine_1", label: "Fine" }]);
  assert.equal(logic.overlapped({ error: "already running" }), true);
  assert.equal(logic.overlapped({ error: "timed out" }), false);
  assert.equal(logic.parseFocusModes({ error: "x" }).ok, false);
  assert.deepEqual(plain(logic.focusTexts(logic.parseFocusModes({ error: "x" }).modes)), ["Off"]);
});

test("set_values_partition: each key takes only its own values, and only those pairs are ever sent", () => {
  const key = new RegExp(command("set").args[0].pattern);
  const value = new RegExp(command("set").args[1].pattern);
  const table = plain(logic.SET_VALUES);
  const keys = Object.keys(table);
  assert.deepEqual(keys.sort(), ["autoDnd.fullscreen", "autoDnd.screenShare", "dnd.allowCritical", "history.limit", "history.persist"]);
  assert.deepEqual(table["history.limit"], ["50", "100", "200", "500", "1000"]);
  for (const k of keys.filter((k) => k !== "history.limit")) assert.deepEqual(table[k], ["on", "off"]);

  const everyValue = new Set(keys.flatMap((k) => table[k]));
  const everyWord = ["on", "off", "50", "100", "200", "500", "1000", "7", "true", "yes", "", "-1", "500\n", "on off"];
  for (const k of keys) {
    assert.ok(key.test(k), k);
    for (const v of everyWord) {
      const sent = logic.setArgs(k, v);
      if (table[k].includes(v)) {
        assert.deepEqual(plain(sent), [k, v]);
        assert.ok(value.test(v), `${k} ${v} passes the manifest`);
      } else assert.equal(sent, null, `${k} ${JSON.stringify(v)} is never sent`);
    }
  }
  // the manifest's value pattern allows nothing that no key takes
  for (const v of everyWord) assert.equal(value.test(v), everyValue.has(v), JSON.stringify(v));
  for (const k of ["history", "history.limit ", "theme.card", "autoDnd.share", "__proto__", "toString", ""]) assert.equal(logic.setArgs(k, "on"), null, k);

  // the limit box sends exactly the panel's choices, plus a limit set some other way when it is the current one
  assert.deepEqual(plain(logic.limitTexts(200)), ["50", "100", "200", "500", "1000"]);
  assert.deepEqual(plain(logic.limitTexts(300)), ["50", "100", "200", "300", "500", "1000"]);
  assert.equal(logic.limitIndex(500), 3);
  assert.equal(logic.limitIndex(300), 3);
  assert.deepEqual(plain(logic.limitArgs(200, 3)), ["history.limit", "500"]);
  assert.equal(logic.limitArgs(300, 3), null, "the current odd limit is not sent back: notify set doesn't take it");
  assert.equal(logic.limitArgs(200, 9), null);
  assert.equal(logic.limitArgs(200, -1), null);

  // Page.qml reaches `set` only through setSetting, and builds its arguments only with Logic.setArgs / limitArgs
  assert.equal([...pageQml.matchAll(/"set"/g)].length, 1);
  assert.equal([...pageQml.matchAll(/setSetting\(/g)].length, 5 + 1, "four switches and the box, plus the definition");
  for (const m of pageQml.matchAll(/setSetting\((.*)\)\s*$/gm)) assert.match(m[1], /Logic\.(setArgs|limitArgs)\(/);
});

test("patterns_match_manifest: logic.js and page.toml carry the same argument patterns and confirms", () => {
  const fromLogic = plain(logic.PATTERNS);
  const fromManifest = {};
  for (const c of manifest.command) if (c.args) fromManifest[c.id] = Object.fromEntries(c.args.map((a) => [a.name, a.pattern]));
  assert.deepEqual(fromLogic, fromManifest);
  for (const c of manifest.command) for (const a of c.args ?? []) assert.ok(a.pattern.startsWith("^") && a.pattern.endsWith("$"), `${c.id}.${a.name} is anchored`);
  const confirms = Object.fromEntries(manifest.command.filter((c) => c.confirm).map((c) => [c.id, c.confirm]));
  assert.deepEqual(plain(logic.CONFIRMS), confirms);
});

test("rejects_hostile_args: no pattern takes an option, a newline, an empty or over-long value, or a path", () => {
  const hostile = ["-rf", "--force", "-", "a\nb", "a\n", "\nx", "", "x".repeat(5000), "x".repeat(41), "../x", "a/b", "a b", " a", "a ", "$(id)", "`id`", "a;b", "a\0b"];
  for (const c of manifest.command) {
    for (const a of c.args ?? []) {
      const re = new RegExp(a.pattern);
      for (const h of hostile) assert.equal(re.test(h), false, `${c.id}.${a.name} accepts ${JSON.stringify(h.slice(0, 20))}`);
      for (const h of ["+x", "-0"]) assert.equal(re.test(h), false, `${c.id}.${a.name} accepts ${h}`);
    }
  }
  // the same in logic.js: a name it can't hand to `notify` is never sent
  const rows = plain(logic.groupRows({ zones: ["ok_1", "-rf", "a b", "../x", "x".repeat(41), "z".repeat(40)], muted: [] }, null));
  assert.deepEqual(rows.map((r) => r.changeable), [true, false, false, false, false, true]);
  const modes = plain(logic.parseFocusModes(ok({ active: "", modes: { "-rf": {}, "a\nb": {}, ["x".repeat(41)]: {}, ok: {} } })).modes);
  assert.deepEqual(modes.map((m) => m.id), ["ok"]);
  assert.equal(logic.setArgs("history.limit", "--force"), null);
  assert.equal(logic.setArgs("--force", "on"), null);
  assert.deepEqual(plain(logic.dndArgs(true)), ["on"]);
  assert.deepEqual(plain(logic.dndArgs(false)), ["off"]);
  assert.equal(logic.muteCommand(true), "mute");
  assert.equal(logic.muteCommand(false), "unmute");
});

test("confirms_match_plan: only clearing the history asks, with the planned words", () => {
  const withConfirm = manifest.command.filter((c) => c.confirm !== undefined);
  assert.deepEqual(withConfirm.map((c) => [c.id, c.confirm]), [["history-clear", "Clear the notification history? This can't be undone."]]);
  assert.deepEqual(command("history-clear").argv, ["/usr/bin/notify", "history", "--clear"]);
  assert.equal(command("history-clear").args, undefined);
  // nothing else erases, and no command is destructive without being listed here
  assert.deepEqual(manifest.command.map((c) => c.id).sort(), ["dnd", "focus", "focus-modes", "history-clear", "mute", "set", "settings", "status", "unmute"]);
  for (const c of manifest.command) {
    assert.ok(c.argv[0] === "/usr/bin/notify", c.id);
    if (c.id !== "history-clear") assert.ok(!c.argv.includes("--clear") && !c.argv.includes("history"), c.id);
  }
  // Page.qml reaches it only from its button
  assert.equal([...pageQml.matchAll(/"history-clear"/g)].length, 1);
});

test("commands_and_watches: what Page.qml runs exists, the reads follow the files, and the changers answer in JSON", () => {
  const ids = new Set(manifest.command.map((c) => c.id));
  const used = new Set();
  for (const m of pageQml.matchAll(/(?:command\.(?:run|watch)|change)\(\s*"([a-z-]+)"/g)) used.add(m[1]);
  used.add("mute").add("unmute"); // chosen by Logic.muteCommand
  assert.deepEqual([...used].sort(), [...ids].sort(), "every command is used and every use is declared");

  const files = (id) => plain(command(id).watch?.files);
  assert.deepEqual(files("status"), ["~/.local/state/notifd/state.json", "~/.config/notifd/rules.json"]);
  assert.deepEqual(files("settings"), ["~/.config/notifd/rules.json"]);
  assert.deepEqual(files("focus-modes"), ["~/.config/notifd/rules.json"]);
  for (const c of manifest.command.filter((c) => c.watch)) assert.ok(!c.args && !c.confirm, `${c.id}: only a plain read can be watched`);

  assert.deepEqual(command("status").argv, ["/usr/bin/notify", "status", "--json"]);
  assert.deepEqual(command("focus-modes").argv, ["/usr/bin/notify", "focus", "list", "--json"]);
  for (const id of ["dnd", "focus", "mute", "unmute"]) {
    assert.equal(command(id).output, "json", id);
    assert.ok(command(id).argv.includes("--json"), id);
  }
  assert.equal(command("set").output, "text");
  assert.deepEqual(plain(manifest.requires), { bins: ["notify"] });
  assert.equal(manifest.id, "noti-fi");
  assert.equal(manifest.name, "Notifications");
  assert.equal(manifest.icon, "preferences-desktop-notification");
  assert.equal(manifest.dbus, undefined, "no D-Bus access is needed");
});

test("settings_rows: every row has keywords and a row in the page", () => {
  assert.deepEqual(manifest.setting.map((s) => s.id), ["dnd", "focus", "groups", "auto-dnd", "history", "rules"]);
  for (const s of manifest.setting) {
    assert.ok(s.keywords?.length > 0, `${s.id} has keywords`);
    assert.ok(allQml.includes(`settingId: "${s.id}"`), `Page.qml has a row for ${s.id}`);
  }
  for (const w of ["notifications", "noti-fi", "do not disturb", "dnd", "focus", "mute", "history"]) assert.ok(manifest.keywords.includes(w), w);
});

test("groups_listed: a group's own name, whether it is muted, in the order the daemon gives", () => {
  const status = plain(logic.parseStatus(ok(fixture("status.json"))));
  const settings = logic.parseSettings(ok({ ...fixture("settings.json"), zones: [{ id: "business", label: "Business" }, { id: "personal", label: "<b>Me</b>" }] }));
  const rows = plain(logic.groupRows(status, settings));
  assert.deepEqual(rows, [
    { id: "business", label: "Business", muted: true, changeable: true },
    { id: "personal", label: "<b>Me</b>", muted: false, changeable: true },
    { id: "system", label: "system", muted: false, changeable: true }
  ]);
  assert.equal(logic.groupRows(status, logic.parseSettings({ error: "x" }))[0].label, "business", "without the settings, the id is the name");
  assert.deepEqual(logic.groupRows(null, null), []);
});

test("untrusted_values: wrong types are errors, long lists and strings are cut", () => {
  const many = Array.from({ length: 900 }, (_, i) => `z${i}`);
  const s = plain(logic.parseStatus(ok({ daemon: { dnd: "yes", muted: many, zones: many, focus: "f".repeat(500), history: -5, cfgError: 7 } })));
  assert.equal(s.dnd, false);
  assert.equal(s.muted.length, 500);
  assert.equal(s.zones.length, 500);
  assert.equal(s.focus.length, 200);
  assert.equal(s.history, 0);
  assert.equal(s.cfgError, "");
  const set = plain(logic.parseSettings(ok({ ...fixture("settings.json"), rules: many.map((n) => ({ name: n + "x".repeat(300) })).concat([{ name: 3 }, null, "x"]), zones: many.map((id) => ({ id, label: 5 })).concat([{ id: "" }, 7]) })));
  assert.equal(set.rules.length, 500);
  assert.equal(set.rules[0].length, 200);
  assert.equal(set.zones.length, 500);
  assert.equal(set.zones[3].label, "z3", "a label that isn't text falls back to the id");
  assert.equal(logic.historyLine(1), "1 notification is kept.");
  assert.equal(logic.historyLine(0), "0 notifications are kept.");
});

test("no_notification_text: the page never reads a summary, a body or an app's name from the state", () => {
  const status = logic.parseStatus(ok(fixture("status.json")));
  assert.ok(!JSON.stringify(plain(status)).includes("Quarterly"));
  assert.ok(!JSON.stringify(plain(logic.groupRows(status, null))).includes("Quarterly"));
  // (Notice.qml has a `summary` of its own: its own sentence, not a notification's)
  const code = readFileSync(join(pageDir, "logic.js"), "utf8") + pageQml + readFileSync(join(pageDir, "RulesSection.qml"), "utf8");
  for (const word of ["summary", "latest", "body", "appName", "sharingBy", "inhibitors"]) {
    assert.ok(!new RegExp(`\\.${word}\\b|"${word}"`).test(code.replace(/\/\/.*$/gm, "")), `${word} is never read`);
  }
});

test("problems: a failed change is a plain sentence, the program's words are kept for Advanced details", () => {
  assert.equal(logic.problemOf(ok(null), "Couldn't x"), "");
  assert.equal(logic.problemOf({ code: 0, output: "fine" }, "Couldn't x"), "");
  assert.equal(logic.problemOf({ error: "Cancelled", cancelled: true }, "Couldn't x"), "", "Cancel is not a failure");
  assert.equal(logic.cancelled({ error: "Cancelled", cancelled: true }), true);
  assert.equal(logic.problemOf({ code: 1, error: "exit code 1: error: notifd refused this change (nope); nothing was changed" }, "Couldn't change this"), "Couldn't change this.");
  assert.equal(logic.detailOf({ code: 1, error: "exit code 1: error: notifd refused this change (nope); nothing was changed" }), "notifd refused this change (nope); nothing was changed");
  assert.equal(logic.detailOf({ code: 1, error: "exit code 1", output: { type: "result", ok: false, message: "unknown zone 'x'" } }), "unknown zone 'x'");
  assert.equal(logic.problemOf({ error: "timed out" }, "Couldn't x"), "Couldn't x.");
  assert.equal(logic.problemOf(null, "Couldn't x"), "Couldn't x.");
  assert.equal(logic.detailOf({ error: "e".repeat(900) }).length, 200);
});

test("page_strings_are_plain: no jargon in anything the page or the manifest shows", () => {
  const jargon = [/D-Bus/i, /\bdbus\b/i, /polkit/i, /\bJSON\b/i, /systemctl/i, /sched_ext/i, /EEVDF/i, /zram/i, /sysctl/i, /notifd/i, /quickshell/i, /daemon/i, /socket/i, /rules\.json/i, /\bargv\b/i, /\bcli\b/i];
  const literals = (src) => {
    const out = [];
    for (let i = 0; i < src.length; i++) {
      if (src.startsWith("//", i)) while (i < src.length && src[i] !== "\n") i++;
      else if (src.startsWith("/*", i)) i = src.indexOf("*/", i) + 1;
      else if (src[i] === '"') {
        let s = "";
        for (i++; src[i] !== '"'; i++) {
          if (src[i] === "\\") i++;
          s += src[i];
        }
        out.push(s);
      }
    }
    return out;
  };
  // property names in the code ("daemon" in the status report) are not shown; text has a capital or a space
  const texty = (list) => list.filter((t) => /[A-Z\s]/.test(t));
  const shown = [
    ...texty(literals(pageQml)),
    ...texty(literals(readFileSync(join(pageDir, "Notice.qml"), "utf8"))),
    ...texty(literals(readFileSync(join(pageDir, "RulesSection.qml"), "utf8"))),
    ...texty(literals(readFileSync(join(pageDir, "logic.js"), "utf8"))),
    manifest.name,
    ...manifest.setting.map((s) => s.label),
    ...manifest.command.filter((c) => c.confirm).map((c) => c.confirm)
  ];
  assert.ok(shown.length > 25, "the extractor found the strings");
  for (const text of shown) for (const re of jargon) assert.ok(!re.test(text), `"${text}" shows ${re}`);
  // sentence case: no title-cased labels, no trailing full stop on a one-word label
  assert.ok(shown.includes("Do not disturb") && shown.includes("Clear history…") && shown.includes("Open noti-fi"));
  assert.ok(shown.includes("noti-fi isn't running. uQ's own notifications are in use."));
});

test("app_id: Open noti-fi names noti-fi's own launcher entry", () => {
  assert.equal(logic.APP_ID, "org.notify.Panel.desktop");
  assert.match(logic.APP_ID, /^[A-Za-z0-9_][A-Za-z0-9._-]*\.desktop$/);
  const desktop = readFileSync(join(here, "../../packaging/org.notify.Panel.desktop"), "utf8");
  assert.match(desktop, /^Exec=notify-panel/m);
});
