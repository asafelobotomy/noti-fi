import QtQuick
import QtCore
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.notify.panel

Kirigami.ApplicationWindow {
    id: root
    title: "noti-fi"
    // started with --background (start at login): stay in the system tray until asked
    visible: !root.startHidden
    width: Kirigami.Units.gridUnit * Number(argValue("--width", args.indexOf("--wide") >= 0 ? "100" : "38"))   // --wide / --width=N (grid units): screenshot hooks
    height: Kirigami.Units.gridUnit * (args.indexOf("--tall") >= 0 ? 80 : 34)   // --tall: a screenshot hook that shows a whole page
    minimumWidth: Kirigami.Units.gridUnit * 22
    minimumHeight: Kirigami.Units.gridUnit * 18

    // The one Rust object the whole UI binds to. It connects to notifd in the background and stays live.
    PanelModel {
        id: panelModel
        Component.onCompleted: { setTrayShowText(uiSettings.showText); start() }
        onCommandFailed: message => root.showPassiveNotification(message)
    }

    // window size and the two history toggles are remembered between runs
    Settings {
        id: uiSettings
        category: "ui"
        property alias width: root.width
        property alias height: root.height
        property bool showText: false
        property bool missedOnly: false
        property bool autoCheckUpdates: false     // off by default: nothing is contacted unless asked
        property bool closeToTray: true
        property bool minimizeToTray: true
    }

    // ---- system tray (the icon and its menu live in Rust; see the model) --------------------------
    readonly property bool trayAvailable: panelModel.trayAvailable
    // started at login (--background): stay hidden in the tray; if no tray shows up within a few seconds, show the window so the app is not left invisible
    readonly property bool startHidden: Qt.application.arguments.indexOf("--background") >= 0
    Timer {
        running: root.startHidden
        interval: 5000
        onTriggered: if (!panelModel.trayAvailable && !root.visible) root.showWindow()
    }
    property bool quitting: false
    function showWindow() {
        root.show()
        if (root.visibility === Window.Minimized) root.showNormal()
        root.raise()
        root.requestActivate()
    }
    function quitApp() { root.quitting = true; Qt.quit() }
    Connections {
        target: uiSettings
        function onShowTextChanged() { panelModel.setTrayShowText(uiSettings.showText) }
    }
    Connections {
        target: panelModel
        // another launch of the program (the app menu, login) or the tray's Open brings this window forward
        function onShowRequested() { console.log("SHOW requested by another launch"); root.showWindow() }
        // the tray asks for the window: when notifd is not running (a left click otherwise toggles notifd's notification list)
        function onToggleRequested() { if (root.visible && root.active) root.hide(); else root.showWindow() }
        function onPageRequested(page) { root.go(page); root.showWindow() }
        function onQuitRequested() { root.quitApp() }
    }
    // closing the window keeps the panel running in the tray (when there is one)
    onClosing: close => {
        if (uiSettings.closeToTray && panelModel.trayAvailable && !root.quitting) {
            close.accepted = false
            root.hide()
        }
    }
    // The opt-in daily update check: shortly after start, then every 24 hours (only when the box on the General page is ticked).
    Timer {
        running: uiSettings.autoCheckUpdates
        interval: 30000
        onTriggered: { panelModel.checkForUpdates(); interval = 86400000; restart() }
    }
    // While only the tray icon is showing, the pages are unloaded and the panel fetches almost no history; both come back when the window does.
    property string lastPage: "overview"
    onVisibleChanged: {
        panelModel.setWindowVisible(root.visible)
        if (!root.visible && panelModel.trayAvailable && !root.quitting) { pageStack.clear(); root.currentPage = "" }
        else if (root.visible && root.currentPage === "") root.go(root.lastPage)
    }
    // minimising moves to the tray where the desktop reports it (not on Wayland, see the General page)
    onVisibilityChanged: {
        if (root.visibility === Window.Minimized && uiSettings.minimizeToTray && panelModel.trayAvailable && !root.quitting)
            Qt.callLater(() => root.hide())
    }

    // ---- command line / test hooks ------------------------------------------------------------
    //   --page=history   open a page directly        --quit-after=N  quit after N seconds
    //   --report         print a JSON summary on exit    --selftest    drive every invokable, then quit
    //   --screenshot=F   render this window (only this window) to the PNG file F after 2.5 s, then quit
    readonly property var args: Qt.application.arguments
    readonly property bool wantReport: args.indexOf("--report") >= 0
    function argValue(name, fallback) {
        const a = args.filter(x => x.indexOf(name + "=") === 0)
        return a.length ? a[0].substring(name.length + 1) : fallback
    }

    // pages are declared as components, so Kirigami places them in the scene itself
    Component { id: overviewPage; OverviewPage { model: panelModel; ui: uiSettings; onOpenPage: name => root.go(name) } }
    Component { id: historyPage; HistoryPage { model: panelModel; ui: uiSettings; onOpenPage: name => root.go(name) } }
    Component { id: appearancePage; AppearancePage { model: panelModel; ui: uiSettings } }
    Component { id: generalPage; GeneralPage { model: panelModel; ui: uiSettings; trayAvailable: panelModel.trayAvailable; onHideRequested: root.hide() } }
    Component { id: focusPage; FocusPage { model: panelModel; ui: uiSettings } }
    Component { id: systemPage; SystemPage { model: panelModel; ui: uiSettings } }
    Component { id: routingPage; RoutingPage { model: panelModel; ui: uiSettings; tab: root.routingTab; onTabSelected: name => { root.routingTab = name; root.lastPage = name } } }

    property string currentPage: ""
    property string routingTab: "apps"
    // Apps, Zones and Rules are the three tabs of one Routing page
    function go(page) {
        lastPage = page
        if (["apps", "zones", "rules"].indexOf(page) >= 0) {
            routingTab = page
            if (currentPage === "routing") { pageStack.currentItem.open(page); return }
            page = "routing"
        }
        if (currentPage === page) return
        currentPage = page
        pageStack.clear()
        pageStack.push(page === "history" ? historyPage : page === "routing" ? routingPage : page === "general" ? generalPage : page === "system" ? systemPage : page === "focus" ? focusPage : page === "appearance" ? appearancePage : overviewPage)
    }

    Timer {
        running: argValue("--quit-after", "") !== ""
        interval: Number(argValue("--quit-after", "0")) * 1000
        onTriggered: Qt.quit()
    }
    Timer {
        running: argValue("--screenshot", "") !== ""
        interval: 2500
        onTriggered: {
            giveUp.start()                       // never hang if the grab cannot complete (for example without a display)
            // the window's own root item has no QML engine, so grab the application content inside it
            const target = root.contentItem.children.length ? root.contentItem.children[0] : root.pageStack
            target.grabToImage(result => {
                result.saveToFile(argValue("--screenshot", ""))
                Qt.quit()
            })
        }
    }
    Timer {
        id: giveUp
        interval: 4000
        onTriggered: { console.error("screenshot: the grab did not complete"); Qt.quit() }
    }
    Connections {
        target: Qt.application
        function onAboutToQuit() {
            if (!root.wantReport) return
            panelModel.report()
            const page = pageStack.currentItem
            console.log("UI " + JSON.stringify(page && page.uiState ? page.uiState : {}))   // what the user would actually see
        }
    }

    // the headless test hooks (--selftest and friends) live in their own file
    SelfTest { model: panelModel; args: root.args }

    globalDrawer: Kirigami.GlobalDrawer {
        title: "noti-fi"
        titleIcon: "preferences-desktop-notification"
        isMenu: false
        collapsible: true
        collapseButtonVisible: true
        // a small window hides the sidebar behind a button; a medium one shows icons only; a big one shows names too
        modal: root.width < Kirigami.Units.gridUnit * 30
        collapsed: !modal && root.width < Kirigami.Units.gridUnit * 48
        // grouped: now · how popups look · where they go · the app itself (a separator between groups)
        actions: [
            Kirigami.Action { text: "Overview"; icon.name: "view-list-details"; checked: root.currentPage === "overview"; onTriggered: root.go("overview") },
            Kirigami.Action { text: "History"; icon.name: "view-history"; checked: root.currentPage === "history"; onTriggered: root.go("history") },
            Kirigami.Action { separator: true },
            Kirigami.Action { text: "Appearance"; icon.name: "preferences-desktop-theme"; checked: root.currentPage === "appearance"; onTriggered: root.go("appearance") },
            Kirigami.Action { text: "Focus modes"; icon.name: "face-glasses"; checked: root.currentPage === "focus"; onTriggered: root.go("focus") },
            Kirigami.Action { separator: true },
            Kirigami.Action { text: "Apps"; icon.name: "preferences-desktop-notification-bell"; checked: root.currentPage === "routing" && root.routingTab === "apps"; onTriggered: root.go("apps") },
            Kirigami.Action { text: "Groups"; icon.name: "view-group"; checked: root.currentPage === "routing" && root.routingTab === "zones"; onTriggered: root.go("zones") },
            Kirigami.Action { text: "Rules"; icon.name: "view-filter"; checked: root.currentPage === "routing" && root.routingTab === "rules"; onTriggered: root.go("rules") },
            Kirigami.Action { separator: true },
            Kirigami.Action { text: "General"; icon.name: "configure"; checked: root.currentPage === "general"; onTriggered: root.go("general") },
            Kirigami.Action { text: "System"; icon.name: "system-run"; checked: root.currentPage === "system"; onTriggered: root.go("system") }
        ]
    }

    // pushed after the window exists, so the page row is already in the scene
    Component.onCompleted: Qt.callLater(() => { if (root.startHidden) root.lastPage = argValue("--page", "overview"); else go(argValue("--page", "overview")) })
}
