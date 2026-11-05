import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// The messages shared by every page that edits the draft: invalid, edited-elsewhere, save result, and the conflict dialog.
ColumnLayout {
    id: root
    property var model
    spacing: 0
    Layout.fillHeight: false      // a nested layout would otherwise take a share of the page's spare height
    readonly property string errorText: error.visible ? error.text : ""
    readonly property string infoText: external.visible ? external.text : ""
    readonly property string saveText: saved.visible ? saved.text : ""
    readonly property bool conflictVisible: conflict.visible

    Kirigami.InlineMessage {
        id: error
        Layout.fillWidth: true
        Layout.margins: visible ? Kirigami.Units.smallSpacing : 0
        visible: root.model !== undefined && root.model !== null && !root.model.draftValid && !root.model.checking && root.model.draftError !== ""
        type: Kirigami.MessageType.Error
        text: root.model ? "Cannot save: " + root.model.draftError : ""
        showCloseButton: false
    }
    Kirigami.InlineMessage {
        id: external
        Layout.fillWidth: true
        Layout.margins: visible ? Kirigami.Units.smallSpacing : 0
        visible: root.model !== undefined && root.model !== null && root.model.externalChanged
        type: Kirigami.MessageType.Information
        text: "rules.json was edited outside this panel. Saving will ask before overwriting it."
        showCloseButton: false
        actions: [ Kirigami.Action { text: "Reload"; onTriggered: root.model.rulesReload() } ]
    }
    Kirigami.InlineMessage {
        id: saved
        Layout.fillWidth: true
        Layout.margins: visible ? Kirigami.Units.smallSpacing : 0
        visible: root.model !== undefined && root.model !== null && root.model.saveMessage !== ""
        type: root.model && root.model.conflict ? Kirigami.MessageType.Warning : Kirigami.MessageType.Positive
        text: root.model ? root.model.saveMessage : ""
    }
    Kirigami.PromptDialog {
        id: conflict
        title: "rules.json changed on disk"
        subtitle: "Someone edited the file after you started. Overwrite it with your version, or reload it and lose your edits? (A .bak copy of the file on disk is kept when you overwrite.)"
        standardButtons: Kirigami.Dialog.NoButton
        visible: root.model ? root.model.conflict : false
        showCloseButton: true
        customFooterActions: [
            Kirigami.Action { text: "Overwrite with my version"; icon.name: "document-save"; onTriggered: root.model.rulesSave(true) },
            Kirigami.Action { text: "Reload from disk"; icon.name: "view-refresh"; onTriggered: root.model.rulesReload() }
        ]
        onClosed: if (root.model && root.model.conflict) root.model.rulesReload()
    }
}
