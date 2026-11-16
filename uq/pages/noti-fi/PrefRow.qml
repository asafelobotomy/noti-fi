// One setting of the rules file: a switch, a choice, or a field, with the daemon's own refusal under it and a Reset that
// shows only while the file sets it. The page follows the file, so after a change the row shows what was written.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.uq.settings
import "logic.js" as Logic

ColumnLayout {
    id: row

    property var host
    /// {key, label, unit, hint} from Logic.PREF_CARDS.
    property var def
    /// The [[setting]] id this row answers Settings search with (the first row of a card).
    property string settingId: ""
    readonly property var pref: row.host ? row.host.pref(row.def.key) : null
    readonly property bool usable: !!row.pref && !!row.host && row.host.known && row.host.idle
    readonly property string shown: Logic.prefText(row.pref)

    spacing: 2

    SettingRow {
        settingId: row.settingId
        label: row.def.label + (row.def.unit ? " (" + row.def.unit + ")" : "")
        highlight: row.host ? row.host.highlight : ""
        enabled: row.usable
        Layout.fillWidth: true

        QQC2.Switch {
            visible: !!row.pref && row.pref.type === "bool"
            checked: !!row.pref && row.pref.value === true
            onToggled: {
                const want = checked;
                checked = Qt.binding(() => !!row.pref && row.pref.value === true);
                row.host.setPref(row.def.key, want ? "on" : "off");
            }
        }
        UqComboBox {
            id: choice
            visible: !!row.pref && row.pref.type === "text" && row.pref.choices.length > 0
            model: row.pref ? row.pref.choices : []
            currentIndex: row.pref ? row.pref.choices.indexOf(row.shown) : -1
            onActivated: (index) => row.host.setPref(row.def.key, row.pref.choices[index])
        }
        QQC2.TextField {
            id: field
            objectName: "field-" + row.def.key
            visible: !!row.pref && row.pref.type !== "bool" && !choice.visible
            text: row.shown
            placeholderText: Logic.prefDefaultText(row.pref)
            implicitWidth: 180
            // Show the file's value again once a change is written, or has failed.
            onEditingFinished: {
                if (field.text === row.shown)
                    return;
                if (field.text.trim() === "")
                    row.host.resetPref(row.def.key);
                else if (!row.host.setPref(row.def.key, field.text))
                    field.text = Qt.binding(() => row.shown);
            }
        }
        QQC2.Button {
            text: "Reset"
            visible: !!row.pref && row.pref.set
            onClicked: row.host.resetPref(row.def.key)
        }
    }
    UqLabel {
        visible: !!row.def.hint
        text: row.def.hint || ""
        opacity: 0.7
        wrapMode: Text.Wrap
        Layout.fillWidth: true
    }
    Notice {
        summary: row.host && row.host.noteFor(row.def.key) ? row.host.noteFor(row.def.key).message : ""
        detail: row.host && row.host.noteFor(row.def.key) ? row.host.noteFor(row.def.key).detail : ""
        color: row.host ? row.host.errorColor() : "red"
        mutedColor: row.host ? row.host.mutedColor() : "gray"
        Layout.fillWidth: true
    }
}
