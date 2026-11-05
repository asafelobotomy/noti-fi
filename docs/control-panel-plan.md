# Rust control panel for noti-fi — plan

Status: **Phase 0 is complete.** All six decisions are made (section 10). The daemon has `validate` / `simulate` / `preview`
and a control socket with live events; the CLI is ported to Rust with `--json` and step-by-step progress events; a typed Rust
client exists and is contract-tested against the real daemon. **Phase 1 (toolkit spike) is also done: GO for cxx-qt + Kirigami**, see `docs/phase1-toolkit-spike.md`. **Phase 2 (shell, Overview, History) and Phase 3 (Rules editor with simulator, live validation, preview, conflict-safe save) are done. Phase 4 (Zones & layout, Appearance & behavior) is done too.** Next: Phase 5/6 (Apps and System are v1.1; packaging is Phase 6).

## 1. Goals and non-goals

**Goals**
- A native-feeling desktop app to do everything `notify` does today, without the terminal: see live state,
  toggle Do Not Disturb / mute zones, browse history, edit rules and zones and theme, check app/browser
  readiness, and install / update / restore the system.
- Safe by construction: it can never save a config the daemon would reject, and it shows *what a rule will do*
  before you save it.
- One source of truth for behaviour. The panel does not re-implement the rule engine or the popup renderer.

**Non-goals (v1)**
- Replacing the Python `notify` CLI (the panel calls it for install/maintenance; see 3.4).
- Remote or multi-user management, a notification *center* (a persistent inbox), cloud anything.
- Non-Plasma desktops.

## 2. What exists today

**Daemon (`notifd`, QML on Quickshell)** exposes these over `qs ipc call notifd <fn>` (string in, JSON string out):
`status`, `configStatus`, `dnd`, `mute`, `history`, `clearHistory`, `dismissAll`, `setAuto`, `reply`.
Config is `~/.config/notifd/rules.json`, hot-reloaded and validated by the daemon.

**CLI (`notify`, Python)** owns lifecycle: `takeover`, `update`, `restore`, `portal`, `adapt`, `hotkey`,
`autodnd`, `tray`, plus `doctor`, `apps`, `rules`, `selftest`, `dev`.

**Gaps a GUI runs into**
| Gap | Why it matters |
|---|---|
| Only `history` has `--json`; `status`, `doctor`, `apps`, `rules` print text | The panel needs structured data, not screen-scraping |
| Every IPC call spawns a `qs` process (~50-100 ms) and there are no push events | Fine for polling at 1 Hz, poor for a live view |
| Validation exists three times (QML, Python lint, soon Rust) | They will drift; the daemon is the only authority |
| No way to ask "what would this rule do to *this* notification?" | The single most useful thing a rules editor can show |
| No way to preview a draft theme/zone layout | Users would have to save, look, and undo |
| No JSON Schema for `rules.json` | Forms and editors can't be generated or checked |

## 3. Architecture

```
 ┌────────────────────┐   control socket (JSON lines, events)   ┌────────────────────┐
 │  notify-panel      │ ◄──────────────────────────────────────►│  notifd (QML)      │
 │  Rust + UI         │                                         │  rules, popups     │
 └─────────┬──────────┘                                         └────────────────────┘
           │ subprocess, --json                                           ▲ hot reload
           ▼                                                              │
 ┌────────────────────┐        atomic write                      ~/.config/notifd/rules.json
 │ notify (Python CLI)│  takeover / update / restore / adapt …
 └────────────────────┘
```

### 3.1 Daemon-authoritative: simulate, validate, preview
Add three daemon methods and make the panel a thin client of them:
- `validate(config)` — run the daemon's own validation on an *unsaved* draft; returns the first error or ok.
- `simulate(config?, notification)` — run a hypothetical notification through the real rule engine; returns
  `{zone, timeout, silent, matchedRules[], hold, wouldShow}`. Powers the "Test this rule" box and a live preview
  column next to the rules list.
