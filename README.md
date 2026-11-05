# noti-fi

*noti-fi* stands for **notification fidelity**. The command is still called `notify`, and the daemon `notifd`.

A notification daemon for KDE Plasma on Wayland, built on [Quickshell](https://quickshell.org),
and the Rust command-line tool and control panel that install and run it.

- **notifd** owns `org.freedesktop.Notifications`, replacing Plasma's built-in notification service.
  Popups are routed into named **zones** (corners of the screen) by **rules** (app, site, urgency, text).
- **notify** installs it as a user service, switches Plasma over and back, and gives daily controls
  (Do Not Disturb, mute a zone, history, a global hotkey).

## Install from a release

Download the archive from the [releases page](https://github.com/asafelobotomy/noti-fi/releases), check it against the `.sha256` file, then:

```sh
tar xzf noti-fi-*-linux.tar.gz && cd noti-fi-*-linux
./install.sh                 # into ~/.local (PREFIX=/usr ./install.sh for system-wide, run with run0)
notify takeover --dry-run    # see exactly what it will change
notify takeover --now        # make notifd your notification service (undo any time: notify restore)
```

The panel (`notify-panel`) only runs on the Qt version it was built with. The archive records that version, and the installer leaves the panel out when it does not match your Qt. On Arch and Manjaro the reliable way to get the panel is to build it for your machine:

```sh
cd packaging && makepkg -si     # builds noti-fi against your installed Qt (about 8 minutes); run it again after a Qt upgrade breaks the panel
```

Needs at run time: `quickshell`, `qt6-declarative`, `kirigami`, `qqc2-desktop-style` (`qt6-multimedia` is optional, for audio/video players).

## Updating

```sh
notify upgrade           # look for a newer release (contacts github.com, only when you run it)
notify upgrade --apply   # download it, verify its checksum, replace notify (and the panel when its Qt matches), refresh the daemon
```

The panel's General page does the same, and can check once a day if you tick the box (off by default). It works for a copy installed in a folder you can write to (such as `~/.local`); a system-wide install is updated with your package tool.

## Releasing

Bump the version in `Cargo.toml` and `daemonVersion` in `notifd/shell.qml` (a test checks they agree), update `CHANGELOG.md`, commit, then push a tag such as `v0.8.1`.
The `release` workflow builds on Arch, packages the archive with `packaging/build-tarball.sh`, and publishes it with its checksum. The `ci` workflow builds and tests every push and pull request.

## Layout

| Path | What it is |
|---|---|
| `crates/notify-cli` | the `notify` command (Rust). Embeds the daemon files, so `notify update` needs no project folder |
| `crates/notify-protocol` | typed messages for the control socket and the `rules.json` model (no I/O) |
| `crates/notify-client` | blocking Rust client for the control socket (used by the CLI; the future panel will use it too) |
| `crates/notify-testkit` | test harness: a throwaway daemon on a private D-Bus session |
| `crates/notify-apps` | finds apps that send, or could send, notifications (launchers, KDE events, Plasma's list, Flatpak permissions) and merges them with the daemon's `seenApps` |
| `crates/notify-panel-core` | the panel's Qt-free logic: connection worker (reconnects, coalesces events), view data, history filtering |
| `crates/notify-panel` | the control panel app (Rust + cxx-qt + Kirigami): Overview (with the focus switch), General (tray, start at login), History, Apps, Focus modes, Rules, Zones & layout and Appearance so far; `cargo run -p notify-panel` |
| `crates/notify-panel-smoke` | headless tests of the real panel app (asserts what the UI displays) |
| `notifd/shell.qml`, `Popup.qml`, `Media.qml`, `TransferChart.qml` | the daemon (QML on Quickshell): rules, state, control socket, popups; `Media.qml` (audio/video players) needs the optional `qt6-multimedia` |
| `notifd/rules.json`, `rules.schema.json` | default config and its JSON Schema |
| `tests/test_notifd.py` | daemon regression suite incl. socket stress tests (private bus) |
| `tests/parity.py` | proves the Rust CLI does exactly what the old Python CLI did |
| `legacy/notify-py` | the old Python CLI, kept only for the parity test |
| `docs/` | `protocol.md` (control socket), `control-panel-plan.md` (the GUI plan) |

Build and test:

```
cargo build --release -p notify-cli && install -m 755 target/release/notify ~/.local/bin/notify
cargo test --workspace -- --test-threads=1      # unit + contract + CLI integration tests (starts a throwaway daemon)
python3 tests/test_notifd.py                    # daemon suite (or: notify selftest)
python3 tests/parity.py                         # Rust CLI vs the legacy Python CLI
```

Installed copies live in `~/.local/share/notifd/` (daemon), `~/.config/notifd/rules.json` (your config),
`~/.config/systemd/user/notifd*.service`, and `~/.local/bin/notify`.

## Install and undo

```
notify takeover --dry-run   # see exactly what would change
notify takeover             # install; notifd takes over at next login
notify takeover --now       # also switch the running session (panel disappears ~5s)
notify restore              # hand notifications back to Plasma, remove everything installed
```

`takeover` also routes Flatpak/portal notifications through notifd (`notify portal`), arms an automatic
fallback (if notifd keeps failing, notifications are handed back to Plasma instead of being lost),
and records what it created so `restore` removes exactly that.

### Machine-readable output

Every command accepts `--json`. Read commands print one JSON document (`status`, `doctor`, `apps`, `rules`, `simulate`,
`history`, ...). Commands that change things stream **progress events**, one JSON object per line:

```
{"type":"plan","name":"takeover","dry_run":false,"total":11,"steps":[{"n":1,"desc":"..."}, ...]}
{"type":"step","n":1,"total":11,"desc":"...","status":"running"}
{"type":"step","n":1,"total":11,"desc":"...","status":"done"}          (or "failed" with an "error")
{"type":"result","ok":true,"message":"..."}
```

A failure always ends with `{"type":"result","ok":false,"message":...}` and exit code 1. The control socket protocol that
the daemon speaks (for tools and the planned GUI) is in [docs/protocol.md](docs/protocol.md).

## Configuring

Edit `~/.config/notifd/rules.json`; it reloads live. A file that fails validation is **rejected as a whole**:
the last good config keeps running, a red pill shows the reason, and `notify status` reports it.
Check a file before saving with `notify rules --file PATH`.

- Rules are checked top to bottom; for each of `zone`, `timeout`, `silent` the **first matching rule that sets it wins**.
- `match` fields (all regexes, case-insensitive): `appName`, `desktopEntry`, `origin` (site name from browsers), `summary`, `body`, `urgency`.
- Regexes are JavaScript regexes (no `(?i)`, no `(?P<name>...)`); `notify rules` flags those.
- `timeout`: milliseconds, `0` = stays until dismissed. An app's own `-t 0` is honoured too.
- `maxVisible`: popups shown per zone; extras queue and a "+N more waiting" chip appears.
- `theme`: colours. `sound`: honours `sound-file`/`sound-name`/`suppress-sound`; `defaultName` plays a theme sound for everything.
- `history.persist: true` keeps history across restarts (off by default so message text is never written to disk).

## Checking and previewing rules (the daemon decides)

```
notify simulate "New message" --app "Brave Origin" --origin example-shop.com   # what would happen, and which rule decided it
notify simulate x --app Steam --config draft.json                          # ... against a draft, without applying it
notify rules --file draft.json                                             # static lint + the daemon's own verdict
notify preview --config draft.json --seconds 8                             # show REAL sample popups with the draft theme/zones
```

`simulate` and `preview` run through the same `decide()` code as real notifications, so the answer is what will happen.
A draft is never applied: `preview` swaps it in for a few seconds and then restores your config.
`notifd/rules.schema.json` describes the file's structure for editors; `notify status|rules|apps --json` give structured output.

## Daily use

```
notify dnd toggle        notify mute business     notify history --missed
notify send "Hi" -a Steam                         notify test
Meta+Shift+N             (after: notify hotkey)   toggles Do Not Disturb
```

## Safety properties (all covered by `notify selftest`)

- Notification text is untrusted: titles are plain text; bodies keep only `<b> <i> <u> <br>`; images/links/other
  tags are dropped and icons are limited to local files and theme names, so a notification cannot make this
  machine fetch a URL.
- A bad config never drops notifications; a failing popup path falls back to a plain popup.
- `notify update` keeps the previous version and rolls back automatically if notifd does not come back healthy.
- Replaced notifications (progress, chat) refresh their timer and update one history entry.

## Developing

```
notify dev                # throwaway daemon from this checkout on a private bus (Ctrl+C to stop)
notify --dev test         # drive it from another terminal
notify selftest           # daemon regression suite; popups flash on screen briefly
notify update --payload notifd   # deploy the checkout's daemon files instead of the embedded ones (smoke-tested, with rollback)
```

## Automatic Do Not Disturb, replies, tray

- **Full-screen windows:** `notify autodnd install` adds a small KWin script that logs whether the active window is
  full screen; notifd follows that line from the journal and holds normal popups until it ends (critical ones still
  show; held ones are in `notify history --missed` with the reason). Turn off with `"autoDnd": {"fullscreen": false}`.
- **Screen sharing:** notifd watches PipeWire for a KWin screen-capture stream and holds popups the same way
  (`"autoDnd": {"screenShare": false}` to disable). *The detection signature has not been confirmed against a real
  share yet; see the notes in `notifd/shell.qml` (`shareScript`).*
- **Inline reply:** notifications that offer a reply (chat apps) get a reply box. Click it, type, press Enter
  (Escape dismisses). The popup takes keyboard focus only when you click into the box. `notify reply ID TEXT` does the same from a terminal.
- **Tray:** `notify tray hide` removes Plasma's own (now inert) notification icon from the system tray; `notify tray show` brings it back.
  Plasma 6 runs its notification service *inside* that icon's applet, so `notify restore` (and the automatic fallback) show it again
  before handing notifications back; otherwise Plasma could not take them over.

## Known limits

- Plasma's per-app notification settings page no longer applies (notifd has its own rules).
- Popups appear on one screen (`"screen"` in rules.json picks which).
- Screen-share detection is unconfirmed (above).

## Planned

A Rust + Kirigami control panel; see [docs/control-panel-plan.md](docs/control-panel-plan.md). Phases 0-4 (daemon/CLI groundwork, toolkit spike, the shell with Overview and History, the Rules editor with simulator, Zones and Appearance) are done; see `docs/phase1-toolkit-spike.md`.

## Licence

MIT. See [LICENSE](LICENSE).
