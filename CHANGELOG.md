# Changelog

## Unreleased

- **Fixed: uQ's theme didn't reach noti-fi on a stock install.** The `rules.json` that `notify takeover` writes set the six theme colours explicitly, and settings in `rules.json` win over a desktop's tokens, so `theme.tokens: "auto"` never showed uQ's colours. The shipped file now leaves them out; the daemon's defaults are the same colours, so nothing changes without uQ. An existing `rules.json` keeps its colours: remove `card`, `border`, `text`, `subtext`, `button` and `buttonHover` from its `theme` to follow uQ.

## 0.18.1

- **Audit fixes (daemon):** while the screen is shared, popups also hide their button labels and the reply hint, and the socket's `history` and `notification`/`updated` events carry no title or text (`hidden: true`). A replaced notification's history entry now follows it (shown or held, urgency, icon, and unread again), and only this run's entries are matched by id (ids restart with notifd). The list's keys act only while the list has focus, and a button of a notification its app just closed no longer fails. Turning screen-share or full-screen holding back on during a share applies at once. The PipeWire watcher no longer leaves `pw-dump`/`python3` behind. Theme tokens accept only `#rgb`/`#rrggbb`; `theme.tokens` also takes `"off"`. History caps titles at 300 characters; sound names cannot start with "-"; an app called "constructor" no longer breaks the seen-apps count; a rule's `silent` must be true or false.
- **Audit fixes (`notify serve`, CLI, job bridge):** Quickshell and the private bus are stopped on every failed start; stale `notifd-bus-*` folders are swept; it refuses to run without `XDG_RUNTIME_DIR`; a dead helper thread stops it (so systemd restarts it) instead of silently dropping signals; inhibitions are retried until notifd takes them. The job bridge forgets its notification ids when notifd restarts. `restore` removes a KWin script that never loaded and carries on past a shortcut it cannot remove. `notify upgrade` and the pre-flight test daemon use private folders (not `/tmp`); Brave backups are private; downloads are capped at 300 MB. `notify rules` (without a daemon) now refuses what notifd refuses. `doctor` checks for `pw-dump` and `python3`.
- **Audit fixes (panel):** says when a focus mode holds notifications; the tray tooltip names full-screen, sharing and app holds; Save no longer stays disabled after notifd restarts mid-edit; History opens the right row after a restart; new controls for corners, following uQ's theme, the list's nine positions and "focused monitor"; app and check text is plain text everywhere.
- **Packaging, CI, tests:** the PKGBUILD depends on `dbus`, `pipewire` and `python` (and no longer on `git`, with a `prepare()` that fetches crates); CI runs the `notify-update` and `notify-bus` unit tests and `--test lifecycle`; the `version` test also checks the PKGBUILD, metainfo and changelog; the CLI smoke test uses a private bus and home; the schema checker knows maximums and lengths; the schema accepts `group` in focus modes; the QML tests fail in CI if their runner is missing.

- **AUR-ready package:** each release also uploads a source archive (`noti-fi-<version>.tar.gz`) with its checksum, and the PKGBUILD builds from it. The release's `aur` artifact holds the PKGBUILD with the real checksum and a `.SRCINFO`, ready to push to the AUR.