- `preview(config?, samples[], seconds)` — show *real* popups on screen using a draft theme/zones, then revert.
  The panel needs no popup renderer of its own, so what you preview is exactly what you get.

This removes the biggest duplication risk: no regex engine, matcher or popup code in Rust.
Offline fallback (daemon not running): a read-only mode plus structural checks from the JSON Schema; regex
semantics via the `regress` crate (ECMAScript syntax) are only used for a "best effort" warning.

### 3.2 Control channel
Add a Unix socket to the daemon (`$XDG_RUNTIME_DIR/notifd/control.sock`, directory 0700) using Quickshell's
`SocketServer`. Protocol: one JSON object per line.
- request `{id, method, params}` → response `{id, result}` or `{id, error}`
- server-pushed events: `state` (dnd/mute/auto/holding), `notification` (shown/held, with the same fields as
  history), `config` (loaded/rejected + reason)
- first message from the server is `hello {protocol, daemon_version}`; the panel refuses to run against an
  incompatible protocol and says so in plain words.
Keep `qs ipc` working unchanged for scripts and the CLI.

### 3.3 Config editing
The panel edits `rules.json` itself with atomic writes (temp file + rename); the daemon hot-reloads. Before saving,
it calls `validate`. It records the file's hash when loaded and warns if the file changed underneath it
(someone edited by hand), offering reload or overwrite. A `rules.schema.json` (JSON Schema 2020-12) is shipped
next to `rules.json`; the Rust side uses it to build forms and to report structural errors with paths.

### 3.4 Lifecycle operations
For v1 the panel runs the Python CLI as a subprocess for `takeover`, `update`, `restore`, `portal`, `adapt`, `hotkey`,
`autodnd`, `tray`, always with `--dry-run` first to show the plan, then for real, streaming progress. This needs a
`--json` mode on those commands (step-by-step events and a final result). Porting the CLI to Rust is deferred:
it is large, already tested, and would add risk without helping the panel. Revisit after v1 (see risks).

### 3.5 Crates (Cargo workspace next to `notifd/`)
- `notify-protocol` — serde types for the control protocol and the config model; no I/O. Shared by everything.
- `notify-client` — async client for the socket with reconnect, event stream, and a `qs ipc` fallback.
- `notify-ops` — wrappers that run the CLI with `--json` and turn output into typed progress.
- `notify-panel` — the application (UI + view-models).
Dependencies I would expect: `serde`, `serde_json`, `tokio`, `thiserror`, `jsonschema`, `directories`; UI crate per 4.

## 4. Toolkit decision

| | cxx-qt + QML (Kirigami) | Slint | iced | GTK4 + libadwaita | egui |
|---|---|---|---|---|---|
| Looks native on Plasma (Breeze, your accent, fonts) | **Yes** | Partly (Qt style) | No | No (foreign look) | No |
| Accessibility (screen reader, keyboard) | **Good** (Qt AT-SPI) | Improving (AccessKit) | Weak | Good | Weak |
| Rust owns the logic | Yes (UI is QML) | **Yes, fully** | **Yes, fully** | **Yes, fully** | **Yes, fully** |
| Already on this machine | **Yes** (Qt 6.11, Kirigami 6.30) | crate only | crate only | Yes (GTK 4.22) | crate only |
| Maturity / churn risk | Medium (cxx-qt 0.10, pre-1.0) | Medium-low (1.18) | Medium (0.14) | Low | Medium |
| Build complexity | Higher (C++ glue, Qt toolchain) | Low | Low | Low | Low |

**Recommendation: cxx-qt + QML with Kirigami.** It is the only option that behaves like a first-class Plasma app
(theming, HiDPI, accessibility, shortcuts), and QML is a language you already have in this project. Because
section 3.1 removes the need for the panel to render popups or match rules, the usual argument for pure-Rust UI
("keep logic in one language") is weaker here: nearly all logic is in the daemon already.
**Runner-up: Slint**, if the cxx-qt build or API churn proves painful. **Not recommended:** iced and egui (not native,
accessibility is weak for a settings app), GTK (works, but looks out of place in Plasma).
A short **toolkit spike** (section 8, step 1) builds the same screen in the top two before committing.

