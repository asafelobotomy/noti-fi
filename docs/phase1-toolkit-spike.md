# Phase 1: toolkit spike (cxx-qt + QML + Kirigami)

**Result: GO.** cxx-qt with Kirigami does what the plan needs, on this machine, with a small and stable Rust/QML boundary.
The Slint comparison was not built: the spike succeeded on every criterion, so there is no reason to spend a day on the fallback.

## What was built

`crates/notify-panel` (about 700 lines in total: 280 Rust, 420 QML). A real window titled "notify" with app id `org.notify.Panel`:

- **Overview**: a Do Not Disturb switch (with the hold reason: manual / full screen / screen sharing), one card per zone with its
  accent colour, popup count and mute switch, a "N on screen, M waiting" line, a "notifd is not running" banner.
- **History**: live list, search filter, "Missed only", "Show message text" toggle (off by default: message text is hidden),
  "Clear history", orange "held: dnd" badges.
- A Rust QObject (`PanelModel`) is the only thing QML talks to. A background thread owns the `notify-client` connection,
  subscribes to events, reconnects every second if the daemon goes away, and executes commands sent from the UI.

## Evidence (all measured on this machine)

| Check | Result |
|---|---|
| Looks native on Plasma | Yes: Breeze dark, your accent colour, system fonts, Kirigami sidebar (screenshots taken) |
| Live data from the daemon | Yes: DND, mutes and popup counts updated within 1.5 s of a CLI change |
| Panel -> daemon commands | Yes: a scripted run (`--selftest`) made the daemon go DND on, muted business, unmuted, DND off, exactly in order (captured from the daemon's events) |
| Accessibility | AT-SPI tree shows headings, labelled switches (`Do Not Disturb`, `Mute Business`...), list items and labels |
| Release binary | 2.8 MB (2.0 MB stripped); links the system Qt (12 libraries) and Kirigami at run time |
| Memory | 168 MB resident, 73 MB unique (rest is shared Qt). For scale: notifd itself is 211 MB |
| Idle CPU | 0.1 % over 10 s, 13 threads |
| Build time | cold release build of the crate: 3 min 2 s; debug cold: 2 min 32 s; **incremental rebuild after editing Rust or QML: about 10 s** |
| Disk | the shared `target/` folder is 1.9 GB (Qt glue is large; it is build output only) |

## What worked well

- **A thin boundary is enough.** The model exposes only bools, ints and strings; lists travel as JSON strings that QML parses
  (`JSON.parse(model.historyJson)`). No `QAbstractListModel` was needed for 200 history rows or for the rules list, and the
  Rust side stays plain, testable Rust.
- The typed client and protocol crates dropped straight in (no changes needed), as did `serde_json`.
- `qt_thread().queue(...)` makes the worker-thread-to-UI hand-off a one-liner; there was no manual locking anywhere.
- Kirigami gives page navigation, sidebar, search field, placeholder messages and inline errors for free.

## Friction found (write these down before Phase 2)

1. **`#[auto_cxx_name]` is per block.** Put it on every `extern "RustQt"` block, or invokables keep their snake_case names and
   QML silently calls methods that do not exist (no warning was printed). Mitigation: a smoke test that calls every invokable,
   and a QML-side check at startup (`typeof model.setDoNotDisturb === "function"`).
2. **Sparse documentation for 0.10.** The API changed between releases; the reliable reference was the example inputs shipped in
   `cxx-qt-gen/test_inputs` in the crate sources. Pin the version and keep the bridge file small and commented.
3. **C++ warnings from GCC 16 inside Qt's own headers** (`-Wsfinae-incomplete`). Harmless, but noisy; silence with a build flag in CI.
4. **Cold builds are slow** (about 3 min). Incremental builds are fast, so this mostly affects CI and first checkout.
5. **QML binding gotcha (not cxx-qt specific):** a `Switch` bound to a model property loses its binding after the user clicks it.
   The Overview page restores it (`checked = Qt.binding(...)`) so the switch always follows the daemon, not the click.
6. Page wiring: Kirigami pages must be pushed with their properties (`pageStack.push(url, {model: model})`); setting properties
   on an `initialPage` URL does nothing.

## Risks that remain

- cxx-qt is pre-1.0 (0.10). A new release may need small edits to the bridge. Mitigated by the thin boundary.
- Needs system packages at run time: `qt6-declarative`, `kirigami`, `qqc2-desktop-style` (already on this machine; goes into the PKGBUILD).
- Not yet checked: HiDPI/fractional scaling, keyboard-only navigation by hand, a headless (`QT_QPA_PLATFORM=offscreen`) UI test.
  All three are Phase 2 / Phase 7 items.
- Very large lists (thousands of rows) would want a real list model; not a concern for history (capped at 200) or rules.

## Recommendation for Phase 2

1. Keep the split: **QML = layout only; one Rust view-model per page**; shared state in one `AppState` the pages read from.
2. Add the `--selftest` and `--page=` hooks to the real app (they proved useful) and a headless smoke test that starts a throwaway
   daemon (the testkit already does), launches the panel offscreen, and asserts on daemon-side effects.
3. Add a startup check that every invokable the QML uses exists.
4. Build the Rules editor next, using `simulate` and `validate` from the daemon for live feedback, as planned.
5. Packaging stays in Phase 6.

## Addendum: lessons from Phase 2 (the real shell)

These cost real time; they are the reason the shell has the test setup it has.

1. **A package with a cxx-qt bridge cannot link its own tests.** The build script's Qt glue is linked into every target of the
   package (including unit and integration tests) and needs symbols only the binary has. So: Qt-free logic lives in
   `notify-panel-core` (normal tests), and tests of the real app live in `notify-panel-smoke`, which runs the binary as an external program.
2. **Qt logs to the systemd journal, not stderr, when it can** (it detects `JOURNAL_STREAM`). QML warnings and `console.log` silently
   vanish from a test's captured output. Tests set `QT_FORCE_STDERR_LOGGING=1`, `QT_LOGGING_RULES=qml.debug=true` and remove
   `JOURNAL_STREAM`. (This is also why the Phase 1 "missing method" bug printed nothing.)
3. **QML name shadowing.** In `OverviewPage { model: model }` the right-hand `model` is the page's own property, so it bound to itself and
   stayed undefined: the UI showed "Not connected" while the Rust state was perfectly fine. Give shared objects distinct ids
   (`panelModel`, `uiSettings`).
4. **Test what the user sees, not just state.** The app now prints a `UI {...}` line on exit (`--report`) describing what the current page
   displays (banner text, subtitle, switch, number of zone cards, history summary), and the smoke tests assert on it.
5. **Bindings like `visible: page.model && page.model.connected` can evaluate to `undefined`** and warn ("Unable to assign [undefined] to bool").
   Use an explicit boolean property. The smoke tests fail on any QML warning that carries a source location, except one known Kirigami quirk
   ("Created graphical object was not placed in the graphics scene", printed by its page row; the pages display correctly).
6. `--screenshot=FILE` renders only the panel's own content (from inside the app), so screenshots can never include other windows.
   It needs a real display (the grab does not complete offscreen) and has a 4-second give-up timer.

## Addendum: Phase 3 (Rules editor)

- The panel edits a **draft** (`notify-panel-core::draft`) held in Rust; every edit is sent to the daemon for `validate` and `simulate`, so the
  simulator shows what the daemon will really do. Nothing touches `rules.json` until Save; saving keeps a `.bak`, refuses an invalid draft, and
  detects a hand edit made since loading (Overwrite or Reload).
- QML tip: keep rows in a `ListModel` updated in place (`set`/`append`/`remove`), and bind text fields with
  `Binding on text { when: !activeFocus }`, otherwise typing is interrupted each time the model refreshes.
- Test hooks: `--selftest-rules` drives every rules command; add `--selftest-save` to let it write (to `$NOTIFD_RULES`, never the real file in tests).

## Addendum: Phase 4 (Zones & layout, Appearance & behavior)

- Same draft, same Save/Revert/Preview as the Rules page, so one Save covers edits made on any editor page. Settings are edited by dotted path
  (`theme.card`, `maxVisible`, `sound.enabled`); only a fixed list of known settings is accepted, and other keys in the file survive untouched.
- Zones: add, set name/corner/accent, delete. Deleting is refused for the default zone and for the last zone; deleting a zone in use clears it
  from the rules that pointed to it. The mini screen moves the selected zone by clicking a corner (no drag).
- QML gotchas hit here: a custom property named `anchors` or `top`/`left` on an `Item` clashes with Item's FINAL properties and the whole
  page fails to load (the app then keeps running with no window). The smoke harness now kills a panel after 90 s so such a failure is a test
  failure, not a hang. Bind colours through a helper that falls back to `transparent`, since values are undefined while offline.
- Hooks: `--selftest-zones` (add `--selftest-save` to write to `$NOTIFD_RULES`).

## Addendum: the Apps page

- **Who sends**: the daemon keeps a registry of every app that ever sent it a notification (`seenApps`, stored in `state.json` even when history is off).
- **Who could**: `notify-apps` reads installed launchers (`X-GNOME-UsesNotifications`, messaging/email categories), KDE `.notifyrc` files, Plasma's `plasmanotifyrc` app list
  and Flatpak notification permissions. It never reads inside an app's own data. Apps are matched by desktop entry first, then by name.
- **Changing an app**: muting, zone and timeout are one managed rule per app, named `App: <name>`, placed above the user's own rules and removed when it no longer does anything.
  A Flatpak app's own permission is changed immediately through `flatpak permission-set/-reset`. Other apps keep their own settings (browsers: `notify adapt`).
- Gotchas: never strip `.desktop` repeatedly from a launcher name (`org.telegram.desktop.desktop`); a nested layout fills spare height by default
  (`Layout.fillHeight: false`); a delegate cannot declare a required property called `id` or `icon`. Test hooks: `--selftest-apps`, and `NOTIFY_APPS_ISOLATED=1` + `XDG_DATA_DIRS` to scan only a fixture folder.

## Addendum: zone placement (3x3 grid and free)

- A zone's `anchor` is one of nine places on a 3x3 grid (`top-left`, `top-center`, `top-right`, `middle-left`, `center`, `middle-right`, `bottom-left`,
  `bottom-center`, `bottom-right`) or `free`. A `free` zone also has `x` and `y` (0..1 of the screen from the left and top; both required).
  A free zone is placed by the card's left edge; in the upper half (`y <= 0.5`) `y` is its top edge and popups stack downward, in the lower half `y` is its
  bottom edge and they stack upward. Fractions, not pixels, so a layout survives a resolution change.
