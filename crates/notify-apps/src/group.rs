//! The system category an app is listed under, taken from its launcher's freedesktop `Categories`.

/// The first matching group, in a fixed order so an app that is both "Network" and "Chat" lands under Messaging.
pub(crate) fn group_of(categories: &str) -> &'static str {
    let has = |names: &[&str]| categories.split(';').map(str::trim).any(|c| names.contains(&c));
    if has(&["InstantMessaging", "Chat", "IRCClient", "Email", "VideoConference"]) {
        "Messaging and email"
    } else if has(&["WebBrowser", "Network", "News", "Feed", "FileTransfer", "P2P"]) {
        "Internet"
    } else if has(&["Office", "Calendar", "ContactManagement", "ProjectManagement"]) {
        "Office"
    } else if has(&["Development", "IDE", "TextEditor"]) {
        "Development"
    } else if has(&["Game"]) {
        "Games"
    } else if has(&["AudioVideo", "Audio", "Video", "Player", "Music"]) {
        "Media"
    } else if has(&["Graphics", "Photography"]) {
        "Graphics"
    } else if has(&["Education", "Science"]) {
        "Education and science"
    } else if has(&["Settings", "System", "Monitor", "PackageManager", "Security", "HardwareSettings"]) {
        "System"
    } else if has(&["Utility", "Accessibility", "Archiving"]) {
        "Utilities"
    } else {
        "Other"
    }
}
