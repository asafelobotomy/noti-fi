#!/bin/sh
# Install noti-fi from this archive.
#   ./install.sh                 into ~/.local  (no root needed)
#   PREFIX=/usr ./install.sh     system-wide (run it with run0)
#   ./install.sh --uninstall     remove what this script installed
set -eu

here=$(cd "$(dirname "$0")" && pwd)
prefix=${PREFIX:-$HOME/.local}

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$prefix/bin/notify" "$prefix/bin/notify-panel" "$prefix/share/applications/org.notify.Panel.desktop"
    rm -rf "$prefix/share/doc/noti-fi"
    echo "removed noti-fi from $prefix (notifd itself is removed with: notify restore)"
    exit 0
fi

# A panel built the old way only runs on the Qt it was built against (same major.minor, installed patch not older); one marked portable runs on any Qt 6. The command-line tool does not use Qt.
host_qt=$(pkg-config --modversion Qt6Core 2>/dev/null || qmake6 -query QT_VERSION 2>/dev/null || true)
built_qt=$(cat "$here/qt-version" 2>/dev/null || true)
qt_ok() {
    [ -n "$host_qt" ] && [ -n "$built_qt" ] || return 1
    h1=${host_qt%%.*}; r=${host_qt#*.}; h2=${r%%.*}; h3=${r#*.}; h3=${h3%%.*}
    b1=${built_qt%%.*}; r=${built_qt#*.}; b2=${r%%.*}; b3=${r#*.}; b3=${b3%%.*}
    [ "$h1" = "$b1" ] || return 1
    [ -f "$here/portable-qml" ] && return 0     # built without ahead-of-time QML: any Qt 6 will do
    [ "$h2" = "$b2" ] && [ "${h3:-0}" -ge "${b3:-0}" ]
}

install -d "$prefix/bin" "$prefix/share/applications" "$prefix/share/doc/noti-fi"
install -m 755 "$here/bin/notify" "$prefix/bin/"
if qt_ok; then
    install -m 755 "$here/bin/notify-panel" "$prefix/bin/"
    panel_note=""
else
    panel_note="notify-panel was NOT installed: it was built for Qt ${built_qt:-?} and this machine has Qt ${host_qt:-?}. Build it for this machine instead: makepkg -si (packaging/PKGBUILD) or cargo build --release -p notify-panel."
fi
install -m 644 "$here/share/applications/org.notify.Panel.desktop" "$prefix/share/applications/"
cp -r "$here/share/doc/noti-fi/." "$prefix/share/doc/noti-fi/"

echo "installed to $prefix"
[ -z "$panel_note" ] || echo "note: $panel_note"
case ":$PATH:" in *":$prefix/bin:"*) ;; *) echo "note: $prefix/bin is not on your PATH" ;; esac
cat <<MSG

Next:
  notify takeover --dry-run     see exactly what it will change
  notify takeover --now         make notifd your notification service (undo any time: notify restore)
  notify-panel                  open the control panel

Needs at run time: quickshell, qt6-declarative, kirigami, qqc2-desktop-style (qt6-multimedia is optional, for audio/video players).
MSG
