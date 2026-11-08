import QtQml

// Keyboard use of the notification list, kept apart from its window so it can be tested on its own (tests/qml/tst_centernav.qml).
// Up/Down/Home/End/PageUp/PageDown move; Enter or Space opens a notification or folds a group's heading; Delete removes a
// notification; 1-9 press its buttons. Tab reaches "Mark all read" and "Clear all"; Escape closes the list.
QtObject {
    // the row to select after moving `delta` rows from `i` (Infinity = the last, -Infinity = the first); -1 when there are none
    function step(count, i, delta) {
        if (count <= 0) return -1
        if (delta === Infinity) return count - 1
        if (delta === -Infinity) return 0
        if (i < 0) return delta < 0 ? count - 1 : 0
        return Math.max(0, Math.min(count - 1, i + delta))
    }
    // how far a key moves the selection, or 0 when it does not move it
    function moveBy(key) {
        switch (key) {
            case Qt.Key_Down: return 1
            case Qt.Key_Up: return -1
            case Qt.Key_PageDown: return 5
            case Qt.Key_PageUp: return -5
            case Qt.Key_End: return Infinity
            case Qt.Key_Home: return -Infinity
        }
        return 0
    }
    // what a key does to the selected row ({kind: "group"|"item"}): "fold", "open", "remove", "button" (with n: 0-8), or ""
    function action(row, key) {
        if (!row) return { what: "" }
        if (key === Qt.Key_Return || key === Qt.Key_Enter || key === Qt.Key_Space) return { what: row.kind === "group" ? "fold" : "open" }
        if (row.kind !== "item") return { what: "" }
        if (key === Qt.Key_Delete || key === Qt.Key_Backspace) return { what: "remove" }
        if (key >= Qt.Key_1 && key <= Qt.Key_9) return { what: "button", n: key - Qt.Key_1 }
        return { what: "" }
    }
}
