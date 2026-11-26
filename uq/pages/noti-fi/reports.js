.pragma library
.import "logic.js" as Logic

// What noti-fi reports for the History, Check-up and Updates tabs: held notifications (never
// their words), the doctor's checks and the update verdict and progress. No Qt calls, so Node
// can test it (tests/uq/reports.test.mjs). Shared helpers and the patterns are in logic.js.

var MAX_ROWS = Logic.MAX_ROWS;
var clip = Logic.clip;
var isObject = Logic.isObject;
var messageOf = Logic.messageOf;
var stringList = Logic.stringList;
var count = Logic.count;
var plural = Logic.plural;

// ------------------------------------------------------------ history (`notify history --json --no-text`)

var WHY = {
    dnd: "Do not disturb",
    focus: "Focus mode",
    fullscreen: "A full-screen app",
    share: "Screen sharing",
    "silent rule": "A rule hid it",
    inhibit: "An app asked for quiet"
};

function whyText(why) {
    if (typeof why !== "string" || why === "")
        return "";
    if (Object.prototype.hasOwnProperty.call(WHY, why))
        return WHY[why];
    if (why.indexOf("muted ") === 0)
        return "Group muted";
    return "Held back";
}

var MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

function two(n) {
    return (n < 10 ? "0" : "") + n;
}

/// "just now", "14:05", "Yesterday 14:05" or "3 Mar 14:05" for a time in ms; `now` is a Date.
function timeText(t, now) {
    if (typeof t !== "number" || !isFinite(t))
        return "";
    var d = new Date(t);
    var clock = two(d.getHours()) + ":" + two(d.getMinutes());
    var days = Math.round((new Date(now.getFullYear(), now.getMonth(), now.getDate()) - new Date(d.getFullYear(), d.getMonth(), d.getDate())) / 86400000);
    if (now - d >= 0 && now - d < 60000)
        return "just now";
    if (days === 0)
        return clock;
    if (days === 1)
        return "Yesterday " + clock;
    return d.getDate() + " " + MONTHS[d.getMonth()] + " " + clock;
}

/// The result of `history --no-text`: {ok, message, rows: [{id, app, group, icon, time, shown, why}]}.
/// Rows carry who sent it, where it went and when, never a title or text.
function parseHistory(result, now) {
    var bad = { ok: false, message: "Couldn't read the history.", rows: [] };
    if (!result || result.error !== undefined || !Array.isArray(result.output)) {
        bad.message = result && result.error !== undefined ? messageOf(result) || bad.message : bad.message;
        return bad;
    }
    var rows = [];
    for (var i = 0; i < result.output.length && rows.length < MAX_ROWS; ++i) {
        var e = result.output[i];
        if (!isObject(e) || typeof e.app !== "string")
            continue;
        var icon = typeof e.icon === "string" && /^[A-Za-z0-9._-]{1,100}$/.test(e.icon) ? e.icon : "";
        rows.push({
            id: typeof e.id === "number" ? e.id : i,
            app: clip(e.app) !== "" ? clip(e.app) : "An app",
            group: clip(e.zone),
            icon: icon,
            time: timeText(e.t, now),
            shown: e.shown !== false,
            why: whyText(e.why)
        });
    }
    return { ok: true, message: "", rows: rows };
}

// ------------------------------------------------------------ check-up (`notify doctor --json`) and rules (`notify rules --json`)

var LEVELS = { ok: "ok", warn: "warn", fail: "fail", info: "info" };

