#!/usr/bin/env bash
# CI entry point for focus.py, run as root in a throwaway archlinux container given one DRM card (vkms, loaded on the
# runner), as uQ's smoke test does: Hyprland cannot start without a DRM device, even headless, and its DRM backend needs
# a seat, so seatd runs for an unprivileged user (Hyprland refuses root).
#   docker run --rm --device "$card" -e SMOKE_CARD="$card" -e SMOKE_UID="$(id -u)" -v "$PWD:/src" archlinux:latest bash /src/tests/hyprland/focus-in-container.sh
set -eu
card=${SMOKE_CARD:?SMOKE_CARD must name the DRM card passed to the container}
pacman -Syu --noconfirm --needed base-devel git rust qt6-base qt6-declarative python quickshell dbus libnotify sqlite hyprland mesa foot ttf-dejavu seatd >/dev/null
id tester >/dev/null 2>&1 || useradd -m -u "${SMOKE_UID:-1000}" tester
chmod 666 "$card"
SEATD_VTBOUND=0 seatd -u tester -l error &
rc=0
su tester -c "set -e
    cd /src
    cargo build --locked -p notify-cli
    export XDG_RUNTIME_DIR=\$(mktemp -d) LIBSEAT_BACKEND=seatd NOTIFD_SRC=/src/notifd NOTIFY_BIN=/src/target/debug/notify
    timeout 400 python3 tests/hyprland/focus.py" || rc=$?
kill %1 2>/dev/null || true
exit "$rc"
