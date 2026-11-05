pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import QtMultimedia

// An audio or video player inside a popup. Loaded only when a notification carries a local media file, so a machine without Qt Multimedia
// still runs notifd (the Loader just reports an error). It never starts by itself: the user presses play.
Item {
    id: root
    property url source
    property string kind: "audio"          // "audio" | "video"
    property color fg: "#f2f3f7"
    property color sub: "#b9bccb"
    property color track: "#272a36"
    property color accent: "#60a5fa"
    property string fontFamily: ""
    property real fontSize: 12
    readonly property bool playing: player.playbackState === MediaPlayer.PlayingState
    readonly property bool failed: player.error !== MediaPlayer.NoError

    implicitHeight: col.implicitHeight

    function fmt(ms) {
        const s = Math.max(0, Math.floor(ms / 1000))
        return Math.floor(s / 60) + ":" + (s % 60 < 10 ? "0" : "") + (s % 60)
    }

    MediaPlayer {
        id: player
        source: root.source
        audioOutput: AudioOutput { }
        videoOutput: root.kind === "video" ? vo : null
    }

    ColumnLayout {
        id: col
        anchors { left: parent.left; right: parent.right }
        spacing: 4

        VideoOutput {
            id: vo
            visible: root.kind === "video" && !root.failed
            Layout.fillWidth: true
            Layout.preferredHeight: Math.round(width * 9 / 16)
            fillMode: VideoOutput.PreserveAspectFit
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Rectangle {
                implicitWidth: 28; implicitHeight: 28; radius: 14
                color: root.accent
                opacity: root.failed ? 0.4 : 1
                Text {
                    anchors.centerIn: parent
                    text: root.playing ? "❚❚" : "▶"
                    color: "#1a1a1a"
                    font.pixelSize: 11
                }
                TapHandler { enabled: !root.failed; onTapped: root.playing ? player.pause() : player.play() }
                Accessible.name: root.playing ? "Pause" : "Play"
            }

            // seek bar: click or drag
            Rectangle {
                id: bar
                Layout.fillWidth: true
                implicitHeight: 6
                radius: 3
                color: root.track
                Rectangle {
                    width: player.duration > 0 ? parent.width * Math.min(1, player.position / player.duration) : 0
                    height: parent.height
                    radius: 3
                    color: root.accent
                }
                MouseArea {
                    anchors { fill: parent; topMargin: -8; bottomMargin: -8 }
                    enabled: player.duration > 0
                    onPressed: mouse => player.position = Math.max(0, Math.min(1, mouse.x / bar.width)) * player.duration
                    onPositionChanged: mouse => { if (pressed) player.position = Math.max(0, Math.min(1, mouse.x / bar.width)) * player.duration }
                }
            }

            Text {
                text: root.failed ? "cannot play" : root.fmt(player.position) + " / " + root.fmt(player.duration)
                color: root.failed ? "#f87171" : root.sub
                font.pixelSize: Math.max(6, root.fontSize - 1)
                font.family: root.fontFamily
            }
        }
    }
}
