// The recent notifications: who sent each, which group it went to, when, and why it was held back (never the text).
// Read when the page opens and again after a clear; the filter shows all, or only what was held back.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.uq.settings
import "logic.js" as Logic

ColumnLayout {
    id: section

    property var host
    property bool loading: false
    property bool loaded: false
    property string problem: ""
    property var rows: []
    /// Bumped by each read, so an answer that arrives after a newer read started is dropped.
    property int generation: 0
    /// The number of notifications the daemon said it held when this was last read.
    property int seenCount: -1

    function show(generation, result) {
        if (generation !== section.generation || (result && result.error === "already running"))
            return;
        section.loading = false;
        const h = Logic.parseHistory(result, new Date());
        section.loaded = h.ok;
        section.problem = h.ok ? "" : h.message;
        section.rows = h.rows;
    }

    function refresh() {
        if (!section.host || !section.host.ctx)
            return;
        section.generation += 1;
        section.seenCount = section.host.status.history;
        const mine = section.generation;
        section.loading = true;
        section.host.ctx.command.run(filter.currentIndex === 1 ? "history-held" : "history", [], section.host.guard(r => section.show(mine, r)));
    }

    /// Reads once the daemon answers, and again whenever the number of notifications kept changes.
    function start() {
        section.refresh();
    }

    Connections {
        target: section.host
        function onStatusChanged() {
            if (section.host.ready && section.host.status.history !== section.seenCount && !section.loading)
                section.refresh();
        }
    }

    spacing: 8

    SettingRow {
        settingId: "recent"
        label: "Show"
        highlight: section.host ? section.host.highlight : ""
        Layout.fillWidth: true

        UqComboBox {
            id: filter
            objectName: "filterBox"
            model: ["All", "Held back"]
            onActivated: section.refresh()
        }
        QQC2.Button {
            text: "Refresh"
            enabled: !section.loading
            onClicked: section.refresh()
        }
    }
    QQC2.ProgressBar {
        visible: section.loading
        indeterminate: true
        Layout.fillWidth: true
    }
    Notice {
        summary: section.problem !== "" ? "Couldn't read the history." : ""
        detail: section.problem
        color: section.host ? section.host.errorColor() : "red"
        mutedColor: section.host ? section.host.mutedColor() : "gray"
        Layout.fillWidth: true
    }
    UqLabel {
        visible: section.loaded && section.rows.length === 0
        text: filter.currentIndex === 1 ? "Nothing was held back." : "No notifications yet."
        opacity: 0.7
    }
    Notice {
        summary: section.host && section.host.noteFor("history-clear") ? section.host.noteFor("history-clear").message : ""
        detail: section.host && section.host.noteFor("history-clear") ? section.host.noteFor("history-clear").detail : ""
        color: section.host ? section.host.errorColor() : "red"
        mutedColor: section.host ? section.host.mutedColor() : "gray"
        Layout.fillWidth: true
    }
    Repeater {
        model: section.rows

        RowLayout {
            id: row

            required property var modelData

            spacing: 12
            Layout.fillWidth: true

            UqAppIcon {
                name: row.modelData.icon
                label: row.modelData.app
                size: 24
            }
            ColumnLayout {
                spacing: 0
                Layout.fillWidth: true

                UqLabel {
                    text: row.modelData.app
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                UqLabel {
                    text: row.modelData.group + (row.modelData.shown ? "" : " · " + row.modelData.why)
                    visible: text !== ""
                    opacity: 0.7
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }
            UqLabel {
                text: row.modelData.time
                opacity: 0.7
            }
        }
    }
}
