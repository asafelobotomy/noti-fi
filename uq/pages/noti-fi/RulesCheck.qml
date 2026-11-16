// The verdict on your rules (`notify rules`): each error and warning in the program's own words, or a plain "no problems".
import QtQuick
import QtQuick.Layouts
import org.uq.settings

ColumnLayout {
    id: section

    property var host
    readonly property var check: section.host ? section.host.rulesCheck : null

    spacing: 8

    SettingRow {
        settingId: "rules-check"
        label: "Rules check"
        highlight: section.host ? section.host.highlight : ""
        Layout.fillWidth: true
    }
    ErrorCard {
        visible: section.check !== null && !section.check.ok && section.host.ready
        title: "Couldn't check your rules"
        detail: section.check ? section.check.message : ""
        Layout.fillWidth: true
    }
    UqLabel {
        visible: section.check !== null && section.check.ok && section.check.errors.length === 0 && section.check.warnings.length === 0
        text: "No problems found."
    }
    Repeater {
        model: section.check ? section.check.errors : []

        ErrorCard {
            required property string modelData
            title: "A rule has an error"
            detail: modelData
            Layout.fillWidth: true
        }
    }
    Repeater {
        model: section.check ? section.check.warnings : []

        UqLabel {
            required property string modelData
            text: modelData
            wrapMode: Text.Wrap
            opacity: 0.8
            Layout.fillWidth: true
        }
    }
}
