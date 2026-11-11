.pragma library
// The Notifications page's pure logic: no Qt calls, so Node can test it too (tests/uq/logic.test.mjs).
// Everything that comes from `notify` is untrusted: it is parsed defensively here, capped, and shown only through
// UqLabel and the components of org.uq.settings. No notification text (summaries, bodies) is ever read.

var MAX_ROWS = 500;
var MAX_TEXT = 200;

/// The argument patterns, exactly as in page.toml (a test compares them).
var PATTERNS = {
    dnd: { mode: "^(on|off)$" },
    focus: { mode: "^(off|[a-z0-9_][a-z0-9_-]{0,39})$" },
    mute: { zone: "^[A-Za-z0-9_][A-Za-z0-9_-]{0,39}$" },
    unmute: { zone: "^[A-Za-z0-9_][A-Za-z0-9_-]{0,39}$" },
    set: {
        key: "^(history\\.limit|history\\.persist|dnd\\.allowCritical|autoDnd\\.fullscreen|autoDnd\\.screenShare)$",
        value: "^(on|off|50|100|200|500|1000)$"
    }
};

/// The confirms of page.toml that need one (a test compares them).
var CONFIRMS = {
    "history-clear": "Clear the notification history? This can't be undone."
};

/// What `notify set` takes for each key. The page sends no other pair.
var SET_VALUES = {
    "history.limit": ["50", "100", "200", "500", "1000"],
    "history.persist": ["on", "off"],
    "dnd.allowCritical": ["on", "off"],
    "autoDnd.fullscreen": ["on", "off"],
    "autoDnd.screenShare": ["on", "off"]
};

var LIMITS = [50, 100, 200, 500, 1000];

var NOT_RUNNING = "noti-fi isn't running. uQ's own notifications are in use.";
var APP_ID = "org.notify.Panel.desktop";

function clip(value) {
    var s = typeof value === "string" ? value : "";
    return s.length > MAX_TEXT ? s.slice(0, MAX_TEXT) : s;
}

function isObject(v) {
    return v !== null && typeof v === "object" && !Array.isArray(v);
}

function matches(pattern, text) {
    return typeof text === "string" && new RegExp(pattern).test(text);
}

/// The first line of an error, without uQ's or notify's wrapping ("exit code 1: error: ..."), cut short.
function messageOf(result) {
    if (!result)
        return "";
    if (isObject(result.output) && typeof result.output.message === "string" && result.output.message !== "")
        return clip(result.output.message);
    var text = typeof result.error === "string" ? result.error : "";
    var at = text.lastIndexOf("error: ");
    if (at >= 0)
        text = text.slice(at + 7);
    return clip(text.split("\n")[0]);
}

/// True when the person pressed Cancel on a confirm: nothing changes and nothing is shown.
function cancelled(result) {
    return !!(result && result.cancelled);
}

/// "" when a command worked, else the plain sentence to show (a failed write puts the control back and shows this).
/// The program's own words for it, for "Advanced details", are `detailOf`.
function problemOf(result, doing) {
    if (!result)
        return doing + ".";
    if (cancelled(result))
        return "";
    if (result.error === undefined && (result.code === undefined || result.code === 0))
        return "";
    return doing + ".";
}

function detailOf(result) {
    if (!result)
        return "No answer.";
    return messageOf(result) || "No reason was given.";
}

function stringList(v) {
    var out = [];
    if (!Array.isArray(v))
        return out;
    for (var i = 0; i < v.length && out.length < MAX_ROWS; ++i)
        if (typeof v[i] === "string")
            out.push(clip(v[i]));
    return out;
}

/// The fields of a Status document that the page uses.
function statusFields(doc) {
    return {
        dnd: doc.dnd === true,
        muted: stringList(doc.muted),
        zones: stringList(doc.zones),
        focus: clip(doc.focus),
        history: typeof doc.history === "number" && doc.history >= 0 ? Math.floor(doc.history) : 0,
        cfgError: clip(doc.cfgError)
    };
}

/// The result of `status`: {kind: "ready" | "notRunning" | "error", message, ...fields}.
/// `notify status --json` wraps the daemon's state in `daemon`, which is null while noti-fi isn't running.
function parseStatus(result) {
    if (!result)
        return { kind: "error", message: "No answer." };
    if (result.error !== undefined)
        return { kind: "error", message: messageOf(result) || "Couldn't read noti-fi's state." };
    var out = result.output;
    if (!isObject(out) || !("daemon" in out))
        return { kind: "error", message: "Couldn't read noti-fi's state." };
    if (out.daemon === null)
        return { kind: "notRunning", message: NOT_RUNNING };
    if (!isObject(out.daemon))
        return { kind: "error", message: "Couldn't read noti-fi's state." };
    var s = statusFields(out.daemon);
    s.kind = "ready";
    s.message = "";
    return s;
}

/// The answer of dnd, mute, unmute and focus: one Status document (not wrapped). Null when it isn't one.
function parseStatusDocument(result) {
    if (!result || result.error !== undefined || !isObject(result.output) || typeof result.output.dnd !== "boolean")
        return null;
    return statusFields(result.output);
}

