#!/bin/sh
# Test notifd on a real Hyprland without installing it: an Arch container runs Hyprland nested in your Wayland session
# (a window appears for about a minute), with notifd from this checkout behind `notify serve`. Needs podman and /dev/dri.
set -e
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
cargo build -q -p notify-cli --manifest-path "$root/Cargo.toml"
podman build -q -t notifi-hypr -f "$here/Containerfile" "$here" >/dev/null
exec podman run --rm --userns=keep-id --device /dev/dri \
    -v "$XDG_RUNTIME_DIR/${WAYLAND_DISPLAY:-wayland-0}:/host-run/wayland-0" \
    -v "$root/notifd:/src/notifd:ro" -v "$root/target/debug/notify:/notify:ro" -v "$here/run.py:/run.py:ro" \
    notifi-hypr python3 /run.py
