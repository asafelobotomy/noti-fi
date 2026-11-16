// Updates: look for a newer release, then install it with a progress bar fed by the command's own steps (`ctx.command.stream`).
// Settings shows what the update would replace, and whether it would refuse (a package install says to use the package tool), first.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.uq.settings
import "logic.js" as Logic

ColumnLayout {
    id: section

    property var host
    property bool checking: false
    property var info: null
    property bool installing: false
    property real fraction: 0
    property string stepText: ""
    property var last: null
    property var outcome: null

    function check() {
        if (!section.host || !section.host.ctx || section.checking)
            return;
        section.checking = true;
        section.outcome = null;
        section.host.ctx.command.run("update-check", [], section.host.guard(result => {
            section.checking = false;
            section.info = Logic.parseUpdate(result);
        }));
    }

    function install() {
        if (!section.host || !section.host.ctx || section.installing)
            return;
        section.installing = true;
        section.fraction = 0;
        section.stepText = "";
        section.last = null;
        section.outcome = null;
        section.host.ctx.command.stream("update", [], section.host.guard(ev => {
            const p = Logic.progressOf(ev);
            if (p) {
                section.fraction = p.fraction;
                section.stepText = p.text;
            } else if (ev && ev.type === "result") {
                section.last = ev;
            }
        }), section.host.guard(done => {
            section.installing = false;
            // Cancel on the confirmation sheet changes nothing and says nothing.
            if (Logic.cancelled(done))
                return;
            section.outcome = Logic.updateOutcome(section.last, done);
            if (section.outcome.ok)
                section.info = null;
        }));
    }

    spacing: 8

    SettingRow {
        settingId: "updates"
        label: section.info && section.info.ok ? (section.info.newer ? "noti-fi " + section.info.latest + " is available (you have " + section.info.current + ")" : "noti-fi " + section.info.current + " is up to date") : "Version"
        highlight: section.host ? section.host.highlight : ""
        Layout.fillWidth: true

        QQC2.Button {
            text: "Check for updates"
            enabled: !section.checking && !section.installing
            onClicked: section.check()
        }
        QQC2.Button {
            text: "Update…"
            visible: !!section.info && section.info.ok && section.info.newer
            enabled: !section.installing
            onClicked: section.install()
        }
    }
    QQC2.ProgressBar {
        visible: section.checking
        indeterminate: true
        Layout.fillWidth: true
    }
    QQC2.ProgressBar {
        visible: section.installing
        value: section.fraction
        Layout.fillWidth: true
    }
    UqLabel {
        visible: section.installing && section.stepText !== ""
        text: section.stepText
        opacity: 0.7
        wrapMode: Text.Wrap
        Layout.fillWidth: true
    }
    Notice {
        summary: section.info && !section.info.ok ? section.info.message : ""
        color: section.host ? section.host.errorColor() : "red"
        mutedColor: section.host ? section.host.mutedColor() : "gray"
        Layout.fillWidth: true
    }
    Notice {
        summary: section.outcome ? section.outcome.message : ""
        detail: section.outcome ? section.outcome.detail : ""
        color: section.outcome && section.outcome.ok ? (section.host ? section.host.mutedColor() : "gray") : (section.host ? section.host.errorColor() : "red")
        mutedColor: section.host ? section.host.mutedColor() : "gray"
        Layout.fillWidth: true
    }
}
