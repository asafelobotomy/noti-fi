// Node tests for the Notifications page: logic.js, page.toml and what Page.qml may send.
//   node --test 'tests/uq/*.test.mjs'
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { loadLogic } from "./load-logic.mjs";
import { parseToml } from "./toml-lite.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const pageDir = join(here, "../../uq/pages/noti-fi");
const logic = loadLogic(join(pageDir, "logic.js"));
const manifest = parseToml(readFileSync(join(pageDir, "page.toml"), "utf8"));
const qmlFiles = readdirSync(pageDir).filter((f) => f.endsWith(".qml")).sort();
const qmlOf = (f) => readFileSync(join(pageDir, f), "utf8");
const pageQml = qmlFiles.map(qmlOf).join("\n"); // every file of the page: the commands and strings can be in any of them
const allQml = pageQml;
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
  assert.equal(s.unread, 3);
  assert.equal(s.active, 0);
  assert.equal(s.holding, "");

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

test("patterns_match_manifest: logic.js and page.toml carry the same argument patterns and confirms", () => {
  const fromLogic = plain(logic.PATTERNS);
  const fromManifest = {};
  for (const c of manifest.command) if (c.args) fromManifest[c.id] = Object.fromEntries(c.args.map((a) => [a.name, a.pattern]));
  assert.deepEqual(fromLogic, fromManifest);
  for (const c of manifest.command) for (const a of c.args ?? []) assert.ok(a.pattern.startsWith("^") && a.pattern.endsWith("$"), `${c.id}.${a.name} is anchored`);
  const confirms = Object.fromEntries(manifest.command.filter((c) => c.confirm).map((c) => [c.id, c.confirm]));
  assert.deepEqual(plain(logic.CONFIRMS), confirms);
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

test("app_id: Open noti-fi names noti-fi's own launcher entry", () => {
  assert.equal(logic.APP_ID, "org.notify.Panel.desktop");
  assert.match(logic.APP_ID, /^[A-Za-z0-9_][A-Za-z0-9._-]*\.desktop$/);
  const desktop = readFileSync(join(here, "../../packaging/org.notify.Panel.desktop"), "utf8");
  assert.match(desktop, /^Exec=notify-panel/m);
});

// ---------------------------------------------------------------- what the page may send

const SEND_HOSTILE = ["-rf", "--force", "-", "-x", "a\nb", "a\n", "\nx", "", "x".repeat(5000), "../x", "a/b", " a", "a ", "$(id)", "`id`", "a;b", "a\0b", "a|b", "a&b", "a'b", 'a"b', "a\\b"];

test("config_keys: the page, the manifest and the CLI agree on every setting", () => {
  const keys = plain(logic.PREF_KEYS);
  const cli = fixture("config.json").settings.map((s) => s.key);
  assert.deepEqual(keys, cli, "PREF_KEYS is `notify config show` in its order (tests/uq/fixtures/config.json is checked against the real command)");
  const pattern = new RegExp(command("config-set").args[0].pattern);
  for (const k of keys) assert.ok(pattern.test(k), k);
  for (const k of ["", "zones", "rules", "history", "theme", "theme.", "constructor", "__proto__", "width ", "Width", "width\n"]) assert.ok(!pattern.test(k), JSON.stringify(k));
  assert.equal(command("config-reset").args[0].pattern, command("config-set").args[0].pattern);

  // every setting has a row somewhere: a card of the Preferences tab, or a switch or box on the other tabs
  const inCards = plain(logic.PREF_CARDS).flatMap((c) => c.rows.map((r) => r.key));
  const elsewhere = ["dnd.allowCritical", "history.limit", "history.persist", "autoDnd.fullscreen", "autoDnd.screenShare"];
  assert.deepEqual([...inCards, ...elsewhere].sort(), [...keys].sort());
  for (const k of elsewhere) assert.ok(pageQml.includes(`"${k}"`), `${k} has its switch or box`);
  for (const c of plain(logic.PREF_CARDS)) for (const r of c.rows) assert.ok(r.label && !/[A-Z]{2}/.test(r.label.replace("uQ", "")), `${r.key} has a plain label`);
});

test("config_is_parsed_per_type: a key with the wrong type or an unknown key is dropped", () => {
  const c = plain(logic.parseConfig(ok(fixture("config.json"))));
  assert.equal(c.ok, true);
  assert.equal(Object.keys(c.byKey).length, 29);
  assert.deepEqual(c.byKey.maxVisible, { key: "maxVisible", type: "number", value: 7, set: true, default: 5, choices: [] });
  assert.equal(c.byKey["center.anchor"].choices.includes("top-left"), true);
  assert.equal(c.byKey["history.persist"].value, false);
  const odd = plain(logic.parseConfig(ok({ settings: [{ key: "width", type: "number", value: "wide" }, { key: "zones", type: "text", value: "x" }, { key: "sound.enabled", type: "bool", value: true, set: "yes" }, null, 4, { key: "screen", type: "weird", value: "" }] })));
  assert.deepEqual(Object.keys(odd.byKey), ["sound.enabled"]);
  assert.equal(odd.byKey["sound.enabled"].set, false, "`set` must be true, not just truthy");
  for (const bad of [null, { error: "x" }, ok(null), ok({}), ok({ settings: "no" })]) assert.equal(logic.parseConfig(bad).ok, false, JSON.stringify(bad));
  assert.equal(logic.parseConfig({ code: 1, error: "exit code 1: error: rules.json is not usable" }).detail, "rules.json is not usable");
});

test("pref_values: only what the CLI takes is ever sent, in the words it takes", () => {
  const c = logic.parseConfig(ok(fixture("config.json"))).byKey;
  const send = (key, text) => plain(logic.prefArgs(c[key], text));
  assert.deepEqual(send("width", "420"), { args: ["width", "420"] });
  assert.deepEqual(send("theme.opacity", " 0.5 "), { args: ["theme.opacity", "0.5"] });
  assert.deepEqual(send("theme.card", "#222233"), { args: ["theme.card", "#222233"] });
  assert.deepEqual(send("sound.enabled", "off"), { args: ["sound.enabled", "off"] });
  assert.deepEqual(send("theme.timerStyle", "line,circle"), { args: ["theme.timerStyle", "line,circle"] });
  assert.deepEqual(send("theme.fontFamily", "Noto Sans"), { args: ["theme.fontFamily", "Noto Sans"] });
  assert.deepEqual(send("center.anchor", "top-left"), { args: ["center.anchor", "top-left"] });
  assert.equal(send("width", "wide").error, "Use a number, like 380.");
  assert.equal(send("width", "-1").error, "Use a number, like 380.");
  assert.equal(send("width", "1e3").error, "Use a number, like 380.");
  assert.equal(send("width", "").error, "Type a value, or use Reset.");
  assert.equal(send("theme.card", "red").error, "Use a colour like #1d1f27.");
  assert.equal(send("sound.enabled", "true").error, "Use on or off.");
  assert.match(send("center.anchor", "nowhere").error, /^Use one of: top-left, /);
  assert.match(send("theme.timerStyle", "line,spin").error, /^Use one of: none, line/);
  assert.equal(send("theme.fontFamily", "$(id)").error, "That has a character noti-fi doesn't take here.");
  assert.equal(plain(logic.prefArgs(undefined, "1")).error, "That setting isn't available.");
  assert.deepEqual(plain(logic.prefResetArgs(c.width)), ["width"]);
  assert.equal(logic.prefResetArgs({ key: "zones" }), null);
  assert.equal(logic.prefResetArgs(null), null);
  assert.equal(logic.prefText(c["history.persist"]), "off");
  assert.equal(logic.prefDefaultText(c.width), "380");

  // whatever the page sends passes the manifest's patterns, and hostile text never does
  const value = new RegExp(command("config-set").args[1].pattern);
  for (const key of Object.keys(c))
    for (const text of ["on", "off", "1", "0.5", "#aabbcc", "line,frame", "Noto Sans", "auto", "top-left", "heatmap", "x".repeat(100)]) {
      const r = plain(logic.prefArgs(c[key], text));
      if (r.args) assert.ok(value.test(r.args[1]), `${key} ${text}`);
    }
  for (const h of SEND_HOSTILE) {
    assert.ok(!value.test(h), `the manifest takes ${JSON.stringify(h.slice(0, 20))}`);
    // (leading and trailing white space is trimmed off what is typed, so "a\n" is sent as "a")
    if (!value.test(h.trim()))
      for (const key of ["width", "theme.fontFamily", "screen", "theme.tokens"]) assert.equal(plain(logic.prefArgs(c[key], h)).args, undefined, `${key} ${JSON.stringify(h.slice(0, 20))}`);
  }
  assert.ok(value.test("Noto Sans") && value.test("a-b") && value.test("x".repeat(100)) && !value.test("x".repeat(101)), "inner spaces and hyphens, 100 characters");
  assert.ok(!value.test("-1") && !value.test("a b "), "never a leading hyphen or a trailing space");

  // the limit box sends exactly the panel's choices, plus a limit set some other way when it is the current one
  assert.deepEqual(plain(logic.limitTexts(200)), ["50", "100", "200", "500", "1000"]);
  assert.deepEqual(plain(logic.limitTexts(300)), ["50", "100", "200", "300", "500", "1000"]);
  assert.equal(logic.limitIndex(500), 3);
  assert.equal(logic.limitValue(200, 3), "500");
  assert.equal(logic.limitValue(300, 3), null, "the current odd limit is shown, never sent back");
  assert.equal(logic.limitValue(200, 9), null);
  assert.equal(logic.limitValue(200, -1), null);
  assert.equal(logic.boolWord(true), "on");
  assert.equal(logic.boolWord(false), "off");
});

test("patterns_match_manifest_args: every argument pattern is anchored and hostile text matches none", () => {
  for (const c of manifest.command) {
    for (const a of c.args ?? []) {
      assert.ok(a.pattern.startsWith("^") && a.pattern.endsWith("$"), `${c.id}.${a.name} is anchored`);
      const re = new RegExp(a.pattern);
      for (const h of SEND_HOSTILE.filter((h) => !(c.id === "config-set" && a.name === "value" && h === "a b"))) assert.equal(re.test(h), false, `${c.id}.${a.name} accepts ${JSON.stringify(h.slice(0, 20))}`);
      for (const h of ["+x", "-0", "x".repeat(41)]) if (c.id !== "config-set") assert.equal(re.test(h), false, `${c.id}.${a.name} accepts ${h}`);
    }
  }
  // names the page hands to `notify` come only through patterns it also checks itself
  const rows = plain(logic.groupRows({ zones: ["ok_1", "-rf", "a b", "../x", "x".repeat(41), "z".repeat(40)], muted: [] }, null));
  assert.deepEqual(rows.map((r) => r.changeable), [true, false, false, false, false, true]);
  const modes = plain(logic.parseFocusModes(ok({ active: "", modes: { "-rf": {}, "a\nb": {}, ["x".repeat(41)]: {}, ok: {} } })).modes);
  assert.deepEqual(modes.map((m) => m.id), ["ok"]);
  assert.deepEqual(plain(logic.dndArgs(true)), ["on"]);
  assert.deepEqual(plain(logic.dndArgs(false)), ["off"]);
  assert.equal(logic.muteCommand(true), "mute");
  assert.equal(logic.muteCommand(false), "unmute");
});

test("confirms_and_previews: the two things that can't be undone ask, after showing what they would do", () => {
  const withConfirm = manifest.command.filter((c) => c.confirm !== undefined);
  assert.deepEqual(withConfirm.map((c) => [c.id, c.confirm]), [
    ["history-clear", "Clear the notification history? This can't be undone."],
    ["update", "Update noti-fi? Notifications pause for about a second."]
  ]);
  assert.deepEqual(command("history-clear").argv, ["/usr/bin/notify", "history", "--clear"]);
  assert.equal(command("history-clear").args, undefined);
  assert.equal(command("history-clear").preview, "history-clear-plan");
  assert.equal(command("update").preview, "update-plan");
  // a preview is a plain read of the same page, with the same arguments, printing one plan document
  for (const c of withConfirm) {
    const plan = command(c.preview);
    assert.ok(plan, `${c.id} has a preview command`);
    assert.equal(plan.output, "json");
    assert.ok(!plan.confirm && !plan.preview && !plan.args && !c.args, `${c.id}: no arguments, so the preview takes none`);
    assert.ok(plan.argv.includes("--json"), c.preview);
  }
  assert.deepEqual(command("history-clear-plan").argv, ["/usr/bin/notify", "--json", "history", "--clear", "--dry-run"]);
  assert.deepEqual(command("update-plan").argv, ["/usr/bin/notify", "--json", "upgrade", "--plan"]);
  assert.deepEqual(command("update").argv, ["/usr/bin/notify", "--json", "upgrade", "--apply"]);
  assert.equal(command("update").output, "ndjson", "progress is streamed");
  assert.deepEqual(plain(logic.CONFIRMS), Object.fromEntries(withConfirm.map((c) => [c.id, c.confirm])));
  // nothing else erases or installs, and every other command is a read or a change that can be put back
  for (const c of manifest.command) {
    assert.ok(c.argv[0] === "/usr/bin/notify", c.id);
    if (!["history-clear", "history-clear-plan"].includes(c.id)) assert.ok(!c.argv.includes("--clear"), c.id);
    if (!["update", "update-plan"].includes(c.id)) assert.ok(!c.argv.includes("--apply") && !c.argv.includes("--plan"), c.id);
    for (const never of ["--force", "takeover", "restore", "portal", "adapt", "hotkey", "autodnd", "tray", "update"]) assert.ok(!c.argv.includes(never) || c.id === "update", `${c.id} never runs ${never}`);
  }
  // Page.qml and its tabs reach each from one button only
  assert.equal([...pageQml.matchAll(/change\("history-clear"/g)].length, 1);
  assert.equal([...pageQml.matchAll(/stream\("update"/g)].length, 1);
});

test("commands_and_watches: what the page runs exists, the reads follow the files, and the changers answer in JSON", () => {
  const ids = new Set(manifest.command.map((c) => c.id));
  const used = new Set();
  for (const m of pageQml.matchAll(/(?:command\.(?:run|watch|stream)|change)\(\s*"([a-z-]+)"/g)) used.add(m[1]);
  for (const m of pageQml.matchAll(/command\.run\(filter\.currentIndex === 1 \? "([a-z-]+)" : "([a-z-]+)"/g)) used.add(m[1]).add(m[2]);
  used.add("mute").add("unmute"); // chosen by Logic.muteCommand
  used.add("history-clear-plan").add("update-plan"); // run by Settings before the confirmation, not by the page
  assert.deepEqual([...used].sort(), [...ids].sort(), "every command is used and every use is declared");

  const files = (id) => plain(command(id).watch?.files);
  assert.deepEqual(files("status"), ["~/.local/state/notifd/state.json", "~/.config/notifd/rules.json"]);
  for (const id of ["settings", "focus-modes", "config", "rules"]) assert.deepEqual(files(id), ["~/.config/notifd/rules.json"], id);
  for (const c of manifest.command.filter((c) => c.watch)) assert.ok(!c.args && !c.confirm, `${c.id}: only a plain read can be watched`);
  for (const id of ["status", "settings", "focus-modes", "config", "rules"]) assert.ok(new RegExp(`watch\\("${id}"`).test(pageQml), `${id} is watched`);

  assert.deepEqual(command("status").argv, ["/usr/bin/notify", "status", "--json"]);
  assert.deepEqual(command("focus-modes").argv, ["/usr/bin/notify", "focus", "list", "--json"]);
  assert.deepEqual(command("config").argv, ["/usr/bin/notify", "config", "show", "--json"]);
  assert.deepEqual(command("history").argv, ["/usr/bin/notify", "history", "--json", "--no-text", "-n", "100"]);
  assert.deepEqual(command("history-held").argv, [...command("history").argv, "--missed"]);
  for (const id of ["history", "history-held"]) assert.ok(command(id).argv.includes("--no-text"), `${id} leaves every title and text out`);
  for (const id of ["dnd", "focus", "mute", "unmute"]) {
    assert.equal(command(id).output, "json", id);
    assert.ok(command(id).argv.includes("--json"), id);
  }
  assert.equal(command("config-set").output, "text");
  assert.deepEqual(command("config-set").argv, ["/usr/bin/notify", "config", "set"]);
  assert.deepEqual(command("config-reset").argv, ["/usr/bin/notify", "config", "reset"]);
  assert.ok(command("doctor").timeout_ms >= 30000, "the check-up can take a while");
  assert.deepEqual(plain(manifest.requires), { bins: ["notify"] });
  assert.equal(manifest.id, "noti-fi");
  assert.equal(manifest.name, "Notifications");
  assert.equal(manifest.icon, "preferences-desktop-notification");
  assert.equal(manifest.uq_api, 3, "previews and streaming need uq_api 3");
  assert.equal(manifest.dbus, undefined, "no D-Bus access is needed");
});

test("settings_rows: every row has keywords and a row in the page", () => {
  const ids = manifest.setting.map((s) => s.id);
  assert.deepEqual(ids, ["dnd", "focus", "now", "groups", "auto-dnd", "urgent", "history", "recent", "rules", "rules-check", "check-up", "updates", "popups", "sound", "appearance", "list"]);
  const rowIds = new Set([...allQml.matchAll(/settingId: "([A-Za-z0-9._-]+)"/g)].map((m) => m[1]));
  for (const c of plain(logic.PREF_CARDS)) rowIds.add(c.id); // the first row of a Preferences card
  for (const s of manifest.setting) {
    assert.ok(s.keywords?.length > 0, `${s.id} has keywords`);
    assert.ok(rowIds.has(s.id), `the page has a row for ${s.id}`);
  }
  for (const id of rowIds) assert.ok(ids.includes(id), `row ${id} is in the manifest, so Settings search finds it`);
  for (const w of ["notifications", "noti-fi", "do not disturb", "dnd", "focus", "mute", "history"]) assert.ok(manifest.keywords.includes(w), w);
});

test("tabs: the page has the six tabs, in the planned order", () => {
  const titleOf = (f) => qmlOf(f).match(/^\s*SettingsTab \{[\s\S]*?\n\s*title: "([A-Za-z ]+)"/m)?.[1];
  assert.deepEqual(["OverviewTab.qml", "QuietTab.qml", "HistoryTab.qml"].map(titleOf), ["Overview", "Quiet", "History"]);
  const inPage = [...qmlOf("Page.qml").matchAll(/^    SettingsTab \{\n\s+title: "([A-Za-z]+)"/gm)].map((m) => m[1]);
  assert.deepEqual(inPage, ["Rules", "Health", "Preferences"]);
  const order = [...qmlOf("Page.qml").matchAll(/^    (OverviewTab|QuietTab|HistoryTab|SettingsTab) \{/gm)].map((m) => m[1]);
  assert.deepEqual(order, ["OverviewTab", "QuietTab", "HistoryTab", "SettingsTab", "SettingsTab", "SettingsTab"]);
});

// ---------------------------------------------------------------- reads

test("history_rows: who sent it, which group, when and why it was held, never the words", () => {
  const now = new Date(2026, 10, 16, 15, 30);
  const at = (d, h, m) => new Date(d.getFullYear(), d.getMonth(), d.getDate(), h, m).getTime();
  const entries = [
    { t: at(now, 15, 29) + 30000, id: 1, app: "Chat", zone: "personal", shown: true, why: "", icon: "chat", summary: "SECRET", body: "SECRET" },
    { t: at(now, 9, 5), id: 2, app: "", zone: "work", shown: false, why: "muted work", icon: "/etc/passwd" },
    { t: at(new Date(2026, 10, 15), 23, 59), id: 3, app: "Mail", zone: "personal", shown: false, why: "fullscreen", icon: "mail-unread" },
    { t: at(new Date(2026, 2, 3), 14, 5), id: 4, app: "Old", zone: "x", shown: false, why: "something new", icon: "a b" },
    null, 5, { app: 3 }
  ];
  const h = plain(logic.parseHistory(ok(entries), now));
  assert.equal(h.ok, true);
  assert.deepEqual(h.rows.map((r) => r.time), ["just now", "09:05", "Yesterday 23:59", "3 Mar 14:05"]);
  assert.deepEqual(h.rows.map((r) => r.why), ["", "Group muted", "A full-screen app", "Held back"]);
  assert.deepEqual(h.rows.map((r) => r.icon), ["chat", "", "mail-unread", ""], "only a plain icon name is passed on");
  assert.equal(h.rows[1].app, "An app");
  assert.ok(!JSON.stringify(h).includes("SECRET"));
  assert.equal(logic.parseHistory(ok(Array.from({ length: 900 }, (_, i) => ({ app: "a", id: i }))), now).rows.length, 500);
  for (const bad of [null, { error: "x" }, ok(null), ok({}), ok("no")]) assert.equal(logic.parseHistory(bad, now).ok, false, JSON.stringify(bad));
  assert.equal(logic.parseHistory({ error: "exit code 1: error: not running" }, now).message, "not running");
  assert.deepEqual(plain(logic.parseHistory(ok(fixture("history.json")), now).rows.map((r) => r.app)), ["Chat", "Mail"]);
  assert.equal(fixture("history.json").every((e) => e.summary === "" && e.body === ""), true, "the fixture is `--no-text` output");
});

test("check_up: groups in order, the program's own counts, and a report under an error code still counts", () => {
  const d = plain(logic.parseDoctor({ code: 1, error: "exit code 1", output: fixture("doctor.json") }));
  assert.equal(d.ok, true);
  assert.deepEqual(d.groups.map((g) => g.name), ["Tools", "Install", "Daemon"]);
  assert.deepEqual(d.groups[2].checks, [{ level: "fail", message: "no notifd instance answering on the control socket", hint: "systemctl --user start notifd" }], "FAIL is read as fail");
  assert.deepEqual(d.summary, { ok: 2, warn: 2, fail: 2 });
  assert.deepEqual(d.rules, { errors: ["rule 3: not a valid pattern"], warnings: ["rule 5 never matches anything"] });
  assert.equal(logic.summaryLine(d), "2 problems to fix, 2 things to look at.");
  assert.equal(logic.summaryLine({ ok: true, summary: { ok: 5, warn: 1, fail: 0 } }), "1 thing to look at.");
  assert.equal(logic.summaryLine({ ok: true, summary: { ok: 5, warn: 0, fail: 1 } }), "1 problem to fix.");
  assert.equal(logic.summaryLine({ ok: true, summary: { ok: 5, warn: 0, fail: 0 } }), "Everything checks out.");
  // without a summary of its own the page counts
  const counted = plain(logic.parseDoctor(ok({ checks: [{ group: "A", level: "warn", message: "w" }, { group: "A", level: "ok", message: "o" }, { level: "weird", message: "?" }] })));
  assert.deepEqual(counted.summary, { ok: 1, warn: 1, fail: 0 });
  assert.deepEqual(counted.groups.map((g) => g.name), ["A", "Other"]);
  assert.equal(counted.groups[1].checks[0].level, "info", "an unknown level is just a note");
  for (const bad of [null, { error: "timed out" }, ok(null), ok({}), ok({ checks: 3 })]) assert.equal(logic.parseDoctor(bad).ok, false, JSON.stringify(bad));
  assert.equal(logic.parseDoctor({ error: "timed out after 60 s" }).message, "timed out after 60 s");
  const r = plain(logic.parseRules({ code: 1, error: "exit code 1", output: { errors: ["bad"], warnings: [], daemon: null } }));
  assert.deepEqual(r, { ok: true, message: "", errors: ["bad"], warnings: [] });
  assert.equal(logic.parseRules(ok(fixture("rules.json"))).warnings.length, 1);
  assert.equal(logic.parseRules({ error: "x" }).ok, false);
});

test("updates: the verdict, the streamed progress and the closing line", () => {
  const u = plain(logic.parseUpdate(ok(fixture("update-check.json"))));
  assert.deepEqual(u, { ok: true, message: "", newer: true, current: "0.19.1", latest: "0.20.0" });
  assert.equal(logic.parseUpdate(ok({ current: "1", latest: "1", newer: false })).newer, false);
  for (const bad of [null, { error: "x" }, ok({}), ok({ newer: "yes" })]) assert.equal(logic.parseUpdate(bad).ok, false, JSON.stringify(bad));
  assert.equal(logic.parseUpdate({ error: "exit code 1: error: could not check for updates: offline" }).message, "could not check for updates: offline");

  assert.deepEqual(plain(logic.progressOf({ type: "plan", total: 3 })), { fraction: 0, text: "Starting…" });
  assert.deepEqual(plain(logic.progressOf({ type: "step", n: 1, total: 3, desc: "download", status: "running" })), { fraction: 0, text: "download" });
  assert.deepEqual(plain(logic.progressOf({ type: "step", n: 2, total: 4, desc: "replace", status: "done" })), { fraction: 0.5, text: "replace" });
  assert.equal(logic.progressOf({ type: "step", n: 9, total: 3, status: "done" }).fraction, 1, "never past the end");
  for (const none of [null, 3, "x", { type: "result" }, { type: "step", n: "1", total: 3 }, { type: "step", n: 1, total: 0 }]) assert.equal(logic.progressOf(none), null, JSON.stringify(none));
  assert.deepEqual(plain(logic.updateOutcome({ type: "result", ok: true, message: "updated to noti-fi 0.20.0." }, { code: 0, output: null })), { ok: true, message: "updated to noti-fi 0.20.0.", detail: "" });
  assert.deepEqual(plain(logic.updateOutcome({ type: "result", ok: false, message: "checksum mismatch" }, { code: 1, error: "exit code 1" })), { ok: false, message: "Couldn't update noti-fi.", detail: "checksum mismatch" });
  assert.equal(logic.updateOutcome(null, { error: "timed out" }).ok, false);
  assert.equal(logic.updateOutcome(null, { code: 0 }).message, "Updated.");
});

test("right_now: counts and a plain reason, never a notification's words", () => {
  const s = plain(logic.parseStatus(ok(fixture("status.json"))));
  assert.deepEqual(plain(logic.nowLines(s)), ["0 popups on screen", "3 unread notifications"]);
  assert.deepEqual(plain(logic.nowLines({ ...s, active: 1, queued: 2, unread: 1, holding: "share" })), ["1 popup on screen, 2 waiting", "1 unread notification", "Your screen is being shared."]);
  assert.equal(logic.holdingText("something-new"), "Popups are being held.");
  assert.equal(logic.holdingText(""), "");
  assert.ok(logic.nowLines({ ...s, sharing: true }).includes("Your screen is being shared, so notification text is hidden."));
});

test("notes: setting one leaves the others, and removing one leaves no trace", () => {
  const a = plain(logic.withNote({}, "x", { message: "m" }));
  const b = plain(logic.withNote(a, "y", { message: "n" }));
  assert.deepEqual(Object.keys(b), ["x", "y"]);
  assert.deepEqual(plain(logic.withNote(b, "x", null)), { y: { message: "n" } });
  assert.deepEqual(a, { x: { message: "m" } }, "the original is not changed");
});

// ---------------------------------------------------------------- what the page shows

test("no_notification_text: the page never reads a title, a body or an app's name from the state", () => {
  const status = logic.parseStatus(ok(fixture("status.json")));
  assert.ok(!JSON.stringify(plain(status)).includes("Quarterly"));
  assert.ok(!JSON.stringify(plain(logic.groupRows(status, null))).includes("Quarterly"));
  // the history is asked for without text, and the page only reads who, where and when
  const logicSrc = readFileSync(join(pageDir, "logic.js"), "utf8").replace(/\/\/.*$/gm, "");
  for (const read of [/\be\.(summary|body)\b/, /\.appName\b/, /\blatest\b\s*[.:]\s*(summary|appName)/, /sharingBy/, /inhibitors/, /doc\.latest/]) assert.ok(!read.test(logicSrc), `${read} is never read`);
  for (const f of qmlFiles) assert.ok(!/modelData\.(summary|body)|appName|sharingBy|inhibitors/.test(qmlOf(f).replace(/\/\/.*$/gm, "")), `${f} reads no notification text`);
  for (const row of plain(logic.parseHistory(ok(fixture("history.json")), new Date()).rows)) assert.deepEqual(Object.keys(row).sort(), ["app", "group", "icon", "id", "shown", "time", "why"]);
});

test("page_strings_are_plain: no jargon in anything the page or the manifest shows", () => {
  const jargon = [/D-Bus/i, /\bdbus\b/i, /polkit/i, /\bJSON\b/i, /systemctl/i, /sched_ext/i, /EEVDF/i, /zram/i, /sysctl/i, /notifd/i, /quickshell/i, /daemon/i, /socket/i, /rules\.json/i, /\bargv\b/i, /\bcli\b/i, /ndjson/i, /\bstream\b/i];
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
  // property names and command words in the code are not shown; text has a capital or a space
  const texty = (list) => list.filter((t) => /[A-Z\s]/.test(t) && !/^[a-z]+(\.[A-Za-z]+)*\s?$/.test(t));
  const shown = [
    ...qmlFiles.flatMap((f) => texty(literals(qmlOf(f)))),
    ...texty(literals(readFileSync(join(pageDir, "logic.js"), "utf8"))),
    manifest.name,
    ...manifest.setting.map((s) => s.label),
    ...manifest.command.filter((c) => c.confirm).map((c) => c.confirm)
  ];
  assert.ok(shown.length > 100, `the extractor found the strings (${shown.length})`);
  for (const text of shown) for (const re of jargon) assert.ok(!re.test(text), `"${text}" shows ${re}`);
  // what the CLI says is shown as it is (the check-up quotes its checks), but the page's own words stay in sentence case
  for (const w of ["Do not disturb", "Clear history…", "Open noti-fi", "Send a test notification", "Check for updates", "Run check-up"]) assert.ok(shown.includes(w), w);
  assert.ok(shown.includes("noti-fi isn't running. uQ's own notifications are in use."));
});

test("every_qml_file_is_used_and_short: no orphan file, and each stays within the project's 400 lines", () => {
  for (const f of qmlFiles) {
    assert.ok(qmlOf(f).split("\n").length <= 400, `${f} is over 400 lines`);
    const type = f.replace(".qml", "");
    if (f === "Page.qml") continue;
    assert.ok(qmlFiles.some((o) => o !== f && new RegExp(`\\b${type}\\s*\\{`).test(qmlOf(o))), `${f} is used by another file`);
  }
});
