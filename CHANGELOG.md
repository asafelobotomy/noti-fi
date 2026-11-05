# Changelog

## 0.8.1

- **Fix: the prebuilt panel could not start on a machine whose Qt differs from the one it was built with** (the first releases were built against Qt 6.12 and failed on Qt 6.11 with an undefined-symbol error). The panel's compiled QML only runs on the Qt it was built against, so now:
  - the release archive records the Qt version it was built with (`qt-version`);
  - `install.sh` and `notify upgrade --apply` replace the command-line tool as before but leave an existing panel alone when the Qt versions are not compatible, and say how to build one for this machine;
  - `packaging/PKGBUILD` builds noti-fi against the Qt installed on your machine (`makepkg -si`), which is the recommended way to install the panel on Arch and Manjaro.

## 0.8.0

- **Updates**: `notify upgrade` looks for a newer release (and `notify upgrade --apply` downloads it, checks its checksum, replaces `notify` and `notify-panel`, and refreshes the daemon). The panel's General page has the same: Check now, Install, Restart, and an opt-in daily check (off by default; nothing is contacted unless you ask). Only this project's GitHub release downloads over HTTPS are accepted, and files are replaced by rename so a running program is never overwritten in place.

## 0.7.0

The control panel and a large set of notification features.

- **Control panel** (Rust, cxx-qt, Kirigami): Overview, History, Apps, Rules (with simulator and preview), Zones and layout, Appearance, Focus modes, General. System tray icon with a status menu, close to tray, start at login, one panel at a time.
- **Placement**: nine places on a 3x3 grid, or free placement.
- **Look and behaviour per rule, zone, app or kind of notification**: transparency, click-through, no focus, width (and drag to resize), fonts, groups, priority, stacking with counts, pictures and opt-in audio/video players, pin from the popup.
- **Focus modes** (only important notifications get through), priorities, expandable download/transfer graphs (line, bars, heatmap).
- **Apps**: every app that sent a notification, and installed apps that could; per-app and per-kind controls; Flatpak permissions.
- **Security and performance**: no `/tmp` fallback for sockets, fenced test hooks, pattern limits, service hardening, lighter hidden panel.
- **Project**: a 400-line limit per source file (ratcheted), CI and release workflows, MIT licence.

## 0.6.0

The notification daemon, the Rust CLI and the control socket.
