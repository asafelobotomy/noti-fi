// noti-fi's page in uQ Settings, laid out like Settings' own pages (uq_api = 3): six tabs of cards. `ctx` is set by Settings
// (docs/settings-pages.md in the uQ repository). All text goes through org.uq.settings components (plain text); parsing and the
// rules for what may be sent live in logic.js. Notification text (titles, bodies) is never read or shown here.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.uq.settings
import "logic.js" as Logic
import "reports.js" as Reports

SettingsPage {
    id: page

    property var ctx
    property var theme
    /// The [[setting]] row Settings search jumped to; the rows flash, and their tab opens.
    property string highlight

    /// loading | ready | notRunning | error
    property string kind: "loading"
    property string kindMessage
    property var status: ({ dnd: false, muted: [], zones: [], focus: "", history: 0, cfgError: "", active: 0, queued: 0, unread: 0, holding: "", sharing: false })
    property var settings: ({ ok: false, message: "", zones: [], rules: [] })
    property var focusModes: ({ ok: false, active: "", modes: [] })
    property var prefs: ({ ok: false, message: "", detail: "", byKey: ({}) })
    property var rulesCheck: ({ ok: false, message: "", errors: [], warnings: [] })
    /// Why the last page-wide change failed, shown until the next change. `problemDetail` is the program's own words for it.
    property string problem
    property string problemDetail
    /// What each row's change said, by row key: {message, detail}. Shown under the row.
    property var notes: ({})
    /// The key of the change that is running ("" = none). One change at a time.
    property string working
    /// False once the page is destroyed: late answers from Settings are then ignored.
    property bool alive: true
    /// A read of these failed (noti-fi wasn't running, say): read again when the state turns ready.
    property bool settingsFailed: false
    property bool modesFailed: false
    property var openedWith: null

    readonly property bool ready: page.kind === "ready"
    readonly property bool idle: page.working === ""
    readonly property bool known: page.ready && page.settings.ok
    readonly property bool loading: page.kind === "loading"

    pageId: "noti-fi"

    function errorColor() {
        const c = page.ctx && page.ctx.theme ? page.ctx.theme.colors : null;
        return (c && c.error) || "red";
    }

    function mutedColor() {
        const c = page.ctx && page.ctx.theme ? page.ctx.theme.colors : null;
        return (c && c.text_muted) || "gray";
    }

    function noteFor(key) {
        return page.notes[key] || null;
    }

    /// Wrap a callback for Settings: nothing happens after the page is gone.
    function guard(fn) {
        return result => {
            if (page.alive)
                fn(result);
        };
    }

    // ------------------------------------------------------------ reading

    function showStatus(result) {
        // a read that overlapped a watched run of the same command: the other answer is coming
        if (Logic.overlapped(result))
            return;
        const s = Logic.parseStatus(result);
        page.kind = s.kind;
        page.kindMessage = s.message;
        if (s.kind === "ready") {
            page.status = s;
            // the other reads failed while noti-fi was down; nothing watched would run them again
            if (page.settingsFailed) {
                page.settingsFailed = false;
                page.ctx.command.run("settings", [], page.guard(page.showSettings));
            }
            if (page.modesFailed) {
                page.modesFailed = false;
                page.ctx.command.run("focus-modes", [], page.guard(page.showModes));
            }
        }
        page.syncBoxes();
    }

    function showSettings(result) {
        if (Logic.overlapped(result))
            return;
        page.settings = Logic.parseSettings(result);
        page.settingsFailed = !page.settings.ok;
        page.syncBoxes();
    }

    function showModes(result) {
        if (Logic.overlapped(result))
            return;
        page.focusModes = Logic.parseFocusModes(result);
        page.modesFailed = !page.focusModes.ok;
        page.syncBoxes();
    }

    function showPrefs(result) {
        if (Logic.overlapped(result))
            return;
        page.prefs = Logic.parseConfig(result);
    }

    function showRules(result) {
        if (Logic.overlapped(result))
            return;
        page.rulesCheck = Reports.parseRules(result);
    }

    // Read now, and again whenever the daemon saves its state or the rules file changes (page.toml `watch`).
    onCtxChanged: {
        if (!page.ctx || page.ctx === page.openedWith)
            return;
        page.openedWith = page.ctx;
        page.ctx.command.watch("status", page.guard(page.showStatus));
        page.ctx.command.watch("settings", page.guard(page.showSettings));
        page.ctx.command.watch("focus-modes", page.guard(page.showModes));
        page.ctx.command.watch("config", page.guard(page.showPrefs));
        page.ctx.command.watch("rules", page.guard(page.showRules));
        historyTab.recent.start();
    }

    /// Put the two boxes back on what was last read (after a read, and after a change that failed).
    function syncBoxes() {
        overview.sync();
        historyTab.sync();
    }

    // ------------------------------------------------------------ changing

    /// Run one change. A failure shows why (under the row `noteKey`, else at the top) and puts every control back on the last
    /// value read; Cancel does nothing. `done(result)` runs after a change that worked.
    function change(command, args, doing, done, noteKey) {
        if (!page.ctx || !page.idle)
            return;
        const key = noteKey || "";
        page.problem = "";
        page.problemDetail = "";
        page.notes = Logic.withNote(page.notes, key, null);
        page.working = key !== "" ? key : command;
        page.ctx.command.run(command, args, page.guard(result => {
            page.working = "";
            if (Logic.cancelled(result)) {
                page.syncBoxes();
                return;
            }
            const why = Logic.problemOf(result, doing);
            if (why !== "") {
                if (key !== "")
                    page.notes = Logic.withNote(page.notes, key, { message: why, detail: Logic.detailOf(result) });
                else {
                    page.problem = why;
                    page.problemDetail = Logic.detailOf(result);
                }
                page.syncBoxes();
                return;
            }
            if (done)
                done(result);
        }));
    }

    /// dnd, mute, unmute and focus answer with the new state.
    function takeStatus(result) {
        const s = Logic.parseStatusDocument(result);
        if (s) {
            page.status = s;
            page.syncBoxes();
        } else {
            page.ctx.command.run("status", [], page.guard(page.showStatus));
        }
    }

    /// One setting in the rules file. The file is followed, so the page shows the new value once it is written.
    function setPref(key, text, doing) {
        const pref = page.prefs.byKey[key];
        const r = Logic.prefArgs(pref, text);
        if (r.error !== undefined) {
            page.notes = Logic.withNote(page.notes, key, { message: r.error, detail: "" });
            return false;
        }
        page.change("config-set", r.args, doing || "Couldn't change this", () => page.ctx.command.run("config", [], page.guard(page.showPrefs)), key);
        return true;
    }

    function resetPref(key) {
        const args = Logic.prefResetArgs(page.prefs.byKey[key]);
        if (args)
            page.change("config-reset", args, "Couldn't reset this", () => page.ctx.command.run("config", [], page.guard(page.showPrefs)), key);
    }

    function chooseLimit(index) {
        const v = Logic.limitValue(page.settings.limit, index);
        if (v === null) {
            page.syncBoxes();
            return;
        }
        page.setPref("history.limit", v, "Couldn't change how many notifications are kept");
    }

    function chooseFocus(index) {
        page.change("focus", [Logic.focusArg(page.focusModes.modes, index)], "Couldn't change the focus mode", page.takeStatus);
    }

    function openApp() {
        page.ctx.openApp(Logic.APP_ID, page.guard(result => {
            if (result && result.error !== undefined) {
                page.problem = "Couldn't open noti-fi.";
                page.problemDetail = Logic.detailOf(result);
            }
        }));
    }

    function pref(key) {
        return page.prefs.byKey[key] || null;
    }

    function prefOn(key, fallback) {
        const p = page.pref(key);
        return p ? p.value === true : fallback;
    }

    Component.onDestruction: page.alive = false

    headerItems: [
        QQC2.Button {
            text: "Open noti-fi"
            onClicked: page.openApp()
        }
    ]

    banners: [
        ErrorCard {
            visible: page.kind === "notRunning"
            title: "noti-fi isn't running"
            detail: page.kindMessage
            Layout.fillWidth: true
        },
        ErrorCard {
            visible: page.kind === "error"
            title: "Couldn't read noti-fi's state"
            detail: page.kindMessage
            Layout.fillWidth: true
        },
        ErrorCard {
            visible: page.ready && page.status.cfgError !== ""
            title: "noti-fi couldn't use your latest rules, so it is using the earlier ones."
            detail: page.status.cfgError
            Layout.fillWidth: true
        },
        Notice {
            summary: page.ready && !page.settings.ok ? page.settings.message : ""
            detail: page.settings.detail || ""
            color: page.errorColor()
            mutedColor: page.mutedColor()
            Layout.fillWidth: true
        },
        Notice {
            summary: page.problem
            detail: page.problemDetail
            color: page.errorColor()
            mutedColor: page.mutedColor()
            Layout.fillWidth: true
        }
    ]

    // ---------------------------------------------------------------- Overview

    OverviewTab {
        id: overview
        host: page
    }

    // ---------------------------------------------------------------- Quiet

    QuietTab {
        host: page
    }

    // ---------------------------------------------------------------- History

    HistoryTab {
        id: historyTab
        host: page
    }

    // ---------------------------------------------------------------- Rules

    SettingsTab {
        title: "Rules"
        iconName: "view-filter"

        SettingsCard {
            title: "Your rules"
            iconName: "view-list-text"
            help: "Rules decide where each app's notifications go and how they look."
            Layout.fillWidth: true

            RulesSection {
                id: rulesSection
                names: Logic.ruleNames(page.settings)
                highlight: page.highlight
                mutedColor: page.mutedColor()
                onOpenRequested: page.openApp()
                Layout.fillWidth: true
            }
        }
        SettingsCard {
            title: "Rules check"
            iconName: "dialog-ok-apply"
            help: "noti-fi checks your rules each time the file changes."
            Layout.fillWidth: true

            RulesCheck {
                host: page
                Layout.fillWidth: true
            }
        }
    }

    // ---------------------------------------------------------------- Health

    SettingsTab {
        title: "Health"
        iconName: "system-help"

        SettingsCard {
            title: "Check-up"
            iconName: "tools-check-spelling"
            help: "Looks at the tools noti-fi needs, how it is installed, and whether it is running."
            wide: true
            Layout.fillWidth: true

            CheckupSection {
                host: page
                Layout.fillWidth: true
            }
        }
        SettingsCard {
            title: "Updates"
            iconName: "system-software-update"
            help: "Looks on GitHub only when you press the button."
            wide: true
            Layout.fillWidth: true

            UpdateSection {
                host: page
                Layout.fillWidth: true
            }
        }
    }

    // ---------------------------------------------------------------- Preferences

    SettingsTab {
        title: "Preferences"
        iconName: "configure"

        PreferencesSection {
            host: page
            Layout.fillWidth: true
        }
    }
}