## 5. Screens

1. **Overview** — big Do Not Disturb switch, current hold reason (manual / full screen / screen sharing), per-zone
   mute switches with live counts, "popups on screen", health badge (owner, service, fallback armed, config OK).
2. **History** — live list, filter by zone/app/site, "missed while held" tab, text hidden by default.
3. **Rules** — ordered list (drag to reorder), edit panel with fields (app, desktop entry, site, title, body,
   urgency) and outcome (zone, timeout, silent). Right-hand **Simulator**: type or pick a notification, see which
   rules match, the outcome, and a green/red validity line. "Preview on screen" button.
4. **Zones & layout** — a mini screen showing the four corners; drag a zone to a corner, set colour/label, `maxVisible`,
   width, margin; choose the output (screen).
5. **Appearance** — theme colours, sizes, sound options; live "preview on screen".
6. **Apps & browsers** — readiness table from `apps` (Brave push on/off, background mode, Firefox, Flatpak portal
   routing) with the `adapt` actions behind a confirm dialog (preview → apply, "close Brave first" guidance).
7. **System** — install / update / restore with the dry-run plan shown first; portal routing; full-screen KWin script;
   tray icon; global shortcut; `doctor` results; daemon log tail.

Everything destructive shows the exact plan first (same rule as the CLI). No action is silent.

## 6. Data model and validation
- `rules.schema.json` is the structural contract (types, enums, required, ranges); keep it next to `rules.json` in the repo
  and test that the shipped defaults validate against it.
- Semantics (regex validity, zone references) are the daemon's `validate`. The panel never decides these itself.
- Differential tests: a corpus of (config, notification) → expected outcome is run through the daemon's `simulate`
  in CI (reusing the private-bus harness in `tests/`), so any change to rule behaviour is caught.

## 7. Privacy and security
- History contains message text. The panel hides bodies by default, never writes them to disk or logs, and says
  when persistence is on.
- Control socket is user-only (0600 in a 0700 runtime dir). No network access anywhere in the panel.
- No privileged operations: everything is user-level. The one `run0` use (managed browser policy, if ever added)
  would be an explicit, separate, confirmed step.
- The panel treats daemon output as untrusted for display (plain text only), same rule as the popup.

## 8. Phases
Estimates are rough, for one person working part-time.

| # | Phase | Deliverable / acceptance | Est. |
|---|---|---|---|
| 0 | **Backend prep** | `rules.schema.json`; daemon `validate` / `simulate` / `preview`; control socket with events; `--json` on `status`, `doctor`, `apps`, `rules`, lifecycle commands; contract tests in the existing harness | 4-5 days |

Phase 0 progress:
- [x] `notifd/rules.schema.json` (JSON Schema 2020-12); shipped with the daemon; tests check the shipped config against it
- [x] one shared `decide()` in the daemon used by real notifications, the simulator and previews
- [x] daemon `validate`, `simulate`, `preview`, `endPreview` (IPC) and CLI `notify simulate`, `notify preview`, `notify rules --file` (daemon verdict, never applied)
- [x] `--json` on `status`, `rules`, `apps`
- [x] tests: simulator-vs-reality differential (8 scenarios), validate-does-not-apply, preview lifecycle, schema; 84 checks total
- [x] `--json` on every command; step-by-step progress events on `takeover` and `restore` (and failures as a final `result` event)
- [x] control socket with `hello` handshake and pushed events; soak/stress tests (1000 pipelined requests, connection churn without fd/memory growth, 400-event flood, a client that never reads, p99 latency, stale-socket recovery)
- [x] Rust crates `notify-protocol` and `notify-client`; contract tests: a typed Rust client against the real private-bus daemon (38 checks), plus 100% byte-identical lifecycle files vs the old Python CLI (parity test)
- [x] Rust `notify` CLI (`crates/notify-cli`) with the daemon embedded in the binary
| 1 | **Toolkit spike** | **Done.** Overview + History built in cxx-qt/Kirigami and measured (native look, live data, commands, accessibility, size, memory); Slint comparison skipped because the spike succeeded. See `docs/phase1-toolkit-spike.md` | 2 days |
| 2 | **Shell + Overview + History** | **Done.** Connection states (connecting / connected / no daemon / incompatible / lost) with advice, reconnect, version handshake, live state, DND and mute, history with Rust-side filtering, error toasts, remembered window size and toggles; Qt-free logic split into `notify-panel-core`; headless smoke tests that assert what the UI displays | 3 days |
| 3 | **Rules editor + Simulator** | Edit, reorder, validate, simulate, preview, save with conflict detection | 5-6 days |
| 4 | **Zones, Appearance** | Layout editor, theme, live preview, output picker | 3-4 days |
| 5 | **Apps & System** | Readiness, `adapt`, install/update/restore with dry-run and progress, `doctor` | 4 days |
| 6 | **Packaging** | PKGBUILD (split: `notifd`, `notify`, `notify-panel`), `.desktop`, icon, AppStream, first-run flow | 2 days |
| 7 | **Polish** | Keyboard-only pass, screen-reader pass, i18n scaffolding, error wording, docs | 3 days |

