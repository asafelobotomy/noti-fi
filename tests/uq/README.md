Tests for the Notifications page in uQ Settings (`uq/pages/noti-fi/`).

```
logic.test.mjs     Node: logic.js, page.toml, what the page may send   node --test 'tests/uq/*.test.mjs'
load-logic.mjs     loads logic.js (no Qt calls in it) for Node
toml-lite.mjs      the small TOML reader the Node tests use for page.toml
tst_*.qml          QML: the page, tab by tab, against a fake ctx (needs Qt 6; CI runs it); PageHarness.qml is what they share
stub/ FakeContext.qml tst_components.qml
                   the page test kit of uQ at the commit pinned in .github/workflows/ci.yml (uq_api 3,
                   templates/settings-page-tests/), unchanged. Refresh them when moving to a newer uQ.
fixtures/          what the `notify` commands print (README inside)
```

Run the QML tests offscreen (`/usr/lib/qt6/bin/qmltestrunner` on Arch, from `qt6-declarative`); `QML_XHR_ALLOW_FILE_READ`
lets the page test read `fixtures/`:

```
QML_XHR_ALLOW_FILE_READ=1 QT_QPA_PLATFORM=offscreen qmltestrunner -import tests/uq/stub -input tests/uq
```

Check the folder itself (uQ's own checker, built from the pinned commit; see `.github/workflows/ci.yml`):

```
uqctl integrations check uq/
```

`notify settings`, `notify set`, `notify config` and the plans (`history --clear --dry-run`), which the page uses, are tested
in `crates/notify-cli/tests/settings.rs` and `config.rs` (`cargo test -p notify-cli --test settings --test config`); those
tests also check `fixtures/` against the real commands and the protocol types
(`UPDATE_FIXTURES=1 cargo test -p notify-cli --test config` rewrites the plan and history fixtures).
