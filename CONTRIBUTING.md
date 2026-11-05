# Contributing

Thanks for helping. A few project rules:

- Every source file stays at or under 400 lines (`cargo test -p notify-cli --test loc`). A few older files are baselined in `tests/loc-baseline.txt`; baselines may only shrink.
- `notifd/*.qml` is the daemon payload and is embedded into `notify` at build time; no list needs editing when you add a file.
- Run before sending a change:
  - `cargo test --workspace --no-fail-fast -- --test-threads=1`
  - `python3 tests/test_notifd.py` (includes the feature and socket suites) and `python3 tests/test_sound.py` (need Quickshell, `dbus-run-session`, `notify-send`)
  - `cargo clippy --workspace --all-targets`
- Tests use a private D-Bus session; they never touch your real notifications.
- Branding: "noti-fi" is display only. The command is `notify`, the daemon `notifd`, the app id `org.notify.Panel`.

Open an issue first for larger changes.