- **Fixed: hovering a taskbar entry hid notifications.** Plasma draws its window previews from short-lived KWin captures read by plasmashell, which counted as screen sharing. Captures read by plasmashell no longer count, nor does a capture stream nobody reads (KDE's remote-control sessions keep an idle one open). Checked live by hovering taskbar entries.

- **Who is capturing the screen:** the screen-share watch now names the programs reading KWin's capture (`sharingBy` in `status`), and `autoDnd.ignoreCaptureBy` lists programs whose captures do not count as sharing. Found because Claude's computer use (`kwin-portal-bridge`) captures the screen and so hid notifications, which looked like another app's doing.

- **Hyprland: screen sharing and full screen hold popups.** notifd follows Hyprland's own events (`HyprlandWatch.qml`, which also gives the focused monitor): a screen capture counts as sharing (text hidden, popups held) until 1.5 s after its last frame, and a full-screen window (not a maximised one) holds popups as the KWin script does on Plasma. Tested on a real Hyprland with `wf-recorder` and a full-screen terminal.
- **The notification list from the keyboard:** arrow keys, Home/End and PageUp/PageDown move; Enter or Space opens a notification or folds a group; Delete removes one; 1-9 press its buttons; Tab reaches the header buttons (Enter presses them). Tested with real key presses on Hyprland (`wtype`) and in `tst_centernav.qml`.

- **Dependencies:** rusqlite 0.40 and sha2 0.11 (the newest that build with Rust 1.85); GitHub Actions checkout 7, cache 6, upload-artifact 7, download-artifact 8 (Dependabot's updates, taken on main in one tested change).
- **Hygiene:** the command-line definitions moved from `main.rs` (399 lines) to `cli.rs`, and a popup's buttons from `Popup.qml` (400) to `PopupButtons.qml`, so neither sits at the line limit; Python bytecode is no longer committed; the README lists every daemon file, crate and test suite.

## 0.18.0

- **Screen sharing is seen at once:** notifd now follows PipeWire as it changes (`pw-dump --monitor`) instead of checking every 4 s, and KDE's screen-share portal's "Screen sharing in progress" inhibition counts as sharing too, so a notification arriving as a share starts no longer shows its text.
- **One notifd at a time:** `notify serve` refuses to start while another notifd answers on the control socket (a second one used to take the socket's path and leave the running one unreachable).
- **Hyprland, tested for real:** `tests/hyprland/run.sh` runs notifd behind `notify serve` in a nested Hyprland (0.56) in a container: the focused monitor, `"screen": "focused"`, the level OSD and unplugging a monitor. `status` gains `screens` (`{popups, osd, focused}`).
- **Fixed:** a binding-loop warning from the OSD window.

- **Notification center:** a left click on the tray icon opens a list of recent notifications next to the tray, under each group's heading in its colour (fold a group into a stack with a click on its heading). **Mark all read** and **Clear all** at the top; click one to mark it read, × to forget it. Escape or a click elsewhere closes it. Its corner is `center.anchor` (default bottom-right), also on the panel's Appearance page.
- **Hyprland and other desktops:** outside KDE (or with `notify takeover --plain`) notifd installs as a plain user service of the graphical session, with no Plasma takeover, fallback or KWin script; `restore` and `update` follow. `"screen": "focused"` puts popups and the list on Hyprland's focused monitor.
- **For desktop shells (uQ):** the socket greeting has `name: "notifd"`; `state` gains `sharing` and `latest` (`{id, t, appName, appIcon, summary, read}`, no title while sharing); a new `osd` command draws a volume, mic or brightness level. The OSD and the confirmation pill appear on Hyprland's focused monitor, and every window picks another screen when its monitor is unplugged.
- **Design tokens:** `theme.tokens` ("auto" = uQ's `~/.local/state/uq/theme/tokens.json` when present, a path, or "" for none) gives the theme its colours, corner radius (new `theme.radius`) and font; your own `theme` settings win. Read every 2 s so uQ's whole-folder swaps are seen; a broken file keeps the last good tokens.
- **Privacy while sharing:** while the screen is being shared, popups and the notification list show only the app: no title, text, pictures or players.
- **Inhibit:** apps can ask for notifications to be held (`Inhibit`/`UnInhibit` and the `Inhibited` property, Plasma's extension, used for presentations, calls and screen recordings). notifd holds popups like Do Not Disturb while any app does (critical ones still show), and the hold ends when the app lifts it or quits. Quickshell cannot answer these, so `notifd.service` now runs `notify serve` (crate `notify-bus`), which owns the notification name, runs Quickshell on a private bus behind it and passes everything else straight through. `notify update` installs the new unit; status shows `inhibitors`, and `holding` is `inhibited`.
- **Buttons in the notification list:** a popup that times out now stays open for its app (like Plasma), so the list shows its buttons and clicking it does what clicking the popup would. Removing it, **Clear all**, or pressing a button closes it for the app; at most `center.keep` (default 50, 0 = the old behaviour; also on the panel's Appearance page) are kept, oldest closed first. Notifications held by Do Not Disturb or a mute are kept the same way. Protocol: `invoke`, history `live` and `actions`, status `kept`.
- **Keyboard:** `notify center [toggle|open|close]` opens and closes the list; `notify hotkey --center [KEYS]` binds it to a global shortcut (default Meta+N), and the panel's System page can set it up.
- **Unread count on the tray icon:** while anything is unread the icon shows the number on a disc split into the colours of the groups the unread notifications are in. The menu gains **Mark all as read** and **Clear all**; the tooltip lists unread per group. A popup you close yourself counts as read, one that times out does not.
- **Protocol:** `status` has `unread`, `unreadPerZone` and `center`; history entries have `read`; new methods `center`, `markRead`, `removeHistory` (and `qs ipc` `center`, `markAllRead`).
- **Fixed (security): the notification list could load remote images.** Its app line, title and button labels used Qt's automatic text format, which reads HTML-looking text as rich text, and escaped markup (`&lt;img src=…&gt;`) was turned back into tags; a notification could make the list fetch a URL when opened (leaking your IP). Every app-supplied string in the list is now plain text; in the panel, labels that cannot be plain text (buttons, list rows) never contain `<` or `>`. Popups were not affected. The no-fetch test now covers app names, escaped markup, button labels and the list.
- **Fixed: a popup's buttons closed it.** A click on a button also counted as a click on the popup, which dismisses it, so a resident notification (a running copy's **Pause**) vanished instead of turning into **Resume**.
- **Fixed: screen sharing was never detected.** KWin's capture stream names itself only in `node.name`/`media.name` (`kwin_wayland`, `kwin-screencast-…`), which the probe did not read. Checked against a real OBS capture.
- **Fixed:** the panel's launcher names `notify-panel` by its full path when installed to a prefix (such as `~/.local`), which not every launcher has on its PATH.
- **`notify restore`** also removes the global shortcuts `notify hotkey` set up.
- **Cleanup:** the old Python CLI (`legacy/notify-py`), its parity test and the finished planning documents are gone; a new Rust test (`--test lifecycle`) runs takeover, update, portal and restore in a fake home instead. `notify` links the system SQLite instead of building its own. The panel finds its QML files by itself and reads its JSON through one helper; the job bridge reads launchers through `notify-apps`.
- **Daemon:** `shell.qml` (1010 lines) is split into `Config.qml`, `Rows.qml`, `Api.qml`, `Control.qml`, `Preview.qml`, `Sound.qml` and `AutoDnd.qml`; it is now 333 lines and the last file exempt from the 400-line limit is gone. No change in behaviour.

## 0.17.0

- **Dolphin's file operations show again:** copy, move, delete, extract (any KIO job from any KDE app) appear as one notifd popup per job with a live progress bar, speed and time left, **Pause**/**Cancel**, and the result when it ends ("Copied · 10 files, 2.1 GiB · To ~/Downloads", with **Open folder**). Failures stay; cancelled and quick jobs leave nothing. KDE reports these to Plasma's job tracker, not as notifications, so with Plasma's notification applet gone they had nowhere to appear.
- **New `notifd-jobs.service`** (`notify jobs`, crate `notify-jobs`): owns `org.kde.JobViewServer` and `org.kde.kuiserver` (JobViewServer V1 and V2, JobView V2 and V3). It starts before plasmashell, which otherwise claims the names for good. `takeover` installs it, `update` adds it to existing installs (active from the next login), `restore` removes it and restarts plasmashell so Plasma takes job progress back. `status` and `doctor` show who holds the job tracker.

## 0.16.0

- **Groups:**
  - **Zones are now Groups.** Apps and websites belong to a group (Business, Personal, any you create), and the old zone map is now the "Where groups appear on screen" section of the Groups page.
  - **What a group shows:** each group lists its apps and websites as chips. Remove one with ×; add one with "Add app…" (any app, searchable) or "Add website…" (the sites seen in your notifications, or type one).
- **System is locked:**
  - **Always there:** the System group always exists. notifd adds it if the config leaves it out, and it cannot be deleted or renamed.
  - **The system's own notifications always land in System:** the kernel, systemd, power, disks, network, polkit and KWin. No rule can move or silence them, and muting System or a focus mode does not hide them; only Do Not Disturb holds them, and critical ones still pass.
  - **Stock system apps:** they can be taken out of System by adding them to another group, and other apps can be added to it.
- **Rules can apply to a group:**
  - **New condition:** "In group" means every notification in that group (the group decided by the rules that do not test the group). A rule like that changes how the group looks and behaves; it cannot move them to another group.
  - **New template:** "Change how a whole group looks".
  - **Rules still apply to single apps** as before.
- **History, add to a group:**
  - **One click:** opening an entry now offers its group directly.
  - **Keywords:** alongside "from this app", "from this website" and "of this type", you can pick words from the notification ("And mentions: invoice"); title words must be in the title, text words in the text.
  - **Preview:** the preview shows how many recent notifications that would catch.
- **Wording:** "zone" is now "group" across the panel and the tray menu ("Mute a group"). The config file keeps the key `zones`.
- **Daemon:** routing (decide) moved from `shell.qml` into `Route.qml`; a rule's `match` accepts `group`; `notify rules` checks it too.

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