/// The result of `doctor`: {ok, message, summary: {ok, warn, fail}, groups: [{name, checks: [{level, message, hint}]}], rules}.
/// doctor exits 1 when your rules have an error but still prints its report, so a report under an error code counts.
function parseDoctor(result) {
    var bad = { ok: false, message: "Couldn't run the check-up.", summary: { ok: 0, warn: 0, fail: 0 }, groups: [], rules: { errors: [], warnings: [] } };
    if (!result || !isObject(result.output) || !Array.isArray(result.output.checks)) {
        bad.message = result && result.error !== undefined ? messageOf(result) || bad.message : bad.message;
        return bad;
    }
    var groups = [];
    var byName = {};
    var summary = { ok: 0, warn: 0, fail: 0 };
    for (var i = 0; i < result.output.checks.length && i < MAX_ROWS; ++i) {
        var c = result.output.checks[i];
        if (!isObject(c) || typeof c.message !== "string")
            continue;
        var level = typeof c.level === "string" && Object.prototype.hasOwnProperty.call(LEVELS, c.level.toLowerCase()) ? LEVELS[c.level.toLowerCase()] : "info";
        var name = clip(c.group) !== "" ? clip(c.group) : "Other";
        if (!Object.prototype.hasOwnProperty.call(byName, name)) {
            byName[name] = { name: name, checks: [] };
            groups.push(byName[name]);
        }
        byName[name].checks.push({ level: level, message: clip(c.message), hint: clip(c.hint) });
        if (level !== "info")
            summary[level] += 1;
    }
    // doctor counts its own findings (a rules error is one of them): use its numbers when it gives them
    var given = result.output.summary;
    if (isObject(given) && typeof given.ok === "number" && typeof given.warn === "number" && typeof given.fail === "number")
        summary = { ok: count(given.ok), warn: count(given.warn), fail: count(given.fail) };
    return { ok: true, message: "", summary: summary, groups: groups, rules: rulesFields(result.output.rules) };
}

function rulesFields(doc) {
    return { errors: isObject(doc) ? stringList(doc.errors) : [], warnings: isObject(doc) ? stringList(doc.warnings) : [] };
}

/// The result of `rules`: {ok, message, errors, warnings}. Like doctor it can exit 1 and still print its verdict.
function parseRules(result) {
    if (!result || !isObject(result.output) || !Array.isArray(result.output.errors))
        return { ok: false, message: (result && messageOf(result)) || "Couldn't check your rules.", errors: [], warnings: [] };
    var f = rulesFields(result.output);
    return { ok: true, message: "", errors: f.errors, warnings: f.warnings };
}

/// One line for the check-up's result.
function summaryLine(doctor) {
    if (!doctor.ok)
        return doctor.message;
    var s = doctor.summary;
    if (s.fail > 0)
        return plural(s.fail, "problem", "problems") + " to fix" + (s.warn > 0 ? ", " + plural(s.warn, "thing", "things") + " to look at." : ".");
    if (s.warn > 0)
        return plural(s.warn, "thing", "things") + " to look at.";
    return "Everything checks out.";
}

// ------------------------------------------------------------ updates (`notify upgrade --json`, `upgrade --apply --json`)

/// The result of `update-check`: {ok, message, newer, current, latest}.
function parseUpdate(result) {
    if (!result || result.error !== undefined || !isObject(result.output) || typeof result.output.newer !== "boolean")
        return { ok: false, message: (result && messageOf(result)) || "Couldn't check for updates.", newer: false, current: "", latest: "" };
    return { ok: true, message: "", newer: result.output.newer, current: clip(result.output.current), latest: clip(result.output.latest) };
}

/// A streamed event of `update` as {fraction (0 to 1), text}, or null for one that says nothing about progress.
function progressOf(ev) {
    if (!isObject(ev))
        return null;
    if (ev.type === "plan")
        return { fraction: 0, text: "Starting…" };
    if (ev.type === "step" && typeof ev.n === "number" && typeof ev.total === "number" && ev.total > 0) {
        var done = ev.status === "done" ? 1 : 0;
        return { fraction: Math.max(0, Math.min(1, (ev.n - 1 + done) / ev.total)), text: clip(ev.desc) };
    }
    return null;
}

/// What to say when `update` ends: `last` is the final `result` event it printed (or null), `done` the end of the run.
function updateOutcome(last, done) {
    if (done && done.error !== undefined && !done.cancelled)
        return { ok: false, message: "Couldn't update noti-fi.", detail: (last && clip(last.message)) || messageOf(done) };
    if (last && last.ok === false)
        return { ok: false, message: "Couldn't update noti-fi.", detail: clip(last.message) };
    return { ok: true, message: last && typeof last.message === "string" ? clip(last.message) : "Updated.", detail: "" };
}

