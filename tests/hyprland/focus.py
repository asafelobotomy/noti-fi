"""A notification button brings its app's window forward, on a real (headless) Hyprland. Run by focus-in-container.sh.
The app here is notify-send: it sends a notification with a View button and then does nothing (no activation token, no
raising), like an app whose own request Hyprland would ignore. notifd must find the window itself. Two cases: the window's
class is the notification's app name, and the class differs from it and is only found through the desktop entry's
StartupWMClass."""
import glob, json, os, socket, subprocess, sys, time

RUN = os.environ["XDG_RUNTIME_DIR"]
HOME = os.path.join(RUN, "home")
os.makedirs(os.path.join(HOME, ".local/share/applications"), exist_ok=True)
base = dict(os.environ, XDG_RUNTIME_DIR=RUN, HOME=HOME, XDG_STATE_HOME=os.path.join(RUN, "state"), LIBGL_ALWAYS_SOFTWARE="1")
open(os.path.join(RUN, "hypr.lua"), "w").write(
    'hl.monitor({ output = "", mode = "1280x720", position = "auto", scale = 1 })\n'
    'hl.config({ misc = { disable_hyprland_logo = true, disable_splash_rendering = true } })\n')
open(os.path.join(HOME, ".local/share/applications/org.test.Thing.desktop"), "w").write(
    "[Desktop Entry]\nType=Application\nName=Thing\nExec=/usr/bin/thing-bin\nStartupWMClass=ThirdClass\n")
results = []
def check(name, ok, detail=""):
    results.append(ok); print(("  [PASS] " if ok else "  [FAIL] ") + name + ("" if ok else f"  ({detail})"), flush=True)

hypr = subprocess.Popen(["Hyprland", "-c", os.path.join(RUN, "hypr.lua")], env=base, stdout=open(os.path.join(RUN, "hypr.log"), "w"), stderr=subprocess.STDOUT)
sig = None
for _ in range(80):
    found = glob.glob(f"{RUN}/hypr/*/.socket.sock")
    if found:
        sig = found[0].split("/")[-2]; break
    time.sleep(0.5)
if not sig:
    print(open(os.path.join(RUN, "hypr.log")).read()[-3000:]); sys.exit("Hyprland did not start")
time.sleep(1)
wl = next((os.path.basename(p) for p in sorted(glob.glob(f"{RUN}/wayland-*")) if not p.endswith(".lock")), "wayland-1")
env = dict(base, HYPRLAND_INSTANCE_SIGNATURE=sig, WAYLAND_DISPLAY=wl, XDG_CURRENT_DESKTOP="Hyprland", QT_QPA_PLATFORM="wayland", NOTIFD_SOCKET=f"{RUN}/notifd.sock")
hc = lambda *a: subprocess.run(["hyprctl", *a], env=env, capture_output=True, text=True).stdout
def clients():
    try: return json.loads(hc("clients", "-j") or "[]")
    except ValueError: return []
def active():
    try: return json.loads(hc("activewindow", "-j") or "{}").get("class", "")
    except ValueError: return ""
def until(pred, t=10):
    end = time.time() + t
    while time.time() < end:
        v = pred()
        if v: return v
        time.sleep(0.25)
    return pred()
print("Hyprland up:", sig, wl, flush=True)

bus = subprocess.Popen(["dbus-daemon", "--session", "--nofork", "--print-address=1"], stdout=subprocess.PIPE, text=True)
env["DBUS_SESSION_BUS_ADDRESS"] = bus.stdout.readline().strip()
payload = os.path.join(RUN, "payload")
subprocess.run(["cp", "-r", os.environ.get("NOTIFD_SRC", "/src/notifd"), payload], check=True)
serve = subprocess.Popen([os.environ.get("NOTIFY_BIN", "/src/target/debug/notify"), "serve", "--payload", payload], env=env,
                         stdout=open(os.path.join(RUN, "serve.log"), "w"), stderr=subprocess.STDOUT)
def call(m, p=None):
    s = socket.socket(socket.AF_UNIX); s.connect(env["NOTIFD_SOCKET"]); f = s.makefile("rw")
    f.readline()
    f.write(json.dumps({"id": 1, "method": m, "params": p or {}}) + "\n"); f.flush()
    r = json.loads(f.readline()); s.close(); return r.get("result"), r.get("error")
def up():
    try: return bool(call("status")[0])
    except (OSError, ValueError): return False
check("notifd is up on Hyprland", bool(until(up, 30)))

wins = [subprocess.Popen(["foot", "-a", cls, "sleep", "600"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL) for cls in ("Target", "Other", "ThirdClass")]
have = until(lambda: {c["class"] for c in clients()} >= {"Target", "Other", "ThirdClass"}, 20)
check("three windows are open", bool(have), str([c["class"] for c in clients()]))

def focus_other():
    hc("dispatch", 'hl.dsp.focus({ window = "class:Other" })')
    return until(lambda: active() == "Other", 5)

def case(label, send, want):
    check(f"[{label}] a different window has focus before the click", bool(focus_other()), active())
    n = subprocess.Popen(["notify-send", *send], env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    def entry():
        r, _ = call("history", {"limit": 5})
        return next((h for h in (r or []) if h.get("live") and any(a["key"] == "view" for a in h["actions"])), None)
    h = until(entry, 10)
    check(f"[{label}] the notification arrives with a View button", bool(h), str(call("history", {"limit": 3})))
    if not h: return
    time.sleep(0.5)
    _, err = call("invoke", {"id": h["id"], "key": "view"})
    check(f"[{label}] pressing View is accepted", err is None, str(err))
    check(f"[{label}] {want} has focus afterwards", bool(until(lambda: active() == want, 6)), f"active={active()}")
    n.terminate()

case("app name is the window class", ["-a", "Target", "-A", "view=View", "-t", "0", "Hello", "x"], "Target")
case("class found through StartupWMClass", ["-a", "Totally Different", "-h", "string:desktop-entry:org.test.Thing", "-A", "view=View", "-t", "0", "Hello", "y"], "ThirdClass")

log = open(os.path.join(RUN, "serve.log")).read()
bad = [l for l in log.splitlines() if ("TypeError" in l or "ReferenceError" in l or "Error:" in l) and "portal" not in l]
check("no QML errors in notifd's log", not bad, "\n".join(bad[:8]))
for w in wins: w.terminate()
serve.terminate(); hypr.terminate(); bus.terminate()
print(f"{sum(results)} passed, {len(results) - sum(results)} failed")
sys.exit(0 if all(results) else 1)
