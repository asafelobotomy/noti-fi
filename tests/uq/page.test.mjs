// Node tests for the Notifications page: reports.js (history, check-up, updates), what is shown
// right now, notes, and the page's QML and words. The rest is in logic.test.mjs.
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
  const logicSrc = ["logic.js", "reports.js"].map((f) => readFileSync(join(pageDir, f), "utf8")).join("\n").replace(/\/\/.*$/gm, "");
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
    ...["logic.js", "reports.js"].flatMap((f) => texty(literals(readFileSync(join(pageDir, f), "utf8")))),
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
