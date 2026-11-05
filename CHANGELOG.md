# Changelog

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
