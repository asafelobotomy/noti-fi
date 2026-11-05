import QtQuick
import QtQuick.Layouts

// A download or transfer: the rate over time, with a button to open the graph and a choice of how to draw it.
// The popup owns the state (open, mode); this only draws it and says what was clicked.
ColumnLayout {
    id: g
    property var samples: []
    property bool open: false
    property string mode: ""            // this popup's own choice; empty = the default
    property string defaultMode: "line"
    property color accent: "#60a5fa"
    property color cardColor: "#1d1f27"
    property color cText: "#f2f3f7"
    property color cSub: "#b9bccb"
    property color cButton: "#272a36"
    property color cBorder: "#2c2f3a"
    property string fontFamily: ""
    property real fontSize: 12
    readonly property bool hovered: graphHover.hovered
    signal modeChosen(string mode)
    signal toggled()

    spacing: 4
    HoverHandler { id: graphHover }

    RowLayout {
        Layout.fillWidth: true
        spacing: 6
        Text {
            text: "Transfer rate"
            color: g.cSub
            font.pixelSize: Math.max(6, g.fontSize - 1)
            font.family: g.fontFamily
            Layout.fillWidth: true
        }
        Repeater {
            model: g.open ? ["bars", "line", "heatmap"] : []
            delegate: Rectangle {
                id: modePill
                required property string modelData
                readonly property bool on: (g.mode !== "" ? g.mode : g.defaultMode) === modelData
                implicitWidth: modeText.implicitWidth + 12
                implicitHeight: modeText.implicitHeight + 4
                radius: 8
                color: on ? g.accent : g.cButton
                Text {
                    id: modeText
                    anchors.centerIn: parent
                    text: modePill.modelData === "bars" ? "Bars" : modePill.modelData === "line" ? "Graph" : "Heatmap"
                    color: modePill.on ? g.cardColor : g.cText
                    font.pixelSize: Math.max(6, g.fontSize - 2)
                    font.family: g.fontFamily
                }
                TapHandler { onTapped: g.modeChosen(modePill.modelData) }
                Accessible.name: "Show as " + modelData
            }
        }
        Rectangle {
            implicitWidth: toggleText.implicitWidth + 12
            implicitHeight: toggleText.implicitHeight + 4
            radius: 8
            color: g.cButton
            Text {
                id: toggleText
                anchors.centerIn: parent
                text: g.open ? "Hide graph" : "Show graph"
                color: g.cText
                font.pixelSize: Math.max(6, g.fontSize - 2)
                font.family: g.fontFamily
            }
            TapHandler { onTapped: g.toggled() }
            Accessible.name: g.open ? "Hide the graph" : "Show the graph"
        }
    }
    TransferChart {
        visible: g.open
        Layout.fillWidth: true
        Layout.preferredHeight: visible ? 118 : 0
        samples: g.samples
        mode: g.mode !== "" ? g.mode : g.defaultMode
        accent: g.accent
        text: g.cSub
        grid: g.cBorder
        fontFamily: g.fontFamily
        fontSize: Math.max(6, g.fontSize - 1)
    }
}
