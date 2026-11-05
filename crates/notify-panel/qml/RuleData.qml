import QtQuick

// What the rule editor offers: the conditions a rule can have, and ready-made rules to start from.
QtObject {
readonly property var matchFields: [
        { field: "appName", label: "App:", menu: "App", role: "mAppName", kind: "exact", hint: "Steam" },
        { field: "desktopEntry", label: "Desktop entry:", menu: "Desktop entry", role: "mDesktopEntry", kind: "exact", hint: "org.kde.kmail2" },
        { field: "origin", label: "Website:", menu: "Website (from a browser)", role: "mOrigin", kind: "word", hint: "example.com" },
        { field: "summary", label: "Title has:", menu: "Title contains", role: "mSummary", kind: "contains", hint: "message" },
        { field: "body", label: "Text has:", menu: "Text contains", role: "mBody", kind: "contains", hint: "" },
        { field: "urgency", label: "Urgency:", menu: "Urgency", role: "mUrgency", kind: "exact", hint: "critical" },
        { field: "category", label: "Kind:", menu: "Kind of notification", role: "mCategory", kind: "start", hint: "email" },
        { field: "channel", label: "Arrives as:", menu: "How it arrives", role: "mChannel", kind: "exact", hint: "web" }
    ]

    // a name, the tags to match and what the rule does (zone "$first" = the first zone)
readonly property var templates: [
        { name: "Mute an app", hint: "Pick the app, then it never pops up (kept in History)", match: {}, silent: true },
        { name: "Send an app to a zone", hint: "Pick the app and the zone", match: {}, zone: "$first" },
        { name: "Keep an app's popups until dismissed", hint: "Pick the app", match: {}, timeout: "0" },
        { name: "Critical notifications stay until dismissed", hint: "", match: { urgency: "^critical$" }, timeout: "0" },
        { name: "Silence a website's notifications", hint: "Pick the site", match: { channel: "^web$" }, silent: true },
        { name: "Hide notifications whose title has…", hint: "Add the words", match: {}, silent: true }
    ]
}
