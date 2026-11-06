import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// An event sound chosen from the installed sound themes, with ▶ to hear it first. "" = the default notification sound.
RowLayout {
    id: root
    property var model                       // the panel model (sound list, playing)
    property string value: ""
    property string unsetLabel: "Default sound"
    signal edited(string text)
    readonly property var names: { try { return JSON.parse(root.model ? root.model.soundNames() : "[]") } catch (e) { return [] } }
    // the usual notification sounds first, then the rest
    readonly property var preferred: ["message-new-instant", "message-new-email", "message", "dialog-information", "bell", "complete", "phone-incoming-call"]
    readonly property var choices: [root.unsetLabel].concat(root.preferred.filter(n => root.names.indexOf(n) >= 0), root.names.filter(n => root.preferred.indexOf(n) < 0))
    function pretty(n) { return n === root.unsetLabel ? n : n.replace(/[-_]/g, " ").replace(/^./, c => c.toUpperCase()) }

    QQC2.ComboBox {
        id: box
        Layout.preferredWidth: Kirigami.Units.gridUnit * 14
        model: root.choices.map(n => root.pretty(n))
        currentIndex: root.value === "" ? 0 : Math.max(0, root.choices.indexOf(root.value))
        displayText: root.value === "" ? root.unsetLabel : root.pretty(root.value)
        onActivated: index => { const n = index === 0 ? "" : root.choices[index]; root.edited(n); if (root.model) root.model.playSound(n) }
        Accessible.name: "Sound"
    }
    QQC2.ToolButton {
        icon.name: "media-playback-start"
        display: QQC2.AbstractButton.IconOnly
        onClicked: if (root.model) root.model.playSound(root.value)
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.text: "Play it"
        Accessible.name: "Play the sound"
    }
}
