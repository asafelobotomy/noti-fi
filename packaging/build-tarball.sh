#!/bin/sh
# Build the release archive from already-built release binaries.
#   packaging/build-tarball.sh <version> [target-dir]      (target-dir defaults to ./target)
# Writes dist/noti-fi-<version>-<arch>-linux.tar.gz and a .sha256 next to it.
set -eu

version=${1:?usage: build-tarball.sh <version> [target-dir]}
target=${2:-target}
root=$(cd "$(dirname "$0")/.." && pwd)
arch=$(uname -m)
name="noti-fi-$version-$arch-linux"
out="$root/dist"
stage="$out/$name"

for bin in notify notify-panel; do
    [ -x "$target/release/$bin" ] || { echo "missing $target/release/$bin: run 'cargo build --release -p notify-cli -p notify-panel' first" >&2; exit 1; }
done

rm -rf "$stage"
mkdir -p "$stage/bin" "$stage/share/applications" "$stage/share/metainfo" "$stage/share/icons/hicolor/scalable/apps" "$stage/share/doc/noti-fi"
install -m 755 "$target/release/notify" "$target/release/notify-panel" "$stage/bin/"
install -m 644 "$root/packaging/org.notify.Panel.desktop" "$stage/share/applications/"
install -m 644 "$root/packaging/org.notify.Panel.metainfo.xml" "$stage/share/metainfo/"
install -m 644 "$root/packaging/org.notify.Panel.svg" "$stage/share/icons/hicolor/scalable/apps/"
install -m 644 "$root/README.md" "$root/LICENSE" "$stage/share/doc/noti-fi/"
cp -r "$root/docs" "$stage/share/doc/noti-fi/docs"
install -m 755 "$root/packaging/install.sh" "$stage/install.sh"
# The panel's compiled QML only runs on the Qt it was built against, so the archive says which one that was.
qt=$(pkg-config --modversion Qt6Core 2>/dev/null || qmake6 -query QT_VERSION 2>/dev/null || true)
[ -n "$qt" ] || { echo "cannot tell which Qt version this was built with (need pkg-config or qmake6)" >&2; exit 1; }
echo "$qt" > "$stage/qt-version"
# A panel with no ahead-of-time compiled QML uses none of Qt's private interfaces and runs on any Qt 6 (see crates/notify-panel/build.rs); say so only if that is true.
if ! nm -D --undefined-only "$target/release/notify-panel" | grep -q PRIVATE_API; then echo portable > "$stage/portable-qml"; fi
install -m 644 "$root/README.md" "$root/LICENSE" "$stage/"

# a reproducible archive: sorted names, fixed owner and times
( cd "$out" && tar --sort=name --owner=0 --group=0 --numeric-owner --mtime='2026-01-01 00:00:00' -czf "$name.tar.gz" "$name" )
( cd "$out" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256" )
rm -rf "$stage"
echo "$out/$name.tar.gz"
