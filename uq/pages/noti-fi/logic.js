.pragma library
// The Notifications page's pure logic: no Qt calls, so Node can test it too (tests/uq/logic.test.mjs).
// Everything that comes from `notify` is untrusted: it is parsed defensively here, capped, and shown only through
// UqLabel and the components of org.uq.settings. No notification text (summaries, bodies) is ever read.

var MAX_ROWS = 500;
var MAX_TEXT = 200;

/// Every setting `notify config` knows, in the order the panel lists them (a test compares this with the manifest and with the CLI's fixture).
var PREF_KEYS = [
    "screen", "defaultZone", "defaultTimeout", "maxVisible", "width", "margin", "spacing",
    "dnd.allowCritical", "history.limit", "history.persist", "sound.enabled", "sound.defaultName",
    "autoDnd.fullscreen", "autoDnd.screenShare",
    "theme.card", "theme.border", "theme.text", "theme.subtext", "theme.button", "theme.buttonHover", "theme.opacity",
    "theme.fontFamily", "theme.fontSize", "theme.timerStyle", "theme.radius", "theme.tokens",
    "transfer.chart", "center.anchor", "center.keep"
];

var KEY_PATTERN = "^(" + PREF_KEYS.map(function (k) { return k.replace(/\./g, "\\."); }).join("|") + ")$";
/// A value starts and ends with a letter, digit, # or . and may hold spaces and hyphens inside (a font's name, a colour, a number, on, off).
var VALUE_PATTERN = "^[A-Za-z0-9#.]([A-Za-z0-9#.,_ -]{0,98}[A-Za-z0-9#.,_-])?$";

/// The argument patterns, exactly as in page.toml (a test compares them).
var PATTERNS = {
    dnd: { mode: "^(on|off)$" },
    focus: { mode: "^(off|[A-Za-z0-9_][A-Za-z0-9_-]{0,39})$" },
    mute: { zone: "^[A-Za-z0-9_][A-Za-z0-9_-]{0,39}$" },
    unmute: { zone: "^[A-Za-z0-9_][A-Za-z0-9_-]{0,39}$" },
    "config-set": { key: KEY_PATTERN, value: VALUE_PATTERN },
    "config-reset": { key: KEY_PATTERN }
};

/// The confirms of page.toml that need one (a test compares them). Each is followed by the plan of the command's `preview`.
var CONFIRMS = {
    "history-clear": "Clear the notification history? This can't be undone.",
    "update": "Update noti-fi? Notifications pause for about a second."
};

var LIMITS = [50, 100, 200, 500, 1000];
var WORKING = "Working…";

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

