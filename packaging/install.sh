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

install -d "$prefix/bin" "$prefix/share/applications" "$prefix/share/doc/noti-fi"
install -m 755 "$here/bin/notify" "$here/bin/notify-panel" "$prefix/bin/"
install -m 644 "$here/share/applications/org.notify.Panel.desktop" "$prefix/share/applications/"
cp -r "$here/share/doc/noti-fi/." "$prefix/share/doc/noti-fi/"

echo "installed to $prefix"
case ":$PATH:" in *":$prefix/bin:"*) ;; *) echo "note: $prefix/bin is not on your PATH" ;; esac
cat <<MSG

Next:
  notify takeover --dry-run     see exactly what it will change
  notify takeover --now         make notifd your notification service (undo any time: notify restore)
  notify-panel                  open the control panel

Needs at run time: quickshell, qt6-declarative, kirigami, qqc2-desktop-style (qt6-multimedia is optional, for audio/video players).
MSG
