import QtQuick

// The headless test hooks: `--selftest` and the `--selftest-*` variants drive the panel's commands the way the UI would, then print
// a summary line the smoke tests read. Nothing here runs without those flags. (Hooks that write need `--selftest-save`.)
Item {
    id: root
    property var model
    property var args: []
    // These hooks change daemon state (Do Not Disturb, mutes, history) and can write rules.json, so they refuse to run unless the process is
    // in test mode (NOTIFY_PANEL_TEST=1); writing the rules file also needs NOTIFD_RULES to point somewhere other than your real one.
    readonly property bool wanted: args.some(a => a.indexOf("--selftest") === 0)
    readonly property bool allowed: wanted && model !== undefined && model !== null && model.testMode
    readonly property bool saveAllowed: allowed && args.indexOf("--selftest-save") >= 0 && model.rulesOverridden
    Timer {
        running: root.wanted && !root.allowed
        interval: 100
        onTriggered: { console.error("SELFTEST refused: it changes your real settings. Run it with NOTIFY_PANEL_TEST=1 (and NOTIFD_RULES=<a copy> to save)."); Qt.quit() }
    }
    property int failures: 0
    property int toasts: 0

    // Calls every invokable the UI uses. A missing one (cxx-qt renames silently) is counted, not ignored.
    function call(name, ...params) {
        if (typeof model[name] !== "function") {
            console.error("SELFTEST: PanelModel has no method '" + name + "'")
            failures++
            return
        }
        model[name](...params)
    }
    Connections {
        target: model
        function onCommandFailed(message) { toasts++ }
    }
    Timer {
        running: root.allowed && args.indexOf("--selftest") >= 0
        interval: 1200
        repeat: true
        property int step: 0
        onTriggered: {
            step++
            if (step === 1) call("setDoNotDisturb", true)
            else if (step === 2) call("setZoneMuted", "business", true)
            else if (step === 3) call("setZoneMuted", "business", false)
            else if (step === 4) call("setDoNotDisturb", false)
            else if (step === 5) call("setHistoryFilter", "steam", false)
            else if (step === 6) {
                console.log("SELFTEST filtered=" + model.historyShown + "/" + model.historyTotal)
                call("setZoneMuted", "nowhere", true)                         // the daemon refuses: a toast is expected
            }
            else if (step === 7) call("clearHistory")
            else {
                console.log("SELFTEST failures=" + failures + " toasts=" + toasts)
                Qt.quit()
            }
        }
    }

    // --selftest-rules: drives every rules invokable against the daemon (it never saves unless --selftest-save is given)
    readonly property bool selftestRules: args.indexOf("--selftest-rules") >= 0
    Timer {
        running: root.allowed && args.indexOf("--selftest-rules") >= 0
        interval: 900
        repeat: true
        property int step: 0
        onTriggered: {
            step++
            const m = model
            if (step === 1) call("simSet", "appName", "Brave Origin")
            else if (step === 2) call("simSet", "origin", "onlyfans.com")
            else if (step === 3) call("ruleAdd")
            else if (step === 4) call("ruleSetName", m.rulesCount - 1, "Selftest rule")
            else if (step === 5) call("ruleSetMatch", m.rulesCount - 1, "appName", "^selftest$")
            else if (step === 6) call("ruleSetZone", m.rulesCount - 1, "business")
            else if (step === 7) call("ruleSetTimeout", m.rulesCount - 1, "4000")
            else if (step === 8) call("ruleSetSilent", m.rulesCount - 1, false)
            else if (step === 9) call("simSet", "appName", "selftest")
            else if (step === 10) call("ruleDuplicate", m.rulesCount - 1)
            else if (step === 11) call("ruleMove", m.rulesCount - 1, -1)
            else if (step === 12) call("ruleSetMatch", 0, "appName", "(")      // invalid regex: the panel must refuse to save
            else if (step === 13) call("ruleSetMatch", 0, "appName", "")
            else if (step === 14) call("rulesPreview")
            else if (step === 15) call("rulesEndPreview")
            else if (step === 16) { if (root.saveAllowed) call("rulesSave", false) }
            else if (step === 17) call("ruleDelete", m.rulesCount - 1)
            else if (step === 18) call("rulesReload")
            else {
                console.log("SELFTEST-RULES failures=" + failures + " rules=" + m.rulesCount + " dirty=" + m.dirty + " valid=" + m.draftValid)
                Qt.quit()
            }
        }
    }

    // --selftest-zones: drives the zone and appearance commands (writes only with --selftest-save)
    readonly property bool selftestZones: args.indexOf("--selftest-zones") >= 0
    Timer {
        running: root.allowed && args.indexOf("--selftest-zones") >= 0
        interval: 900
        repeat: true
        property int step: 0
        property string zone: ""
        onTriggered: {
            step++
            if (step === 1) call("zoneAdd")
            else if (step === 2) {
                const zs = JSON.parse(model.settingsJson).zones
                zone = zs[zs.length - 1].id
                call("zoneSet", zone, "label", "Selftest zone")
            }
            else if (step === 3) call("zoneSet", zone, "anchor", "bottom-center")
            else if (step === 4) { call("zoneSet", zone, "accent", "#ff8800"); call("zonePlace", zone, 0.25, 0.75) }
            else if (step === 5) call("settingSet", "theme.card", "#101010")
            else if (step === 6) call("settingSet", "maxVisible", "4")
            else if (step === 7) call("settingSet", "sound.enabled", "false")
            else if (step === 8) call("settingSet", "maxVisible", "lots")        // refused: a toast is expected
            else if (step === 9) call("zoneDelete", "personal")                 // the default zone: refused, a toast is expected
            else if (step === 10) call("rulesPreview")
            else if (step === 11) call("rulesEndPreview")
            else if (step === 12) { if (root.saveAllowed) call("rulesSave", false) }
            else if (step === 13) call("zoneDelete", zone)
            else if (step === 14) call("rulesReload")
            else {
                console.log("SELFTEST-ZONES failures=" + failures + " toasts=" + toasts + " dirty=" + model.dirty + " valid=" + model.draftValid)
                Qt.quit()
            }
        }
    }

    // --selftest-apps: drives the app commands (writes only with --selftest-save). Needs an app named "Appstest" to have sent a notification.
    readonly property bool selftestApps: args.indexOf("--selftest-apps") >= 0
    Timer {
        running: root.allowed && args.indexOf("--selftest-apps") >= 0
        interval: 900
        repeat: true
        property int step: 0
        onTriggered: {
            step++
            if (step === 1) call("appsRescan")
            else if (step === 2) call("appSet", "appName", "Appstest", "Appstest", "muted", "true")
            else if (step === 3) call("appSet", "appName", "Appstest", "Appstest", "zone", "business")
            else if (step === 4) call("appSet", "appName", "Appstest", "Appstest", "timeout", "2500")
            else if (step === 5) call("appSet", "appName", "Appstest", "Appstest", "muted", "maybe")     // refused: a toast is expected
            else if (step === 6) call("appFlatpak", "--help", "deny")                                      // refused: a toast is expected
            else if (step === 7) call("appFlatpak", "org.example.App", "sideways")                         // refused: a toast is expected
            else if (step === 8) { if (root.saveAllowed) call("rulesSave", false) }
            else if (step === 9) call("appsRescan")
            else {
                console.log("SELFTEST-APPS failures=" + failures + " toasts=" + toasts + " dirty=" + model.dirty + " valid=" + model.draftValid)
                Qt.quit()
            }
        }
    }

    // --selftest-style: drives the look-and-behaviour commands (writes only with --selftest-save)
    readonly property bool selftestStyle: args.indexOf("--selftest-style") >= 0
    Timer {
        running: root.allowed && args.indexOf("--selftest-style") >= 0
        interval: 900
        repeat: true
        property int step: 0
        onTriggered: {
            step++
            if (step === 1) call("ruleSetStyle", 0, "opacity", "0.6")
            else if (step === 2) call("ruleSetStyle", 0, "clickThrough", "true")
            else if (step === 3) call("ruleSetStyle", 0, "noFocus", "false")
            else if (step === 4) call("ruleSetStyle", 0, "width", "480")
            else if (step === 5) call("ruleSetStyle", 0, "fontFamily", "Serif")
            else if (step === 6) call("ruleSetStyle", 0, "group", "Sites")
            else if (step === 7) call("zoneStyle", "personal", "opacity", "0.5")
            else if (step === 8) call("settingSet", "theme.opacity", "0.9")
            else if (step === 9) call("settingSet", "theme.fontSize", "14")
            else if (step === 10) call("appSet", "appName", "Styleapp", "Styleapp", "fontSize", "16")
            else if (step === 11) call("appSetKind", JSON.stringify([["appName", "Styleapp"], ["category", "email.arrived"], ["channel", "app"]]), "Styleapp mail", "group", "Styleapp mail")
            else if (step === 12) call("ruleSetStyle", 0, "flavour", "x")               // refused: a toast is expected
            else if (step === 13) call("zoneStyle", "personal", "group", "x")           // a zone has no group: a toast is expected
            else if (step === 14) call("appSetKind", "not json", "x", "muted", "true")  // refused: a toast is expected
            else if (step === 15) { if (root.saveAllowed) call("rulesSave", false) }
            else {
                console.log("SELFTEST-STYLE failures=" + failures + " toasts=" + toasts + " dirty=" + model.dirty + " valid=" + model.draftValid)
                Qt.quit()
            }
        }
    }

    // --selftest-focus: focus modes and the priority / stack / media settings (writes only with --selftest-save; leaves focus "urgent" on)
    readonly property bool selftestFocus: args.indexOf("--selftest-focus") >= 0
    Timer {
        running: root.allowed && args.indexOf("--selftest-focus") >= 0
        interval: 900
        repeat: true
        property int step: 0
        property string mode: ""
        onTriggered: {
            step++
            if (step === 1) call("setFocus", "urgent")
            else if (step === 2) call("setFocus", "ghost")                        // refused by the daemon: a toast is expected
            else if (step === 3) { if (typeof model.focusAdd === "function") mode = model.focusAdd(); else failures++ }
            else if (step === 4) call("focusSet", mode, "label", "Work")
            else if (step === 5) call("focusSet", mode, "minPriority", "high")
            else if (step === 6) call("focusSet", mode, "allow", "^Signal$")
            else if (step === 7) call("focusSet", mode, "minPriority", "mega")    // refused: a toast is expected
            else if (step === 8) call("focusSet", mode, "allow", "appName:")      // refused: a toast is expected
            else if (step === 9) call("settingSet", "transfer.chart", "heatmap")
            else if (step === 10) call("ruleSetStyle", 0, "priority", "urgent")
            else if (step === 11) call("ruleSetStyle", 0, "stack", "true")
            else if (step === 12) call("ruleSetStyle", 0, "media", "false")
            else if (step === 13) { if (root.saveAllowed) call("rulesSave", false) }
            else {
                console.log("SELFTEST-FOCUS failures=" + failures + " toasts=" + toasts + " dirty=" + model.dirty + " valid=" + model.draftValid + " mode=" + mode)
                Qt.quit()
            }
        }
    }

    // --selftest-autostart: turn start-at-login on and off (the files go to this run's own config folder)
    Timer {
        running: root.allowed && args.indexOf("--selftest-autostart") >= 0
        interval: 700
        repeat: true
        property int step: 0
        onTriggered: {
            step++
            if (step === 1) call("setAutostartEnabled", true)
            else if (step === 2) console.log("SELFTEST-AUTOSTART on=" + model.autostart)
            else if (step === 3) call("setAutostartEnabled", false)
            else { console.log("SELFTEST-AUTOSTART off=" + !model.autostart + " failures=" + failures); Qt.quit() }
        }
    }
}
