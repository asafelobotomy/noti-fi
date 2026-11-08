import QtQuick
import QtTest
import "../../notifd"

// The notification list's keyboard: where the selection moves and what each key does to the selected row.
TestCase {
    name: "CenterNav"
    CenterNav { id: nav }

    function test_moving() {
        compare(nav.step(0, -1, 1), -1, "nothing to select in an empty list")
        compare(nav.step(5, -1, 1), 0, "Down with nothing selected picks the first row")
        compare(nav.step(5, -1, -1), 4, "Up with nothing selected picks the last row")
        compare(nav.step(5, 2, 1), 3)
        compare(nav.step(5, 4, 1), 4, "stops at the end")
        compare(nav.step(5, 0, -1), 0, "stops at the start")
        compare(nav.step(5, 2, Infinity), 4)
        compare(nav.step(5, 2, -Infinity), 0)
        compare(nav.step(5, 1, 5), 4, "PageDown stops at the end")
        compare(nav.moveBy(Qt.Key_Down), 1)
        compare(nav.moveBy(Qt.Key_Home), -Infinity)
        compare(nav.moveBy(Qt.Key_A), 0)
    }

    function test_keys_act_on_the_selected_row() {
        const group = { kind: "group", zone: "personal" }, item = { kind: "item", entry: { id: 1 } }
        compare(nav.action(group, Qt.Key_Return).what, "fold", "Enter on a heading folds the group")
        compare(nav.action(group, Qt.Key_Space).what, "fold")
        compare(nav.action(item, Qt.Key_Return).what, "open", "Enter on a notification opens it")
        compare(nav.action(item, Qt.Key_Delete).what, "remove")
        compare(nav.action(group, Qt.Key_Delete).what, "", "a heading cannot be removed")
        const b = nav.action(item, Qt.Key_2)
        compare(b.what, "button"); compare(b.n, 1, "2 presses the second button")
        compare(nav.action(null, Qt.Key_Return).what, "", "nothing selected: nothing happens")
        compare(nav.action(item, Qt.Key_X).what, "")
    }
}