/// The result of `settings`: {ok, message, detail, allowCritical, limit, persist, fullscreen, screenShare, zones, rules}.
function parseSettings(result) {
    var bad = { ok: false, message: "Couldn't read noti-fi's settings.", detail: "" };
    if (!result || result.error !== undefined) {
        bad.detail = messageOf(result);
        return bad;
    }
    var o = result.output;
    if (!isObject(o) || !isObject(o.dnd) || !isObject(o.history) || !isObject(o.autoDnd))
        return bad;
    if (typeof o.dnd.allowCritical !== "boolean" || typeof o.history.persist !== "boolean" || typeof o.history.limit !== "number")
        return bad;
    if (typeof o.autoDnd.fullscreen !== "boolean" || typeof o.autoDnd.screenShare !== "boolean")
        return bad;
    var zones = [];
    if (Array.isArray(o.zones))
        for (var i = 0; i < o.zones.length && zones.length < MAX_ROWS; ++i) {
            var z = o.zones[i];
            if (isObject(z) && typeof z.id === "string" && z.id !== "")
                zones.push({ id: clip(z.id), label: clip(z.label) !== "" ? clip(z.label) : clip(z.id) });
        }
    var rules = [];
    if (Array.isArray(o.rules))
        for (var j = 0; j < o.rules.length && rules.length < MAX_ROWS; ++j)
            if (isObject(o.rules[j]) && typeof o.rules[j].name === "string")
                rules.push(clip(o.rules[j].name));
    return {
        ok: true,
        message: "",
        detail: "",
        allowCritical: o.dnd.allowCritical,
        limit: Math.floor(o.history.limit),
        persist: o.history.persist,
        fullscreen: o.autoDnd.fullscreen,
        screenShare: o.autoDnd.screenShare,
        zones: zones,
        rules: rules
    };
}

/// The result of `focus list --json`: {ok, active, modes: [{id, label}]}. A mode whose name `notify focus`
/// couldn't be given (the argument pattern) is left out: it can only be chosen in noti-fi's own app.
function parseFocusModes(result) {
    var bad = { ok: false, active: "", modes: [] };
    if (!result || result.error !== undefined || !isObject(result.output) || !isObject(result.output.modes))
        return bad;
    var modes = [];
    var ids = Object.keys(result.output.modes);
    for (var i = 0; i < ids.length && modes.length < MAX_ROWS; ++i) {
        if (!matches(PATTERNS.focus.mode, ids[i]) || ids[i] === "off")
            continue;
        var m = result.output.modes[ids[i]];
        var label = isObject(m) ? clip(m.label) : "";
        modes.push({ id: ids[i], label: label !== "" ? label : ids[i] });
    }
    return { ok: true, active: clip(result.output.active), modes: modes };
}

/// The texts of the focus mode box: "Off", then each mode's label.
function focusTexts(modes) {
    var out = ["Off"];
    for (var i = 0; i < modes.length; ++i)
        out.push(modes[i].label);
    return out;
}

/// The box index for the active mode (0 = Off). An active mode not in the list also reads as Off.
function focusIndex(modes, active) {
    for (var i = 0; i < modes.length; ++i)
        if (modes[i].id === active)
            return i + 1;
    return 0;
}

/// The argument for `focus` for a box index.
function focusArg(modes, index) {
    if (index <= 0 || index > modes.length)
        return "off";
    return modes[index - 1].id;
}

/// The limits the box offers: the panel's choices, plus the current one when it was set some other way.
function limitChoices(current) {
    var out = LIMITS.slice();
    if (typeof current === "number" && out.indexOf(current) < 0) {
        out.push(current);
        out.sort(function (a, b) { return a - b; });
    }
    return out;
}

function limitTexts(current) {
    return limitChoices(current).map(function (n) { return String(n); });
}

function limitIndex(current) {
    return limitChoices(current).indexOf(current);
}

/// `set history.limit <n>` for a box index, or null when the choice isn't one `notify set` takes.
function limitArgs(current, index) {
    var choices = limitChoices(current);
    if (index < 0 || index >= choices.length)
        return null;
    return setArgs("history.limit", String(choices[index]));
}

/// The two arguments for `set`, or null for a pair `notify set` doesn't take.
function setArgs(key, value) {
    if (!Object.prototype.hasOwnProperty.call(SET_VALUES, key))
        return null;
    if (SET_VALUES[key].indexOf(value) < 0)
        return null;
    return [key, value];
}

function boolWord(on) {
    return on ? "on" : "off";
}

function dndArgs(on) {
    return [boolWord(on)];
}

/// The command for switching a group's mute: "mute" or "unmute".
function muteCommand(muted) {
    return muted ? "mute" : "unmute";
}

/// The group rows: [{id, label, muted, changeable}]. The label is the group's own name from the rules (or its id).
function groupRows(status, settings) {
    var labels = {};
    if (settings && settings.ok)
        for (var i = 0; i < settings.zones.length; ++i)
            labels[settings.zones[i].id] = settings.zones[i].label;
    var rows = [];
    if (!status || !status.zones)
        return rows;
    for (var j = 0; j < status.zones.length && rows.length < MAX_ROWS; ++j) {
        var id = status.zones[j];
        rows.push({
            id: id,
            label: Object.prototype.hasOwnProperty.call(labels, id) ? labels[id] : id,
            muted: status.muted.indexOf(id) >= 0,
            changeable: matches(PATTERNS.mute.zone, id)
        });
    }
    return rows;
}

/// The names of the rules, "Rule N" for an unnamed one (the CLI already numbers them).
function ruleNames(settings) {
    return settings && settings.ok ? settings.rules : [];
}

/// One line saying how many notifications history holds (a number, never their text).
function historyLine(count) {
    if (count === 1)
        return "1 notification is kept.";
    return count + " notifications are kept.";
}
