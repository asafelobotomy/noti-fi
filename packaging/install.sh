#!/bin/sh
# Install noti-fi from this archive.
#   ./install.sh                 into ~/.local  (no root needed)
#   PREFIX=/usr ./install.sh     system-wide (run it with run0); also installs the Notifications page for uQ Settings
#   ./install.sh --uninstall     remove what this script installed
# DESTDIR=<folder> puts everything under that folder instead of the real root (for packagers and tests).
set -eu

here=$(cd "$(dirname "$0")" && pwd)
prefix=${PREFIX:-$HOME/.local}
base=${DESTDIR:-}$prefix
# uQ Settings reads pages only from /usr/share/uq/settings/pages (never from a folder a user can write to), so the page is
# installed for a system-wide install only: under ~/.local it would never be found.
case "$prefix" in /usr | /usr/) uq_page=1 ;; *) uq_page= ;; esac
uq_pages_dir=$base/share/uq/settings/pages

if [ "${1:-}" = "--uninstall" ]; then
    # notify restore needs the notify binary: run it first, or notifd and its job bridge would stay installed with no way to remove them
    if [ -e "${XDG_STATE_HOME:-$HOME/.local/state}/notifd/takeover.json" ]; then
        echo "notifd is still installed: run 'notify restore' first (it removes notifd and the job bridge and hands notifications back to Plasma), then run this again" >&2
        exit 1
    fi
    rm -f "$base/bin/notify" "$base/bin/notify-panel" "$base/share/applications/org.notify.Panel.desktop" "$base/share/metainfo/org.notify.Panel.metainfo.xml" "$base/share/icons/hicolor/scalable/apps/org.notify.Panel.svg"
    rm -rf "$base/share/doc/noti-fi" "$uq_pages_dir/noti-fi"
    echo "removed noti-fi from $base"
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

install -d "$base/bin" "$base/share/applications" "$base/share/metainfo" "$base/share/icons/hicolor/scalable/apps" "$base/share/doc/noti-fi"
install -m 755 "$here/bin/notify" "$base/bin/"
if qt_ok; then
    install -m 755 "$here/bin/notify-panel" "$base/bin/"
    panel_note=""
else
    panel_note="notify-panel was NOT installed: it was built for Qt ${built_qt:-?} and this machine has Qt ${host_qt:-?}. Build it for this machine instead: makepkg -si (packaging/PKGBUILD) or cargo build --release -p notify-panel."
fi
# the launcher names the program by its full path: a prefix such as ~/.local/bin is not on every launcher's PATH
sed "s|^Exec=notify-panel|Exec=$prefix/bin/notify-panel|" "$here/share/applications/org.notify.Panel.desktop" > "$base/share/applications/org.notify.Panel.desktop"
chmod 644 "$base/share/applications/org.notify.Panel.desktop"
install -m 644 "$here/share/metainfo/org.notify.Panel.metainfo.xml" "$base/share/metainfo/"
install -m 644 "$here/share/icons/hicolor/scalable/apps/org.notify.Panel.svg" "$base/share/icons/hicolor/scalable/apps/"
gtk-update-icon-cache -q "$base/share/icons/hicolor" 2>/dev/null || true
cp -r "$here/share/doc/noti-fi/." "$base/share/doc/noti-fi/"
if [ -n "$uq_page" ] && [ -d "$here/share/uq/settings/pages/noti-fi" ]; then
    rm -rf "$uq_pages_dir/noti-fi"
    install -d "$uq_pages_dir/noti-fi"
    install -m 644 "$here/share/uq/settings/pages/noti-fi/"* "$uq_pages_dir/noti-fi/"
fi

echo "installed to $base"
[ -z "$panel_note" ] || echo "note: $panel_note"
case ":$PATH:" in *":$prefix/bin:"*) ;; *) echo "note: $prefix/bin is not on your PATH" ;; esac
cat <<MSG

Next:
  notify takeover --dry-run     see exactly what it will change
  notify takeover --now         make notifd your notification service (undo any time: notify restore)
  notify-panel                  open the control panel

Needs at run time: quickshell, qt6-declarative, kirigami, qqc2-desktop-style, dbus, pipewire (pw-dump) and python (screen-share detection); qt6-multimedia is optional, for audio/video players.
MSG
