# uQ integration

`pages/noti-fi/` adds a **Notifications** page to uQ Settings (under Apps): Do not disturb, focus mode, muting a
group, automatic Do not disturb, history, and the names of your rules. It runs `notify` commands only (`status`,
`settings`, `focus`, `dnd`, `mute`, `unmute`, `set`, `history --clear`), follows `state.json` and `rules.json`, and
shows no notification text. The package installs it to `/usr/share/uq/settings/pages/noti-fi/`.

How it was tested: `tests/uq/` (Node tests of `logic.js` and the manifest, QML tests against a fake context,
and a packaging check; see its README), `crates/notify-cli/tests/settings.rs` for `notify settings` and
`notify set`, and `uqctl integrations check uq/` in CI. Contract: `docs/settings-pages.md` in the uQ repository.
