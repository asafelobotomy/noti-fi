# noti-fi

Notification daemon for KDE Plasma on Wayland (`notifd`, QML on Quickshell) plus the Rust `notify` CLI and a Kirigami control panel. Display name "noti-fi"; the command is `notify`, the daemon `notifd`, the app id `org.notify.Panel`.

## Commands
- Build: `cargo build --workspace --locked`; CLI only: `cargo build --release -p notify-cli`.
- Test (full, as CONTRIBUTING says): `cargo test --workspace --no-fail-fast -- --test-threads=1` (starts a throwaway daemon on a private D-Bus).
- Test (CI's unit tests, no desktop needed): `cargo test --locked -p notify-protocol -p notify-apps -p notify-panel-core -p notify-jobs -p notify-update -p notify-bus --lib` and `cargo test --locked -p notify-cli --test loc --test version --test lifecycle --test settings` (lifecycle: takeover, update and restore in a fake home; settings: `notify settings` and `notify set` against a scripted notifd).
- Job bridge on a private bus (needs `dbus-daemon` only): `cargo test -p notify-jobs --test bridge`. The proxy (`notify serve`) end to end: `cargo test -p notify-cli --test serve` (needs Quickshell and a display). On a real Hyprland (nested, in a podman container; needs `/dev/dri`): `tests/hyprland/run.sh`.
- No desktop (CI): `tests/ci-daemon.sh` runs the daemon and sound suites under a headless labwc compositor. CI also runs the QML tests (`--test qml`, needs `kirigami`) and the job bridge test.
- Daemon suites (need Quickshell, `dbus-run-session`, `notify-send`): `python3 tests/test_notifd.py` (includes features and socket suites), `python3 tests/test_sound.py`.
- Lint: `cargo clippy --workspace --all-targets` (CI reports but does not block). Dependency policy: `cargo deny check` (`deny.toml`).
- Panel: `cargo run -p notify-panel`.

## Layout
- `crates/notify-cli` (`notify`; embeds the `notifd/` files), `notify-protocol` (socket messages and `rules.json` model), `notify-client`, `notify-testkit`, `notify-apps`, `notify-update`, `notify-jobs` (job bridge: KIO file-operation progress -> notifd, run as `notify jobs` by `notifd-jobs.service`), `notify-bus` (`notify serve`, what `notifd.service` runs: owns org.freedesktop.Notifications, runs Quickshell on a private bus behind it, forwards calls and signals, answers Inhibit), `notify-panel` (cxx-qt + Kirigami), `notify-panel-core` (Qt-free logic), `notify-panel-smoke` (headless panel tests).
- `notifd/`: daemon QML (`shell.qml`, `Popup.qml`, ...), `rules.json` and `rules.schema.json`. `tests/`: Python daemon suites, `tests/qml`, `tests/loc-baseline.txt`.
- `docs/protocol.md` documents the control socket. `packaging/`: PKGBUILD, `install.sh`, `build-tarball.sh`, desktop and metainfo files.

## Conventions
- Every source file is at most 400 lines (`cargo test -p notify-cli --test loc`); files in `tests/loc-baseline.txt` may only shrink.
- `notifd/*.qml` is embedded into `notify` at build time; no list needs editing when you add a file.
- Tests use a private D-Bus session and must never touch real notifications.
- Release: bump `version` in the root `Cargo.toml`, `daemonVersion` in `notifd/shell.qml` and `pkgver` in `packaging/PKGBUILD`, add the metainfo `<release>` line and a `## X.Y.Z` CHANGELOG section (the `version` test checks they all agree), commit, then push tag `vX.Y.Z`. The release workflow (Arch container) fails if the tag differs from the crate version, then publishes the tarball, a source archive (`noti-fi-X.Y.Z.tar.gz`, what the AUR package builds from) and their `.sha256` files. Its `aur` artifact holds the PKGBUILD with the real checksum and a `.SRCINFO`; pushing that to the AUR is the user's step, never an agent's. Bump `pkgver` in `packaging/PKGBUILD` with the version.
- Rust edition 2024, MSRV 1.89, MIT. Work on `main`.

## Gotchas
- Building the panel needs Qt 6 (CI installs `qt6-base qt6-declarative` on Arch). Runtime: `quickshell`, `qt6-declarative`, `kirigami`, `qqc2-desktop-style`; optional `qt6-multimedia`.
- `target/` and `/dist/` are git-ignored.
