// Preferences: every setting of the rules file the panel edits, in cards. The file is followed (page.toml `watch`),
// so a change made in the app or by hand shows here, and a change here shows there.
import QtQuick
import QtQuick.Layouts
import org.uq.settings
import "logic.js" as Logic

ColumnLayout {
    id: section

    property var host

    spacing: 16

    ErrorCard {
        title: "Couldn't read noti-fi's preferences"
        detail: section.host ? section.host.prefs.detail || section.host.prefs.message : ""
        visible: !!section.host && !section.host.prefs.ok && section.host.openedWith !== null
        Layout.fillWidth: true
    }
    Repeater {
        model: Logic.PREF_CARDS

        SettingsCard {
            id: card

            required property var modelData

            title: card.modelData.title
            iconName: card.modelData.icon
            help: card.modelData.help
            wide: true
            Layout.fillWidth: true

            Repeater {
                model: card.modelData.rows

                PrefRow {
                    required property var modelData
                    required property int index
                    host: section.host
                    settingId: index === 0 ? card.modelData.id : ""
                    def: modelData
                    Layout.fillWidth: true
                }
            }
        }
    }
}
