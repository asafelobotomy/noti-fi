#!/bin/sh
# TEMPORARY: screenshot a real Chromium web notification through notify serve on headless labwc.
set -eu
cd "$(dirname "$0")/../.."
ROOT=$PWD
OUT=$ROOT/shots; mkdir -p "$OUT"
export XDG_RUNTIME_DIR=$(mktemp -d); chmod 700 $XDG_RUNTIME_DIR
export WLR_BACKENDS=headless WLR_HEADLESS_OUTPUTS=1 WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman
unset DISPLAY WAYLAND_DISPLAY
cfg=$(mktemp -d); labwc -C $cfg > $OUT/labwc.log 2>&1 &
for _ in $(seq 100); do s=$(find $XDG_RUNTIME_DIR -maxdepth 1 -name 'wayland-*' ! -name '*.lock' | head -n1); [ -n "$s" ] && break; sleep 0.1; done
export WAYLAND_DISPLAY=$(basename $s)
ls -la $XDG_RUNTIME_DIR; echo "wayland=$WAYLAND_DISPLAY"
D=$(mktemp -d); mkdir -p $D/payload $D/run $D/state; chmod 700 $D/run
cp notifd/*.qml notifd/rules.json $D/payload/
dbus-daemon --session --nofork --nopidfile --print-address=1 > $D/addr 2>/dev/null &
WLPATH=$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY
sleep 1; ADDR=$(head -n1 $D/addr)
DBUS_SESSION_BUS_ADDRESS=$ADDR XDG_RUNTIME_DIR=$D/run XDG_STATE_HOME=$D/state NOTIFD_SOCKET=$D/run/notifd.sock QT_NO_XDG_DESKTOP_PORTAL=1 \
  WAYLAND_DISPLAY=$WLPATH QT_QPA_PLATFORM=wayland target/debug/notify serve --payload $D/payload > $OUT/serve.log 2>&1 &
sleep 8
export DBUS_SESSION_BUS_ADDRESS=$ADDR
gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.GetCapabilities > $OUT/caps.txt 2>&1 || busctl --user call org.freedesktop.Notifications /org/freedesktop/Notifications org.freedesktop.Notifications GetCapabilities > $OUT/caps.txt 2>&1 || true
cat $OUT/caps.txt
# real Chromium on Xvfb, same bus
cd tests/shot; (python3 -m http.server 8765 >/dev/null 2>&1 &)
npm i --no-audit --no-fund playwright-core >/dev/null 2>&1
cat > run.js <<'JS'
const {chromium}=require('playwright-core');
(async()=>{const b=await chromium.launch({executablePath:'/usr/bin/chromium',headless:false,args:['--no-sandbox']});
const c=await b.newContext();await c.grantPermissions(['notifications'],{origin:'http://localhost:8765'});
const p=await c.newPage();p.on('console',m=>console.log('console:',m.text()));await p.goto('http://localhost:8765/page.html');
await p.waitForTimeout(4000);console.log('title',await p.title());
globalThis.hold=await new Promise(r=>setTimeout(r,9000));await b.close()})()
JS
xvfb-run -a node run.js > $OUT/chromium.log 2>&1 &
CPID=$!
sleep 9
grim $OUT/web-notification-large-image.png || true
kill $CPID 2>/dev/null || true
cd $ROOT
# fallback: the exact payload Chromium sends, in case the browser did not get to show one
busctl --user call org.freedesktop.Notifications /org/freedesktop/Notifications org.freedesktop.Notifications Notify susssasa{sv}i "Google Chrome" 0 "" "Dana" "hey there\n<img src=\"file://$ROOT/tests/shot/big.png\" alt=\"\"/>" 6 default Activate 0 Reply settings Settings 3 desktop-entry s chromium-browser x-kde-origin-name s example.com urgency y 2 0 > $OUT/busctl.log 2>&1 || true
sleep 3
grim $OUT/payload-mimic.png || true
ls -la $OUT; tail -20 $OUT/serve.log $OUT/chromium.log
