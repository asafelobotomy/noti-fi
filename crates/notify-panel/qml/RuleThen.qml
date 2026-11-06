import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// "Then": what the selected rule does: the group it puts notifications in (with a mini screen of where that is), how long they stay,
// and whether they are shown at all.
ColumnLayout {
    id: root
    property var page
    Layout.alignment: Qt.AlignTop
    Layout.fillWidth: true
    Layout.preferredWidth: 1
    spacing: Kirigami.Units.smallSpacing
    Kirigami.Heading { level: 3; text: "Then" }
    QQC2.Label { text: "Put in group"; font.bold: true }
    // a rule for a group changes how that group's notifications look and behave; it cannot move them to another
    QQC2.Label {
        visible: page.current !== null && page.current.mGroup !== ""
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        opacity: 0.7
        text: "This rule applies to a group, so it keeps them in that group."
    }
    RowLayout {
      visible: !(page.current !== null && page.current.mGroup !== "")
      Layout.fillWidth: true
      spacing: Kirigami.Units.largeSpacing
      MiniScreen {
        readonly property var zn: page.current ? page.zoneInfo(page.current.zone) : null
        dim: zn === null
        anchorName: zn ? zn.anchor : "top-right"
        fx: zn ? Number(zn.x) : 0.5; fy: zn ? Number(zn.y) : 0.1
        accent: zn && zn.accent ? zn.accent : ""
        Accessible.name: "Where this rule puts popups"
      }
      QQC2.ComboBox {
        id: zoneBox
        Layout.fillWidth: true
        model: page.zoneChoiceLabels
    // shown from the rule itself: a list refresh can reset a combo box's index
    displayText: page.current && page.current.zone !== "" ? page.zoneLabel(page.current.zone) : page.zoneChoiceLabels[0]
        currentIndex: page.current && page.current.zone !== "" ? Math.max(0, page.zoneChoices.indexOf(page.current.zone)) : 0
        onActivated: index => page.model.ruleSetZone(page.selected, index === 0 ? "" : page.zoneChoices[index])
        Accessible.name: "Put in group"
      }
    }
    QQC2.Label { Layout.topMargin: Kirigami.Units.smallSpacing; text: "Stays on screen"; font.bold: true }
    DurationChips {
        Layout.fillWidth: true
        unsetLabel: "Not set here"
        unsetTip: "Later rules, the app or the default decide"
        value: page.current ? page.current.timeout : ""
        onEdited: text => page.model.ruleSetTimeout(page.selected, text)
    }
    QQC2.CheckBox {
        Layout.topMargin: Kirigami.Units.smallSpacing
        text: "Never show it, keep it in history only"
        checked: page.current ? page.current.silent : false
        onToggled: {
            page.model.ruleSetSilent(page.selected, checked)
            checked = Qt.binding(() => page.current ? page.current.silent : false)
        }
        Accessible.name: "Never show, keep in history only"
    }
}
