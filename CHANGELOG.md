# Changelog

## Unreleased

- **Panel layout pass:** the General page now scrolls and is grouped as Window and tray, Start at login, Updates, with shorter help text. Help text on General, Rules, Focus and Appearance is the small font it was meant to be (it rendered large). Focus mode cards stack their fields so nothing runs past the card. Section headings have breathing room, the Appearance heading no longer clips, and the System page buttons line up.

## 0.10.0

- **System page in the panel:** the health checks (`notify doctor`) and the install actions (take over, restore, make Brave deliver, update the daemon). Every action shows its plan first and only runs after a second click and a confirmation.
- **First run:** when notifd was never installed, the panel and `notify` now say so and show the two commands to run, instead of "notifd is not running".
- **Panel:** a "Hide to tray" button on the General page (Wayland never reports a minimise, so the minimise-to-tray option cannot work there; the button does the same job on demand).
- **Popups** announce themselves to screen readers (alert role with app, title and text).
- **Packaging:** an AppStream metainfo file and a scalable icon (`org.notify.Panel`).
- **Repo:** SECURITY.md, CONTRIBUTING.md, issue templates, Dependabot, and a `cargo deny` job in CI (advisories, licences, sources). README "Known limits" now lists what is unverified or unsupported.
- **Tests:** popup interaction tests (tap, pin, stack, click-through, drag-resize) with Qt Quick Test, a sound-playback test with a stand-in player, a System page smoke test; one timing-sensitive daemon test given a wider margin.
- **Internal:** clippy is clean; `draft.rs`, `install.rs`, `model.rs`, the protocol and apps crates, `Popup.qml`, `RulesPage.qml` and the daemon test suite are now under the 400-line limit (`shell.qml` is down from 1394 to 1073 lines and stays baselined).

## 0.9.1

- **Fix: `notify update` / `notify upgrade --apply` from 0.9.0 failed its smoke test and left the daemon on the old version** (nothing was broken: the check exists to stop exactly this). The binary's hand-kept list of daemon files was missing `TimerRing.qml`. The list is now generated from the `notifd/` folder at build time, and tests check that every file there is embedded and that the embedded daemon starts on a private bus.

## 0.9.0

- **Time left on each popup.** Three optional indicators, alone or together: a footer line that shortens, a circle that empties (in the popup's header), and a frame around the card that shrinks from the top middle clockwise. Set `timerStyle` (`none`, or any of `line`, `circle`, `frame` separated by commas) on the theme, a zone, an app, a kind of notification or a rule; the Appearance page and the look sections of the Rules, Zones and Apps pages have a picker. New installs default to the footer line; existing configs stay as they are (off) until you choose.
- Hovering, a reply box, a pin, a playing media file or an open graph now *pause* a popup's countdown instead of restarting it, and the indicators freeze with it.
- Internal: the daemon's transfer sampling and media-file lookup moved into `Transfers.qml` and `MediaFile.qml`, the protocol's config model into its own file, and the test harnesses copy every `*.qml` in `notifd/` instead of keeping a list.

## 0.8.2

- **The panel now runs on any Qt 6, not just the one it was built with.** Qt compiles QML ahead of time into code that uses its private interfaces, which is what made 0.7.0 and 0.8.0 fail to start on a machine with a different Qt. The panel's build now skips that step (the QML is compiled when the panel starts, which is a little slower to open) and so uses no private interfaces. Release archives mark such a panel `portable-qml`, and `install.sh` and `notify upgrade --apply` install it on any Qt 6; the Qt-version guard from 0.8.1 still protects older, non-portable archives. Set `NOTIFI_QML_AOT=1` when building to keep Qt's ahead-of-time compilation.

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
