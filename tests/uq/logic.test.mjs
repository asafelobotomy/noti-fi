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
// logic.js: shared helpers, status, settings and preferences; reports.js: history, check-up, updates.
const logic = { ...loadLogic(join(pageDir, "logic.js")), ...loadLogic(join(pageDir, "reports.js")) };
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

