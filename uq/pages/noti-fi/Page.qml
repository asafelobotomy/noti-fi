// noti-fi's page in uQ Settings. `ctx` is set by Settings (docs/settings-pages.md in the uQ repository). All text goes
// through org.uq.settings components (plain text); parsing and the rules for what may be sent live in logic.js.
// Notification text (summaries, bodies) is never read or shown here.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.uq.settings
import "logic.js" as Logic

Item {
    id: page

    property var ctx
    /// The [[setting]] row Settings search jumped to (the rows flash by themselves; the page scrolls to it).
    property string highlight

    /// loading | ready | notRunning | error
    property string kind: "loading"
    property string kindMessage
    property var status: ({ dnd: false, muted: [], zones: [], focus: "", history: 0, cfgError: "" })
    property var settings: ({ ok: false, message: "", zones: [], rules: [] })
    property var focusModes: ({ ok: false, active: "", modes: [] })
    /// Why the last change failed (or the app couldn't open), shown until the next change.
    property string problem
    /// The program's own words for it, under "Advanced details".
    property string problemDetail
    /// The command whose change is running ("" = none). One change at a time.
    property string working

    readonly property bool ready: page.kind === "ready"
    readonly property bool idle: page.working === ""
    readonly property bool known: page.ready && page.settings.ok
    readonly property color errorColor: (page.ctx && page.ctx.theme && page.ctx.theme.colors && page.ctx.theme.colors.error) || "red"
    readonly property color mutedColor: (page.ctx && page.ctx.theme && page.ctx.theme.colors && page.ctx.theme.colors.text_muted) || "gray"

    // ------------------------------------------------------------ reading

    function showStatus(result) {
        const s = Logic.parseStatus(result);
        page.kind = s.kind;
        page.kindMessage = s.message;
        if (s.kind === "ready")
            page.status = s;
        page.syncBoxes();
    }

    function showSettings(result) {
        page.settings = Logic.parseSettings(result);
        page.syncBoxes();
    }

    function showModes(result) {
        page.focusModes = Logic.parseFocusModes(result);
        page.syncBoxes();
    }

    // Read now, and again whenever the daemon saves its state or the rules file changes (page.toml `watch`).
    onCtxChanged: {
        if (!page.ctx)
            return;
        page.ctx.command.watch("status", page.showStatus);
        page.ctx.command.watch("settings", page.showSettings);
        page.ctx.command.watch("focus-modes", page.showModes);
    }

    /// Put the two boxes back on what was last read (after a read, and after a change that failed).
    function syncBoxes() {
        limitBox.currentIndex = Logic.limitIndex(page.settings.limit);
        focusBox.currentIndex = Logic.focusIndex(page.focusModes.modes, page.status.focus);
    }

    // ------------------------------------------------------------ changing

    /// Run one change. A failure shows why and puts every control back on the last value read; Cancel does nothing.
    function change(command, args, doing, done) {
        if (!page.ctx || !page.idle)
            return;
        page.problem = "";
        page.problemDetail = "";
        page.working = command;
        page.ctx.command.run(command, args, result => {
            page.working = "";
            if (Logic.cancelled(result)) {
                page.syncBoxes();
                return;
            }
            const why = Logic.problemOf(result, doing);
            if (why !== "") {
                page.problem = why;
                page.problemDetail = Logic.detailOf(result);
                page.syncBoxes();
                return;
            }
            done(result);
        });
    }

    /// dnd, mute, unmute and focus answer with the new state.
    function takeStatus(result) {
        const s = Logic.parseStatusDocument(result);
        if (s) {
            page.status = s;
            page.syncBoxes();
        } else {
            page.ctx.command.run("status", [], page.showStatus);
        }
    }

    function setSetting(args, doing) {
        if (!args) {
            page.syncBoxes();
            return;
        }
        page.change("set", args, doing, () => page.ctx.command.run("settings", [], page.showSettings));
    }

    function chooseLimit(index) {
        page.setSetting(Logic.limitArgs(page.settings.limit, index), "Couldn't change how many notifications are kept");
    }

    function chooseFocus(index) {
        page.change("focus", [Logic.focusArg(page.focusModes.modes, index)], "Couldn't change the focus mode", page.takeStatus);
    }

    function openApp() {
        page.ctx.openApp(Logic.APP_ID, result => {
            if (result && result.error !== undefined) {
                page.problem = "Couldn't open noti-fi.";
                page.problemDetail = Logic.detailOf(result);
            }
        });
    }

    // ------------------------------------------------------------ scrolling to a search result

    function scrollTo(id) {
        const target = { "dnd": dndRow, "focus": focusRow, "groups": groupsRow, "auto-dnd": autoRow, "history": historyRow, "rules": rulesRow }[id];
        if (!target)
            return;
        const y = target.mapToItem(column, 0, 0).y;
        flick.contentY = Math.max(0, Math.min(y - 8, flick.contentHeight - flick.height));
    }

    onHighlightChanged: Qt.callLater(() => page.scrollTo(page.highlight))
    Component.onCompleted: {
        if (page.highlight !== "")
            Qt.callLater(() => page.scrollTo(page.highlight));
    }

    Flickable {
        id: flick
        anchors.fill: parent
        contentWidth: width
        contentHeight: column.implicitHeight + 32
        clip: true
        QQC2.ScrollBar.vertical: QQC2.ScrollBar {}

        ColumnLayout {
            id: column
            x: 16
            y: 16
            width: flick.width - 32
            spacing: 8

            UqLabel {
                visible: page.kind === "loading"
                text: "Checking…"
                Layout.fillWidth: true
            }
            UqLabel {
                visible: page.kind === "notRunning"
                text: page.kindMessage
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            ErrorCard {
                visible: page.kind === "error"
                title: "Couldn't read noti-fi's state"
                detail: page.kindMessage
                Layout.fillWidth: true
            }
            ErrorCard {
                visible: page.ready && page.status.cfgError !== ""
                title: "noti-fi couldn't use your latest rules, so it is using the earlier ones."
                detail: page.status.cfgError
                Layout.fillWidth: true
            }
            Notice {
                summary: page.ready && !page.settings.ok ? page.settings.message : ""
                detail: page.settings.detail || ""
                color: page.errorColor
                mutedColor: page.mutedColor
                Layout.fillWidth: true
            }
            Notice {
                summary: page.problem
                detail: page.problemDetail
                color: page.errorColor
                mutedColor: page.mutedColor
                Layout.fillWidth: true
            }
            UqLabel {
                visible: !page.idle
                text: "Working…"
                color: page.mutedColor
            }

            ColumnLayout {
                visible: page.ready
                spacing: 8
                Layout.fillWidth: true

                // ---- Do not disturb
                SwitchRow {
                    id: dndRow
                    settingId: "dnd"
                    label: "Do not disturb"
                    highlight: page.highlight
                    value: page.status.dnd
                    enabled: page.idle
                    onChosen: (isOn) => page.change("dnd", Logic.dndArgs(isOn), "Couldn't change Do not disturb", page.takeStatus)
                    Layout.fillWidth: true
                }

                // ---- Focus mode
                SettingRow {
                    id: focusRow
                    settingId: "focus"
                    label: "Focus mode"
                    highlight: page.highlight
                    enabled: page.idle
                    Layout.fillWidth: true

                    UqComboBox {
                        id: focusBox
                        objectName: "focusBox"
                        model: Logic.focusTexts(page.focusModes.modes)
                        onActivated: (index) => page.chooseFocus(index)
                    }
                }
                UqLabel {
                    text: "Only important notifications show while a focus mode is on."
                    color: page.mutedColor
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }

                // ---- Groups
                SettingRow {
                    id: groupsRow
                    settingId: "groups"
                    label: "Groups"
                    highlight: page.highlight
                    Layout.fillWidth: true
                }
                UqLabel {
                    text: "Mute a group to keep its notifications out of sight. They still go to the history."
                    color: page.mutedColor
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                Repeater {
                    model: Logic.groupRows(page.status, page.settings)

                    SwitchRow {
                        required property var modelData
                        label: "Mute " + modelData.label
                        value: modelData.muted
                        enabled: page.idle && modelData.changeable
                        onChosen: (isOn) => page.change(Logic.muteCommand(isOn), [modelData.id], "Couldn't change " + modelData.label, page.takeStatus)
                        Layout.fillWidth: true
                    }
                }

                // ---- Automatic Do not disturb
                SettingRow {
                    id: autoRow
                    settingId: "auto-dnd"
                    label: "Automatic Do not disturb"
                    highlight: page.highlight
                    Layout.fillWidth: true
                }
                SwitchRow {
                    label: "When an app is full screen"
                    value: page.settings.ok && page.settings.fullscreen
                    enabled: page.known && page.idle
                    onChosen: (isOn) => page.setSetting(Logic.setArgs("autoDnd.fullscreen", Logic.boolWord(isOn)), "Couldn't change this")
                    Layout.fillWidth: true
                }
                SwitchRow {
                    label: "When you share your screen"
                    value: page.settings.ok && page.settings.screenShare
                    enabled: page.known && page.idle
                    onChosen: (isOn) => page.setSetting(Logic.setArgs("autoDnd.screenShare", Logic.boolWord(isOn)), "Couldn't change this")
                    Layout.fillWidth: true
                }

                // ---- History
                SettingRow {
                    id: historyRow
                    settingId: "history"
                    label: "History"
                    highlight: page.highlight
                    Layout.fillWidth: true
                }
                SettingRow {
                    label: "Remember at most"
                    enabled: page.known && page.idle
                    Layout.fillWidth: true

                    UqComboBox {
                        id: limitBox
                        objectName: "limitBox"
                        model: Logic.limitTexts(page.settings.limit)
                        onActivated: (index) => page.chooseLimit(index)
                    }
                }
                SwitchRow {
                    label: "Keep after restart"
                    value: page.settings.ok && page.settings.persist
                    enabled: page.known && page.idle
                    onChosen: (isOn) => page.setSetting(Logic.setArgs("history.persist", Logic.boolWord(isOn)), "Couldn't change this")
                    Layout.fillWidth: true
                }
                UqLabel {
                    text: "Turning this on saves the text of notifications to a file on this computer."
                    color: page.mutedColor
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                SwitchRow {
                    label: "Let urgent notifications through Do not disturb"
                    value: page.settings.ok && page.settings.allowCritical
                    enabled: page.known && page.idle
                    onChosen: (isOn) => page.setSetting(Logic.setArgs("dnd.allowCritical", Logic.boolWord(isOn)), "Couldn't change this")
                    Layout.fillWidth: true
                }
                RowLayout {
                    Layout.fillWidth: true

                    UqLabel {
                        text: Logic.historyLine(page.status.history)
                        Layout.fillWidth: true
                    }
                    QQC2.Button {
                        text: "Clear history…"
                        enabled: page.idle
                        // Settings asks "Clear the notification history? This can't be undone." first; this runs only on Confirm.
                        onClicked: page.change("history-clear", [], "Couldn't clear the history", () => page.ctx.command.run("status", [], page.showStatus))
                    }
                }

                // ---- Your rules (read only)
                SettingRow {
                    id: rulesRow
                    settingId: "rules"
                    label: "Your rules"
                    highlight: page.highlight
                    Layout.fillWidth: true
                }
                Repeater {
                    model: Logic.ruleNames(page.settings)

                    UqLabel {
                        required property string modelData
                        text: modelData
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }
                UqLabel {
                    text: "Change your rules in noti-fi's own app."
                    color: page.mutedColor
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                QQC2.Button {
                    text: "Open noti-fi"
                    onClicked: page.openApp()
                }
            }
        }
    }
}