Phase 0 is useful on its own: it also gives scripts and other tools a stable API.

## 9. Risks and mitigations
| Risk | Mitigation |
|---|---|
| cxx-qt is pre-1.0 and its API changes | Pin the version; keep the Rust/QML boundary small (view-models only); Slint as fallback; spike first |
| Panel and daemon version skew | `hello` handshake with protocol number; clear "update notifd" message |
| Socket events missed on reconnect | On (re)connect the panel fetches full state, then applies events |
| Hand-edits to `rules.json` while the panel is open | Hash check on save; file watcher; offer reload/overwrite |
| `preview` shows popups over real work | Short fixed duration, clearly labelled, never during Do Not Disturb unless you ask |
| Python CLI stays a runtime dependency | Acceptable for v1; after v1 evaluate porting the CLI core into `notify-ops` (shared by CLI and panel) |
| QML/Quickshell `SocketServer` limits (framing, backpressure) | Spike it in phase 0 with a 1000-message soak test before building on it |
| Scope creep into a full notification center | Out of scope for v1 (section 1) |

## 10. Decisions
1. **Toolkit — DECIDED:** cxx-qt + QML with Kirigami (Slint is the fallback if the spike goes badly).
2. **Daemon-authoritative design — DECIDED:** the daemon owns `validate` / `simulate` / `preview`; the panel stays thin.
3. **Control socket — DECIDED and built:** JSON lines with live events; `qs ipc` unchanged. See `docs/protocol.md`.
4. **CLI — DECIDED: port to Rust, done.** `crates/notify-cli` replaces the Python CLI (kept in `legacy/` for the parity test).
   The binary embeds the daemon files, so updates no longer need a project folder.
5. **v1 scope — DECIDED:** Overview, History, Rules (with simulator), Zones and Appearance first; Apps and System in v1.1.
6. **Name — DECIDED:** "notify". Licence: still open (no LICENSE file has been added).

## 11. Repository layout (as built; `crates/notify-panel` and `packaging/` are still to come)
```
notify/                      (this project)
├── notifd/                  daemon (QML) + rules.json + rules.schema.json
├── legacy/notify-py         the old Python CLI (parity test only)
├── tests/                   private-bus harness (also used for protocol contract tests)
├── crates/
│   ├── notify-protocol/     typed messages + config model
│   ├── notify-client/       blocking socket client
│   ├── notify-testkit/      throwaway-daemon test harness
│   ├── notify-cli/          the `notify` command (replaces notify-ops: lifecycle lives here)
│   ├── notify-panel-core/   Qt-free logic: connection worker, view data, filtering
│   ├── notify-panel/        the Qt/QML app (Overview, History; Rules next)
│   └── notify-panel-smoke/  headless tests of the real app
├── packaging/               PKGBUILD, .desktop, icons, AppStream
└── docs/
```
