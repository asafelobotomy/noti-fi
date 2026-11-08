"""notifd on a real Hyprland. tests/hyprland/run.sh runs this in a container, as a nested Hyprland window on the host's
Wayland session, with notifd from the checkout behind `notify serve`. Checks: the focused monitor, "screen": "focused",
the level OSD, and unplugging a monitor."""
import glob, json, os, shutil, socket, subprocess, sys, time

RUN = "/tmp/run"
os.makedirs(RUN, mode=0o700, exist_ok=True)
env = dict(os.environ, XDG_RUNTIME_DIR=RUN, WAYLAND_DISPLAY="/host-run/wayland-0", XDG_STATE_HOME="/tmp/state", HOME="/tmp/home")
os.makedirs("/tmp/home", exist_ok=True)
open("/tmp/h.conf", "w").write("monitor=,preferred,auto,1\nmisc {\n  disable_hyprland_logo = true\n  disable_splash_rendering = true\n}\n")
results = []
def check(name, ok, detail=""):
    results.append(ok); print(("  [PASS] " if ok else "  [FAIL] ") + name + ("" if ok else f"  ({detail})"), flush=True)

hypr = subprocess.Popen(["Hyprland", "--config", "/tmp/h.conf"], env=env, stdout=open("/tmp/hypr.log", "w"), stderr=subprocess.STDOUT)
sig = None
for _ in range(60):
    found = glob.glob(f"{RUN}/hypr/*/.socket.sock")
    if found:
        sig = found[0].split("/")[-2]; break
    time.sleep(0.5)
if not sig:
    print(open("/tmp/hypr.log").read()[-2000:]); sys.exit("Hyprland did not start")
wl = next((os.path.basename(p) for p in sorted(glob.glob(f"{RUN}/wayland-*")) if not p.endswith(".lock")), "wayland-1")
henv = dict(env, HYPRLAND_INSTANCE_SIGNATURE=sig, WAYLAND_DISPLAY=wl, XDG_CURRENT_DESKTOP="Hyprland", QT_QPA_PLATFORM="wayland", NOTIFD_SOCKET=f"{RUN}/notifd.sock")
hc = lambda *a: subprocess.run(["hyprctl", *a], env=henv, capture_output=True, text=True).stdout
print("Hyprland up:", sig, wl, hc("monitors", "-j") and [m["name"] for m in json.loads(hc("monitors", "-j"))], flush=True)

payload = "/tmp/payload"
shutil.copytree("/src/notifd", payload)
serve = subprocess.Popen(["dbus-run-session", "--", "/notify", "serve", "--payload", payload], env=henv, stdout=open("/tmp/serve.log", "w"), stderr=subprocess.STDOUT)

def call(m, p=None):
    s = socket.socket(socket.AF_UNIX); s.connect(henv["NOTIFD_SOCKET"]); f = s.makefile("rw")
    hello = json.loads(f.readline())
    f.write(json.dumps({"id": 1, "method": m, "params": p or {}}) + "\n"); f.flush()
    r = json.loads(f.readline()); s.close(); return hello, r.get("result"), r.get("error")
def status():
    try: return call("status")[1] or {}
    except OSError: return {}
def wait(pred, t=8):
    end = time.time() + t
    while time.time() < end:
        st = status()
        if st and pred(st): return st
        time.sleep(0.25)
    return status()

st = wait(lambda s: s.get("screens", {}).get("focused"), 20)
first = st.get("screens", {}).get("focused", "")
check("on Hyprland, notifd sees the focused monitor", first != "" and st["screens"]["osd"] == first, str(st.get("screens")))
hello = call("ping")[0]
check("the greeting names notifd", hello.get("name") == "notifd", str(hello))

hc("output", "create", "headless", "TEST-2")
time.sleep(1)
names = [m["name"] for m in json.loads(hc("monitors", "-j"))]
second = next((n for n in names if n != first), "")
hc("dispatch", f'hl.dsp.focus({{ monitor = "{second}" }})')      # Hyprland 0.56+: dispatchers are Lua
time.sleep(1)
st = wait(lambda s: s["screens"]["focused"] == second)
check(f"focusing another monitor ({second}) moves the OSD there", second != "" and st["screens"]["osd"] == second, f"{names} {st.get('screens')}")
check("popups stay on the configured screen (the first) by default", st["screens"]["popups"] == first, str(st.get("screens")))
_, r, e = call("osd", {"kind": "volume", "value": 30})
check("a level OSD can be shown there", r == {"ok": True}, str(e))

cfg = json.load(open(f"{payload}/rules.json")); cfg["screen"] = "focused"
open(f"{payload}/rules.json", "w").write(json.dumps(cfg))
st = wait(lambda s: s["screens"]["popups"] == second)
check('"screen": "focused" moves popups and the list to the focused monitor', st["screens"]["popups"] == second, str(st.get("screens")))

hc("output", "remove", second)
st = wait(lambda s: s["screens"]["osd"] == first and s["screens"]["popups"] == first)
check("unplugging that monitor moves the OSD and popups back, and notifd keeps running", st["screens"]["osd"] == first and st["screens"]["popups"] == first and serve.poll() is None, str(st.get("screens")))
_, r, e = call("osd", {"kind": "brightness", "value": 80})
check("the OSD still works after the unplug", r == {"ok": True}, str(e))

log = open("/tmp/serve.log").read()
bad = [l for l in log.splitlines() if ("TypeError" in l or "ReferenceError" in l or "Error:" in l) and "portal" not in l]
check("no QML errors in notifd's log", not bad, "\n".join(bad[:8]))
serve.terminate(); hypr.terminate()
print(f"{sum(results)} passed, {len(results) - sum(results)} failed")
sys.exit(0 if all(results) else 1)
