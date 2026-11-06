import QtQuick
import QtTest
import "../../crates/notify-panel/qml"

// The shared choice controls and the look editor's styles: what is shown as picked, and what an edit emits.
Item {
    width: 900; height: 700
    property var emitted: []
    LookEditor { id: look; width: 800; values: ({ fontSize: "11", width: "320", timerStyle: "line", media: "false" }); onEdited: (k, t) => emitted.push(k + "=" + t) }
    DurationChips { id: dur; width: 600; value: "6000" }
    DurationChips { id: odd; width: 600; value: "20000" }
    Segmented { id: seg; options: [{ value: "a", label: "A" }, { value: "b", label: "B" }]; value: "b" }
    TestCase {
        name: "Controls"
        when: windowShown
        function test_a_style_is_recognised_from_its_values() {
            compare(look.currentPreset, "compact")
            compare(look.changedHere.length, 4)
        }
        function test_applying_a_style_sets_its_keys_and_clears_the_rest() {
            emitted = []
            look.applyPreset(look.presets.find(p => p.id === "glass"))
            verify(emitted.indexOf("opacity=0.8") >= 0, emitted)
            verify(emitted.indexOf("clickThrough=true") >= 0, emitted)
            verify(emitted.indexOf("fontSize=") >= 0, emitted)
            verify(emitted.indexOf("width=") >= 0, emitted)
            verify(emitted.indexOf("timerStyle=line") < 0, "an unchanged key is not re-sent: " + emitted)
        }
        function test_durations_are_named_choices() {
            verify(dur.isPreset)
            verify(!odd.isPreset, "20 s is not one of the named choices, so Custom is shown")
            verify(odd.customOpen)
        }
        function test_segmented_starts_wide_enough_for_one_line() {
            verify(seg.implicitWidth > 0)
            verify(seg.implicitHeight > 0)
        }
    }
}