- Daemon: an axis left unanchored in layer-shell is centred, which is how the middle row and middle column work. Panel: click a cell to move the selected zone,
  or drag any zone chip on the mini screen to make it free.
- Not verified by a test: how the popups look on a real compositor (the tests show the popups and check for script errors; the placement itself needs eyes).

## Addendum: look and behaviour of popups

New keys, usable on a **rule**, on a **zone** and (opacity, fontFamily, fontSize) on the **theme**:

| key | meaning |
|---|---|
| `opacity` 0.1–1 | background transparency (text stays solid) |
| `clickThrough` | clicks go to the window underneath; such a popup cannot be dismissed or pinned by clicking |
| `noFocus` | never takes keyboard focus; the reply box is hidden |
| `width` 120–4000 | popup width in px |
| `fontFamily`, `fontSize` | text (body size; the title is 2 larger) |
| `group` (rules only) | popups with the same group in one zone are kept together under a heading |

- **Resolution**: per key, the first matching rule that sets it wins (an explicit `false` counts), then the zone, then the theme / global width. A rule can therefore set only
  what it cares about. Note that a general rule's `clickThrough: true` also reaches the more specific kinds of that app unless a rule above it sets `false`.
- **New match fields**: `category` (the app's freedesktop category hint, e.g. `email.arrived`) and `channel` (`app`, `portal`, `web`). "Kinds" an app sends are captured in `seenApps`.
- **On the popup itself**: a **Pin** button keeps a popup until dismissed; dragging its inner edge resizes it for as long as it lives. Click-through is implemented with per-popup
  regions subtracted from the zone window's input mask (the window is as wide as its widest popup).
- **In the panel**: Rules (look section + simulator "Look" line), Zones (per-zone look), Appearance (theme opacity/font), Apps (per app and per kind of notification, plus a
  list of the ways notifications can arrive and what they can do). Per-app and per-kind settings are managed rules named `App: ...`; the more specific rule goes first.
- Gotchas: a `ListModel` role called `width` or `opacity` clashes with Item properties (use `cardWidth`, `bgOpacity`); `Variants` silently drops equal values, so give
  each region a distinct index; the typed `Config` in `notify-protocol` is strict, so every new daemon key must be added there too or the panel stops parsing the daemon's config.
- Not checked by a test: how pinning, resizing by hand, click-through and transparency look and feel on a real compositor.

## Addendum: focus, priority, stacks, media and transfer graphs

- **Priority** (`low`, `normal`, `high`, `urgent`) is a style key like the others (rule, zone). Without one it follows the app's urgency (critical = urgent, low = low). Urgent passes Do Not Disturb
  (as critical did) and every focus mode; high is marked on the popup; more important popups stay above less important ones in a zone.
- **Focus modes** (`focus.modes.<name>` = `{label, minPriority, allow[]}`) are runtime state (`notify focus <name>|off|list`, the Overview switch, status `focus`), saved in `state.json`.
  While one is on a notification is shown only if its priority >= `minPriority` or it matches an `allow` entry (a set of patterns like a rule's `match`); the rest is held with the reason
  `focus <name>` and stays in History. No schedules yet.
- **Stacking** (`stack: true` on a rule or zone): notifications with the same stack key (the group name, else site/app + category) in one zone share one place, shown as the newest with a count badge and
  a faint pile. Clicking it expands the stack; an expanded stack shows as many notifications as a zone does (`maxVisible`) and the mouse wheel scrolls the rest; the header has Collapse and Clear all.
  `status.stacks`, `apiStack` and `notify`'s client expose the same operations.
- **Media**: a large picture is shown when a notification's image is at least 200 px wide (smaller images stay as the icon). A notification with the hint `x-notify-video`, `x-notify-audio`,
  `x-notify-media` or `x-kde-urls` pointing at a *local* audio/video file gets a player (never autoplays; remote URLs are never loaded; popups do not expire while playing). It needs `qt6-multimedia`;
  without it only the player is missing. Players are **off unless a rule (or zone) sets `media: true`** (a notification can name any local file); `media: false` turns pictures and players off.
- **Transfer graphs**: a notification with a progress value (`value` hint) or a speed in its text (`3.2 MB/s`) is sampled on each update; 2 or more samples make the popup expandable. The graph shows bytes per
  second when speeds are known, otherwise percent per second, as a line graph, bars or a heatmap strip (one cell per measured interval, exact value on hover). `transfer.chart` sets the default; each popup can switch.
- Gotchas: Quickshell only loads QML files that exist in the payload folder, so new files (`Media.qml`, `TransferChart.qml`) must be listed in the CLI payload, the test harnesses and `notify update`;
  `qml6` can render `Popup.qml` offscreen with stand-in notifications, which is how the popup visuals are checked without touching the real screen.

## Addendum: system tray, start at login, one panel, and the 400-line rule

- **Tray**: Qt's own tray support is not usable here (`Qt.labs.platform`'s menus need Qt Widgets, and the KDE platform theme aborts when it makes a tray icon without a `QApplication`). The icon and its menu are
  therefore a StatusNotifierItem spoken directly over D-Bus with the `ksni` crate (`notify-panel-core::tray`, Qt-free, the menu built from a plain `TrayState` and unit-tested). Left click shows or hides the window, middle click
  toggles Do Not Disturb, and the menu has the status line, Do Not Disturb, Focus (Off + modes), Mute a zone, Recent notifications (app and age only, titles only when "Show message text" is on), Show all history, Settings and Quit.
  The icon changes with the state (normal, Do Not Disturb, Focus, notifd not running). It starts on its own thread so a slow tray host cannot delay the window; `NOTIFY_PANEL_NO_TRAY=1` turns it off (the tests set it so they never touch your real tray).
- **Close and minimise to tray** (General page): closing the window keeps the panel running in the tray when there is one; Quit in the tray menu really exits. Minimise-to-tray only works where the desktop reports minimising to the
  window; Plasma on Wayland does not, so it is a no-op there (the page says so).
- **Start at login** (General page): writes `~/.config/autostart/org.notify.Panel.desktop` (`Exec=<this program> --background`, so it starts hidden in the tray) plus an app-menu launcher. The path is the running program's, so install the
  panel where it will stay (for example `~/.local/bin/notify-panel`) before turning it on.
- **One panel at a time**: a second launch (app menu, login) asks the running one to show its window over `$XDG_RUNTIME_DIR/notify-panel.sock` and exits; a stale socket from a crash is replaced.
- **400 lines per file**: `cargo test` fails if a `.rs`, `.qml`, `.py` or `.sh` file (outside `legacy/`, `target/`) exceeds 400 lines, unless it is listed in `tests/loc-baseline.txt` with the length it must not exceed. The list is a ratchet:
  entries may only shrink, and a file that gets within the limit must be removed from the list. Splitting an oversized file is done by moving code, not by raising its number. (`model.rs` was split into `model/edit.rs`
  and `model/glue.rs`, `Main.qml` into `SelfTest.qml`, and the smoke tests into three files plus `common/`.)

## Addendum: review fixes (security and performance)

- **No `/tmp` fallback**: without `$XDG_RUNTIME_DIR` the daemon opens no control socket (notifications still work), the clients' socket path points nowhere and says which variable is missing, and the panel skips its one-at-a-time check.
- **Test hooks are fenced**: the `--selftest*` hooks change Do Not Disturb, mutes and history and can write `rules.json`, so they refuse to run unless `NOTIFY_PANEL_TEST=1`, and refuse to save unless `NOTIFD_RULES` points elsewhere.
  (`NOTIFY_PANEL_NO_TRAY=1` keeps the tray off. Running the offscreen test platform with the tray on does not exit cleanly, which is another reason tests turn it off.)
- **Players are opt-in**: pictures show by default, audio/video players only where a rule or zone sets `media: true`.
- **Pattern limits**: patterns are at most 500 characters and only the first 2000 characters of a text are tested, so a badly backtracking pattern cannot stall the thread that draws popups much.
- **Service**: `notifd.service` sets `NoNewPrivileges`, `UMask=0077`, `LockPersonality` and `RestrictSUIDSGID`. `notify dev` and the update smoke test pass paths to `bash -c` as arguments, not by pasting them into the script.
- **Performance**: a panel with only its tray icon showing unloads its pages and refreshes just 8 history entries instead of 200; the daemon updates its seen-apps registry in place and writes it 5 s after the last change, forgets the state
  of stacks that no longer exist, and shares one empty sample list instead of making one per popup on every sample.
- `Validate.qml` now holds the daemon's configuration checks (it was in `shell.qml`); like `Media.qml` it must be in the payload list.

## Addendum: updates

- `notify-update` (Qt-free, tested with a fake download source) finds the latest release through GitHub's API, accepts only HTTPS addresses under this project's release path, checks the archive against the `.sha256` file published with it
  (this catches a damaged download, not a compromised account), unpacks it and replaces each file by writing a new one and renaming it over the old. It checks that both programs are in the archive before replacing anything.
- `notify upgrade` reports, `notify upgrade --apply` installs (it refuses early, before any network use, when the running copy is not in a writable `bin` folder, for example a build in `target/`), then runs the new `notify update` to refresh the daemon.
- The panel runs the same `notify upgrade --apply`; after it, **Restart noti-fi** starts a fresh copy a second later and quits (Linux reports a replaced running program as "(deleted)", which is stripped). The daily check is an opt-in setting and defaults to off.

## Addendum: the panel is tied to the Qt it was built with

- `qmlcachegen` (always on in `cxx-qt-build` 0.10) compiles the QML into the binary using Qt's *private* interfaces, which differ between Qt minor versions. The first releases were built on Arch (Qt 6.12.0) and the panel failed to start on Manjaro (Qt 6.11.2)
  with `undefined symbol: ...QQmlPrivate::AOTLookupValidation...`. The command-line tool has no Qt dependency and is unaffected.
- The release archive therefore contains `qt-version`; `install.sh` and `notify upgrade --apply` replace the panel only when that is the same major.minor as the installed Qt and the installed patch is not older, otherwise they update everything else and print why.
  `packaging/PKGBUILD` is the supported way to get a panel that matches your machine; rebuild it after a Qt upgrade.

## Addendum: a panel that does not depend on the Qt minor version

- `crates/notify-panel/build.rs` sets `QMAKE` to a wrapper that reports a stand-in `qmlcachegen` (every other Qt tool is symlinked from the real libexec folder). The stand-in writes the output file it is asked for, empty, plus the one function the module's plugin code refers to
  (`qInitResources_qmlcache_org_notify_panel`). The QML is then loaded from the binary's resources and compiled at start-up instead of being compiled ahead of time, which removes every `Qt_6_PRIVATE_API` symbol from the binary (check: `nm -D --undefined-only notify-panel | grep -c PRIVATE_API` is 0).
- `packaging/build-tarball.sh` writes `portable-qml` into the archive only when that check passes; the updater and `install.sh` then accept the panel on any Qt 6 and fall back to the strict same-minor rule when the marker is absent.
- `NOTIFI_QML_AOT=1` builds the old way. Still unverified: that a panel built on a newer Qt runs on an older one (the release workflow builds on Arch's newest Qt; the first release built this way is the test on a Qt 6.11 machine).

## Addendum: time-left indicators

- `timerStyle` is a style key (theme, zone, rule, app, kind) holding `none` or any of `line`, `circle`, `frame` separated by commas; per key the first matching rule wins, then the zone, then the theme (default `none`). A popup counts its own elapsed time
  in 40 ms steps (`elapsed`, using timestamps so a busy machine does not stretch it) and exposes `remaining` (1 to 0). The footer line and frame are `TimerIndicators.qml`, the ring is `TimerRing.qml`; nothing is drawn for a persistent popup (timeout 0) or a pinned one.
- The frame is drawn by tracing the rounded outline segment by segment (Qt Quick's canvas ignores `setLineDash`).