/// True for the answer of a read that ran while a watched run of the same command was still going (uQ allows one at a time).
function overlapped(result) {
    return !!(result && result.error === "already running");
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

function count(v) {
    return typeof v === "number" && v >= 0 ? Math.floor(v) : 0;
}

/// The fields of a Status document that the page uses.
function statusFields(doc) {
    return {
        dnd: doc.dnd === true,
        muted: stringList(doc.muted),
        zones: stringList(doc.zones),
        focus: clip(doc.focus),
        history: count(doc.history),
        cfgError: clip(doc.cfgError),
        active: count(doc.active),
        queued: count(doc.queued),
        unread: count(doc.unread),
        holding: clip(doc.holding),
        sharing: doc.sharing === true
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
/// couldn't be given (the argument pattern), or that `notify focus` would read as a subcommand (`list`), is left out: it can only be chosen in noti-fi's own app.
function parseFocusModes(result) {
    var bad = { ok: false, active: "", modes: [] };
    if (!result || result.error !== undefined || !isObject(result.output) || !isObject(result.output.modes))
        return bad;
    var modes = [];
    var ids = Object.keys(result.output.modes);
    for (var i = 0; i < ids.length && modes.length < MAX_ROWS; ++i) {
        if (!matches(PATTERNS.focus.mode, ids[i]) || ids[i] === "off" || ids[i] === "list")
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

/// The limit for a box index, or null when it isn't one of the panel's choices (an odd current limit is shown, never sent back).
function limitValue(current, index) {
    var choices = limitChoices(current);
    if (index < 0 || index >= choices.length || LIMITS.indexOf(choices[index]) < 0)
        return null;
    return String(choices[index]);
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

// ------------------------------------------------------------ what is happening now

/// Why popups are being held, in the person's words ("" = they are not).
function holdingText(holding) {
    var words = {
        dnd: "Do not disturb is on.",
        focus: "A focus mode is on.",
        fullscreen: "A full-screen app is in front.",
        share: "Your screen is being shared.",
        inhibit: "An app asked for quiet."
    };
    if (holding === "")
        return "";
    return Object.prototype.hasOwnProperty.call(words, holding) ? words[holding] : "Popups are being held.";
}

function plural(n, one, many) {
    return n + " " + (n === 1 ? one : many);
}

/// The lines under "Right now": counts only, never what a notification says.
function nowLines(status) {
    var lines = [plural(status.active, "popup", "popups") + " on screen" + (status.queued > 0 ? ", " + status.queued + " waiting" : "")];
    lines.push(plural(status.unread, "unread notification", "unread notifications"));
    if (status.holding !== "")
        lines.push(holdingText(status.holding));
    else if (status.sharing)
        lines.push("Your screen is being shared, so notification text is hidden.");
    return lines;
}

// ------------------------------------------------------------ preferences (`notify config show --json`)

/// The cards of the Preferences tab: what each setting is called, in the person's words. The CLI knows the types and choices.
var PREF_CARDS = [
    { id: "popups", title: "Popups", icon: "window", help: "Where popups show and how long they stay. An app can ask for its own time.", rows: [
        { key: "defaultTimeout", label: "How long a popup stays", unit: "ms", hint: "In milliseconds. 0 keeps it until you close it." },
        { key: "maxVisible", label: "Popups shown at once, per group", hint: "The rest wait their turn. 0 shows them all." },
        { key: "width", label: "Popup width", unit: "px" },
        { key: "margin", label: "Gap from the screen edge", unit: "px" },
        { key: "spacing", label: "Gap between popups", unit: "px" },
        { key: "screen", label: "Screen", hint: "A screen's name, or focused for the one you are using. Empty uses the first." },
        { key: "defaultZone", label: "Group for anything no rule places", hint: "A group's id, as your rules name it." }
    ] },
    { id: "sound", title: "Sound", icon: "audio-volume-high", help: "The sound popups play.", rows: [
        { key: "sound.enabled", label: "Play a sound" },
        { key: "sound.defaultName", label: "Sound", hint: "A name from your sound theme. Empty plays none." }
    ] },
    { id: "appearance", title: "Look", icon: "preferences-desktop-theme", help: "Colours, text and corners of popups and the list. Reset puts a setting back to its default.", rows: [
        { key: "theme.tokens", label: "Follow uQ's colours", hint: "auto follows uQ, off keeps noti-fi's own, or give a file." },
        { key: "theme.card", label: "Background" },
        { key: "theme.border", label: "Border" },
        { key: "theme.text", label: "Text" },
        { key: "theme.subtext", label: "Secondary text" },
        { key: "theme.button", label: "Buttons" },
        { key: "theme.buttonHover", label: "Buttons under the pointer" },
        { key: "theme.opacity", label: "Background opacity", hint: "From 0.1 to 1. Text stays solid." },
        { key: "theme.radius", label: "Corner radius", unit: "px" },
        { key: "theme.fontFamily", label: "Font", hint: "Empty uses the system font." },
        { key: "theme.fontSize", label: "Text size", unit: "px" },
        { key: "theme.timerStyle", label: "Time left", hint: "How a popup shows the time it has left." }
    ] },
    { id: "list", title: "Notification list", icon: "view-list-text", help: "The list that the tray icon opens, and the transfer chart.", rows: [
        { key: "center.anchor", label: "Where the list opens" },
        { key: "center.keep", label: "Notifications the list keeps" },
        { key: "transfer.chart", label: "Transfer chart" }
    ] }
];

var NUMBER = /^[0-9]+(\.[0-9]+)?$/;
var COLOR = /^#[0-9a-fA-F]{3,8}$/;

/// The result of `config`: {ok, message, detail, byKey: {key: {key, type, default, value, set, choices}}}.
/// Only the keys the page knows are kept, and only with the type they are expected to have.
function parseConfig(result) {
    var bad = { ok: false, message: "Couldn't read noti-fi's preferences.", detail: "", byKey: {} };
    if (!result || (result.error !== undefined && !isObject(result.output))) {
        bad.detail = messageOf(result);
        return bad;
    }
    var o = result.output;
    if (!isObject(o) || !Array.isArray(o.settings))
        return bad;
    var byKey = {};
    for (var i = 0; i < o.settings.length && i < MAX_ROWS; ++i) {
        var r = o.settings[i];
        if (!isObject(r) || typeof r.key !== "string" || PREF_KEYS.indexOf(r.key) < 0)
            continue;
        if (["text", "color", "bool", "number"].indexOf(r.type) < 0)
            continue;
        var want = r.type === "bool" ? "boolean" : r.type === "number" ? "number" : "string";
        if (typeof r.value !== want)
            continue;
        byKey[r.key] = {
            key: r.key,
            type: r.type,
            value: r.value,
            set: r.set === true,
            "default": r["default"],
            choices: stringList(r.choices)
        };
    }
    return { ok: true, message: "", detail: "", byKey: byKey };
}

/// A setting's value as shown in its field.
function prefText(pref) {
    if (!pref)
        return "";
    if (pref.type === "bool")
        return pref.value ? "on" : "off";
    return String(pref.value);
}

/// What the field shows when the setting isn't in the file.
function prefDefaultText(pref) {
    return pref ? prefText({ type: pref.type, value: pref["default"] }) : "";
}

/// The two arguments for `config-set` for what was typed, or {error} saying what to type instead. `pref` is from parseConfig.
/// An empty text is not a value: use prefResetArgs for it.
function prefArgs(pref, text) {
    if (!pref)
        return { error: "That setting isn't available." };
    var t = typeof text === "string" ? text.trim() : "";
    if (t === "")
        return { error: "Type a value, or use Reset." };
    if (pref.type === "bool" && t !== "on" && t !== "off")
        return { error: "Use on or off." };
    if (pref.type === "number" && !NUMBER.test(t))
        return { error: "Use a number, like " + prefDefaultText(pref) + "." };
    if (pref.type === "color" && !COLOR.test(t))
        return { error: "Use a colour like " + prefDefaultText(pref) + "." };
    if (pref.type === "text" && pref.choices.length > 0) {
        var parts = pref.key === "theme.timerStyle" ? t.split(",") : [t];
        for (var i = 0; i < parts.length; ++i)
            if (pref.choices.indexOf(parts[i]) < 0)
                return { error: "Use one of: " + pref.choices.join(", ") + "." };
    }
    if (!matches(VALUE_PATTERN, t))
        return { error: "That has a character noti-fi doesn't take here." };
    return { args: [pref.key, t] };
}

function prefResetArgs(pref) {
    return pref && PREF_KEYS.indexOf(pref.key) >= 0 ? [pref.key] : null;
}

// ------------------------------------------------------------ notes under a row

/// A copy of `notes` with the note for `key` set (null removes it).
function withNote(notes, key, note) {
    var out = {};
    for (var k in notes)
        if (Object.prototype.hasOwnProperty.call(notes, k))
            out[k] = notes[k];
    if (note === null)
        delete out[key];
    else
        out[key] = note;
    return out;
}
