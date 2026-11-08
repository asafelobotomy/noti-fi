#!/bin/sh
# Run everything that starts a real notifd where there is no desktop (CI): notifd's popup windows need a Wayland compositor with layer-shell,
# so start labwc (a small wlroots compositor) on its headless backend first. Everything else is private to the suites
# (their own D-Bus session, state and socket). Needs: quickshell, labwc, dbus, libnotify, python, kirigami, qqc2-desktop-style, and the workspace built.
set -eu
cd "$(dirname "$0")/.."

if [ -z "${XDG_RUNTIME_DIR:-}" ]; then
    XDG_RUNTIME_DIR=$(mktemp -d)
    export XDG_RUNTIME_DIR
fi
chmod 700 "$XDG_RUNTIME_DIR"
export WLR_BACKENDS=headless WLR_HEADLESS_OUTPUTS=1 WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman
unset DISPLAY WAYLAND_DISPLAY

cfg=$(mktemp -d)
log=$(mktemp)
labwc -C "$cfg" > "$log" 2>&1 &
wm_pid=$!
trap 'kill "$wm_pid" 2>/dev/null || true; rm -rf "$cfg"' EXIT

sock=""
for _ in $(seq 100); do
    sock=$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -name 'wayland-*' ! -name '*.lock' 2>/dev/null | head -n 1)
    [ -n "$sock" ] && break
    sleep 0.1
done
if [ -z "$sock" ]; then
    echo "labwc did not start:" >&2
    cat "$log" >&2
    exit 1
fi
WAYLAND_DISPLAY=$(basename "$sock")
export WAYLAND_DISPLAY

python3 tests/test_notifd.py
python3 tests/test_sound.py
# the CLI end to end, the client contract and the panel smoke tests each start a throwaway daemon (one at a time)
cargo test --locked -p notify-cli -p notify-client -p notify-panel-smoke -- --test-threads=1
