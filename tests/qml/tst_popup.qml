import QtQuick
import QtTest
import "../../notifd"

// Interaction tests for the popup card, run offscreen: tap, pin, stack, click-through, drag-resize.
Item {
    id: host
    width: 600; height: 400

    property int dismissed: 0
    property int pinToggled: 0
    property int stackToggled: 0
    property real lastWidth: 0

    Component {
        id: cardComp
        Popup {
            x: 20; y: 20; width: 400; height: 140
            timeoutMs: 0
            notif: ({ appName: "app", summary: "Hello", body: "text", actions: [], hints: {}, image: "", appIcon: "", desktopEntry: "", resident: false })
            onDismissed: host.dismissed++
            onPinToggled: host.pinToggled++
            onStackToggled: host.stackToggled++
            onResized: w => host.lastWidth = w
        }
    }

    TestCase {
        name: "Popup"
        when: windowShown

        property var card: null

        function init() {
            host.dismissed = 0; host.pinToggled = 0; host.stackToggled = 0; host.lastWidth = 0
            card = createTemporaryObject(cardComp, host)
            wait(300)                       // the slide-in
        }

        function test_tap_dismisses() {
            mouseClick(card, 200, 100)
            compare(host.dismissed, 1)
        }

        function test_click_through_ignores_taps() {
            card.clickThrough = true
            mouseClick(card, 200, 100)
            compare(host.dismissed, 0)
        }

        function test_inactive_ignores_taps() {
            card.active = false
            mouseClick(card, 200, 100)
            compare(host.dismissed, 0)
        }

        function test_collapsed_stack_opens_instead_of_dismissing() {
            card.stackCount = 4
            mouseClick(card, 200, 100)
            compare(host.stackToggled, 1)
            compare(host.dismissed, 0)
            card.stackExpanded = true
            mouseClick(card, 200, 100)
            compare(host.dismissed, 1)
        }

        function test_pin_pill_toggles_without_dismissing() {
            let target = null
            function walk(it) {
                for (let i = 0; i < it.children.length; i++) {
                    const c = it.children[i]
                    if (c.text === "Pin" || c.text === "Pinned") { target = c.parent; return }
                    walk(c)
                    if (target) return
                }
            }
            walk(card)
            verify(target !== null, "pin pill exists")
            const p = card.mapFromItem(target, target.width / 2, target.height / 2)
            mouseMove(card, p.x, p.y)       // hover first, as a pointer would
            mouseClick(card, p.x, p.y)
            compare(host.pinToggled, 1)
            compare(host.dismissed, 0)
        }

        function test_a_button_of_a_resident_notification_keeps_it() {
            let pressed = 0
            card.notif = { appName: "Dolphin", summary: "Copying", body: "", hints: {}, image: "", appIcon: "", desktopEntry: "", resident: true,
                           actions: [{ identifier: "suspend", text: "Pause", invoke: () => pressed++ }] }
            wait(100)
            let target = null
            function walk(it) {
                for (let i = 0; i < it.children.length; i++) {
                    const c = it.children[i]
                    if (c.text === "Pause") { target = c.parent; return }
                    walk(c)
                    if (target) return
                }
            }
            walk(card)
            verify(target !== null, "the Pause button exists")
            const p = card.mapFromItem(target, target.width / 2, target.height / 2)
            mouseMove(card, p.x, p.y)       // hover first, as a pointer would
            mouseClick(card, p.x, p.y)
            compare(pressed, 1, "the button ran its action")
            compare(host.dismissed, 0, "and the card around it did not dismiss the notification (a running copy stays up to be resumed)")
        }

        function test_drag_resize() {
            // the handle is the strip on the card's right edge (left edge when handleOnLeft)
            const x0 = card.width - 4, y0 = card.height / 2
            mousePress(card, x0, y0)
            mouseMove(card, x0 + 60, y0)
            mouseRelease(card, x0 + 60, y0)
            verify(host.lastWidth > card.width, "dragging right widens: " + host.lastWidth)
            compare(host.dismissed, 0)
        }

        function test_resize_is_clamped() {
            const x0 = card.width - 4, y0 = card.height / 2
            mousePress(card, x0, y0)
            mouseMove(card, x0 - 5000, y0)
            mouseRelease(card, x0 - 5000, y0)
            compare(host.lastWidth, 160)
        }
    }
}
