# Changelog

## 0.15.0

- **Look settings are part of the page:** the "Change how it looks and behaves…" buttons are gone; the look editor sits directly in a rule, an app, a notification type and a zone, under "How it looks and behaves".
- **Preview at the top:** the rule tester is opened with a **Preview** button at the top of the rule editor, and its result shows there, above the rule.
- Fixed a warning: the colour picker's list of swatches clashed with a built-in Qt property.
- **Rule editor order:** name, then how it looks and behaves (so the preview is in view first), then "When a notification matches" and "Then". In a full-screen window "When" and "Then" sit side by side and the rule list takes a third of the width.
- **One sidebar, grouped:** Overview, History · Appearance, Focus modes · Apps, Zones, Rules · General, System. Apps, Zones and Rules are entries of their own (the second side panel is gone). In a medium window the sidebar shows icons only; in a small one it hides behind the menu button.
- Fixed: a zone drop-down could show "(leave to later rules)" or the wrong zone after its list refreshed.
- **Change a notification from History:** click an entry to open it.
  - **Choose what "like this" means:** from this app, from this website, of this type; the box shows how many recent notifications that would match.
  - **Then pick** the zone (with the mini screen), how long they stay, whether to show them at all, and the full look editor.
  - **Its own rule:** the first change creates a rule just for these notifications, at the top of the list so it wins over broader rules; later changes edit the same rule. Save and Revert are right there, and "Open in Rules" shows the rule.
- **History records more:** each entry now keeps the sender's desktop entry, the notification's type (category) and how it arrived, so a rule can match exactly; the rule preview counts these too (entries recorded before this version lack them, which it says).

## 0.14.0

Release 1 of the panel redesign (see `docs/design/ui-redesign-combined.html`): choose, don't type.

- **New look editor** for zones, apps, notification types and rules:
  - **Styles:** pick a ready-made style first (As inherited, Compact, Glass, Loud, Minimal), each shown as a thumbnail.
  - **Tabs:** then adjust under three tabs: Look (text size, width, font, background), Timing (which time-left indicators) and Behaviour (priority, stacking, pictures and players, clicks, keyboard focus, group).
  - **Choices:** every setting is a set of choices.
  - **Inheritance:** an unchanged setting says what it inherits ("from the theme: 380 px"), a changed one has an undo button, and "Reset this level" clears everything changed here. A breadcrumb (Theme › Zone › App › Rule) shows the levels a popup's look passes through.
  - **Live preview:** click a part of the popup (title, edge, timer bar) to jump to its setting; "Hold to compare" shows it without this level's changes.
- **Timeouts are named choices in seconds** (App's own, Glance 3 s, Read 6 s, Long 15 s, Until closed, or Custom with a slider) for rules, apps and the default for all zones; no more milliseconds to type.
- **Zone colours** are picked from swatches (Plasma's accent first) or any colour from the colour dialog.
- **Appearance:**
  - **Font:** a searchable list where each font is drawn in its own face.
  - **Text size:** Small to Largest.
  - **Time left:** Nothing, Footer line, Circle or Frame, in any mix.
  - **Sound:** chosen from the installed sound themes, with a play button.
  - **History:** keeps 50 to 1000 notifications.

## 0.13.1

- **Audit fixes:**
  - **Wide windows:** Overview, History, General, System, Focus, Apps and Appearance keep a readable width, centred, instead of stretching labels and their switches across the whole window.
  - **Zone names:** zones are shown by name ("Business") everywhere they are picked (Rules, Apps, an app's notification types, the default zone), not by their id ("business").
  - **Rules list:** a rule reads as words when its conditions are plain lists ("website is shop or chat → business", "urgency is critical"); real regular expressions still show as patterns.
  - **Urgency and how a notification arrives** accept a bare word ("critical") as a tag; before, it was treated as a regular expression.
  - **Apps:** an app's details show where its popups appear (the mini screen was missing).
  - **Overview:** the zone switch reads "Mute", the action, rather than "Muted".
  - **`notify --help`:** every argument and option has a description.

## 0.13.0

- **Show, don't tell:**
  - **Live popup preview:** in the look settings for a zone, an app or a rule, a sample popup is drawn with that level's changes (opacity, width, font, time-left style). The timer runs, so a timeout and an indicator can be seen. On the Appearance page the preview now stays in view while you scroll.
  - **Rules:** "Would have matched N of your last M notifications" with examples from History as you edit the conditions; a mini screen next to "Show in zone" marks where the popups will appear.
  - **Apps:** a mini screen shows where an app's popups appear.
  - **Zones:** each zone on the map is drawn as the stack of popups it would show, to scale (width, gap between popups, distance from the screen edge, popups per zone).

## 0.12.0

- **Routing side panel:** Apps, Zones and Rules are chosen from a side panel on the left (icons only in a narrow window) instead of tabs along the top.
- **Apps:** view as Grid, List or List (compact), and every app is filed under its system category (Messaging and email, Internet, Office, Development, Media, Graphics, Games, Education, System, Utilities, Other). Categories that hold an app that has sent something open by themselves; click a heading to open or close it. In the grid, click a tile for its settings.
- **Zones and Rules use a wide window:** Zones puts the map and the zone's settings side by side; Rules puts the list and the editor side by side (they used to stack even in a wide window). Rule fields have their label above them so nothing is cut off.
- **Less duplication:** one "Add rule" menu (blank or a template) with the move, duplicate and delete buttons beside it; one Add zone button next to the zone buttons; the Place drop-down on a zone is gone (the map and the Snap/Freeform switch do that).

- **Routing tabs tidied:** Zones shows one zone at a time (pick it from the row of zone buttons, on the map, or add a new one), with the screen, default zone and size/spacing settings under "All zones". Apps keeps one line per app; the look and the per-type settings open separately under the arrow. Rules hides the look section and the sample-notification tester until asked for.

## 0.11.0

- **One Routing page:** Apps, Zones and Rules are now three tabs of a single page with one Save, so a tab switch never loses an edit.
- **Apps:** compact rows (icon, activity, zone, mute); timeout, Flatpak permission, look and notification types open with the arrow.
- **Zones:** a Snap to grid / Freeform switch. Snapping drops a dragged zone on the nearest of nine places; freeform places it anywhere.
- **Rules:** conditions are tags with suggestions from your history and apps (patterns that are real regular expressions stay editable as text); only the conditions in use are shown, with "Add a condition"; "New from template" for common rules (mute an app, send it to a zone, keep popups until dismissed, critical stays, silence a website, hide by title); the look section opens only when a rule changes something.
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
