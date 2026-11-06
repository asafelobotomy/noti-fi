import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// A font chosen from the installed ones: a button showing the current font in its own face, opening a searchable list where each
// name is drawn in its face. The list is built only when opened (a system can have hundreds of fonts). "" = inherit / system font.
QQC2.Button {
    id: root
    property string value: ""
    property string unsetLabel: "System font"
    signal edited(string text)
    text: root.value === "" ? root.unsetLabel : root.value
    font.family: root.value === "" ? Kirigami.Theme.defaultFont.family : root.value
    icon.name: "font-select-symbolic"
    onClicked: pop.open()
    Accessible.name: "Font: " + text

    QQC2.Popup {
        id: pop
        y: root.height
        width: Math.max(root.width, Kirigami.Units.gridUnit * 18)
        height: Kirigami.Units.gridUnit * 18
        padding: Kirigami.Units.smallSpacing
        onOpened: { search.text = ""; search.forceActiveFocus() }
        readonly property var all: pop.visible ? Qt.fontFamilies().filter(f => !f.startsWith(".")) : []
        readonly property var shown: [root.unsetLabel].concat(pop.all.filter(f => search.text === "" || f.toLowerCase().indexOf(search.text.toLowerCase()) >= 0))
        contentItem: ColumnLayout {
            Kirigami.SearchField { id: search; Layout.fillWidth: true; placeholderText: "Find a font"; Keys.onReturnPressed: if (pop.shown.length > 1) { root.edited(pop.shown[1]); pop.close() } }
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: pop.shown
                delegate: QQC2.ItemDelegate {
                    required property string modelData
                    required property int index
                    width: ListView.view.width
                    text: modelData
                    font.family: index === 0 ? Kirigami.Theme.defaultFont.family : modelData
                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.15
                    highlighted: (index === 0 && root.value === "") || modelData === root.value
                    onClicked: { root.edited(index === 0 ? "" : modelData); pop.close() }
                }
                QQC2.ScrollBar.vertical: QQC2.ScrollBar {}
            }
        }
    }
}
