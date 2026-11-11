Tests for the Notifications page in uQ Settings (`uq/pages/noti-fi/`).

```
logic.test.mjs     Node: logic.js, page.toml, what Page.qml may send   node --test 'tests/uq/*.test.mjs'
load-logic.mjs     loads logic.js (no Qt calls in it) for Node
toml-lite.mjs      the small TOML reader the Node tests use for page.toml
tst_page.qml       QML: the page against a fake ctx (needs Qt 6; CI runs it)
stub/ FakeContext.qml tst_components.qml
                   the page test kit of uQ v0.3.0-rc.1 (templates/settings-page-tests/), unchanged.
                   Refresh them when moving to a newer uQ.
fixtures/          what the `notify` commands print (README inside)
```

Run the QML tests offscreen (`/usr/lib/qt6/bin/qmltestrunner` on Arch, from `qt6-declarative`):

```
QT_QPA_PLATFORM=offscreen qmltestrunner -import tests/uq/stub -input tests/uq
```

Check the folder itself (uQ's own checker, built from the pinned release; see `.github/workflows/ci.yml`):

```
uqctl integrations check uq/
```

`notify settings` and `notify set`, which the page uses, are tested in `crates/notify-cli/tests/settings.rs`
(`cargo test -p notify-cli --test settings`); that test also checks `fixtures/` against the real command and the protocol types.
