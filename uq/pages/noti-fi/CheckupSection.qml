// The check-up (`notify doctor`): grouped checks with a mark each, and what to do about the ones that need a look.
// It runs the first time this tab is shown, and again when asked; the run can take a few seconds.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.uq.settings
import "logic.js" as Logic
import "reports.js" as Reports

ColumnLayout {
    id: section

    property var host
    property bool running: false
    property bool ran: false
    property var report: null
    property int generation: 0

    function run() {
        if (!section.host || !section.host.ctx || section.running)
            return;
        section.running = true;
        section.ran = true;
        section.host.ctx.command.run("doctor", [], section.host.guard(result => {
            section.running = false;
            if (Logic.overlapped(result))
                return;
            section.report = Reports.parseDoctor(result);
        }));
    }

    /// The first time the tab is shown (also when Settings opens the page on it).
    function autoload() {
        if (section.visible && !section.ran)
            section.run();
    }

    function mark(level) {
        return { ok: "✓", warn: "!", fail: "✕", info: "•" }[level] || "•";
    }

    function markColor(level) {
        if (level === "fail")
            return section.host ? section.host.errorColor() : "red";
        return level === "ok" || level === "info" ? section.host ? section.host.mutedColor() : "gray" : "orange";
    }

    onVisibleChanged: section.autoload()
    Connections {
        target: section.host
        function onCtxChanged() {
            section.autoload();
        }
    }

    spacing: 8

    SettingRow {
        settingId: "check-up"
        label: section.report ? Reports.summaryLine(section.report) : "Not run yet"
        highlight: section.host ? section.host.highlight : ""
        Layout.fillWidth: true

        QQC2.Button {
            text: section.ran ? "Run again" : "Run check-up"
            enabled: !section.running
            onClicked: section.run()
        }
    }
    QQC2.ProgressBar {
        visible: section.running
        indeterminate: true
        Layout.fillWidth: true
    }
    UqLabel {
        visible: section.running
        text: "Checking…"
        opacity: 0.7
    }
    ErrorCard {
        visible: section.report !== null && !section.report.ok
        title: "Couldn't run the check-up"
        detail: section.report ? section.report.message : ""
        Layout.fillWidth: true
    }
    Repeater {
        model: section.report && section.report.ok ? section.report.groups : []

        ColumnLayout {
            id: group

            required property var modelData

            spacing: 4
            Layout.fillWidth: true

            UqLabel {
                text: group.modelData.name
                font.bold: true
            }
            Repeater {
                model: group.modelData.checks

                RowLayout {
                    id: check

                    required property var modelData

                    spacing: 8
                    Layout.fillWidth: true

                    UqLabel {
                        text: section.mark(check.modelData.level)
                        color: section.markColor(check.modelData.level)
                        Layout.alignment: Qt.AlignTop
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true

                        UqLabel {
                            text: check.modelData.message
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                        UqLabel {
                            visible: check.modelData.hint !== ""
                            text: check.modelData.hint
                            opacity: 0.7
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }
    }
}
