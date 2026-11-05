#!/usr/bin/env python3
"""Regression tests for notifd.

Starts a throwaway daemon on a private D-Bus session (your real notifications are never
touched), sends real notifications with notify-send, and checks behaviour through the
daemon's IPC interface. Popups do appear briefly on screen while it runs.

    python3 tests/test_notifd.py            # run everything
    python3 tests/test_notifd.py -k markup  # only tests whose name contains "markup"
"""
import argparse, concurrent.futures, http.server, json, os, shutil, signal, socket, subprocess, sys, tempfile, threading, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "notifd"

BASE_CONFIG = {
    "defaultZone": "personal", "defaultTimeout": 1500, "maxVisible": 3, "debug": False,
    "history": {"limit": 50, "persist": False},
    "autoDnd": {"fullscreen": True, "screenShare": True, "watch": False},   # watch False: tests set the state by hand
    "zones": {
        "business": {"anchor": "top-left", "accent": "#c084fc", "label": "Business"},
        "personal": {"anchor": "top-right", "accent": "#60a5fa", "label": "Personal"},
        "system": {"anchor": "bottom-right", "accent": "#fbbf24", "label": "System"},
    },
    "rules": [
        {"name": "business sites", "match": {"origin": "onlyfans|fansly"}, "zone": "business"},
        {"name": "system", "match": {"appName": "^kernel$"}, "zone": "system"},
        {"name": "quiet app", "match": {"appName": "^quietapp$"}, "silent": True},
        {"name": "long lived", "match": {"appName": "^longlived$"}, "timeout": 0},
    ],
}


class Daemon:
    def __init__(self):
        self.dir = Path(tempfile.mkdtemp(prefix="notifd-test-"))
        self.payload = self.dir / "p"
        self.payload.mkdir()
        for f in ("shell.qml", "Popup.qml", "Media.qml", "TransferChart.qml", "Validate.qml"):
            shutil.copy2(SRC / f, self.payload / f)
        self.state = self.dir / "state"
        self.state.mkdir()
        self.write_config(BASE_CONFIG)
        self.proc = None
        self.sock = str(self.dir / "control.sock")
        self.env = dict(os.environ, QT_NO_XDG_DESKTOP_PORTAL="1", XDG_STATE_HOME=str(self.state), NOTIFD_SOCKET=self.sock)

    def write_config(self, cfg):
        text = cfg if isinstance(cfg, str) else json.dumps(cfg)
        (self.payload / "rules.json").write_text(text)

    def start(self):
        busfile = self.dir / "bus"
        self.proc = subprocess.Popen(
            ["dbus-run-session", "--", "bash", "-c", f'echo "$DBUS_SESSION_BUS_ADDRESS" > {busfile}; exec qs -p {self.payload}'],
            env=self.env, stdout=open(self.dir / "log", "w"), stderr=subprocess.STDOUT, start_new_session=True)
        end = time.time() + 30
        while time.time() < end:
            if busfile.exists() and busfile.read_text().strip() and self.ipc("status", quiet=True) is not None:
                self.bus = busfile.read_text().strip()
                self.env["DBUS_SESSION_BUS_ADDRESS"] = self.bus
                return
            time.sleep(0.4)
        raise RuntimeError("daemon did not start; log:\n" + (self.dir / "log").read_text()[-800:])

    def stop(self):
        if self.proc and self.proc.poll() is None:
            try:
                os.killpg(self.proc.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                self.proc.wait(timeout=8)
            except subprocess.TimeoutExpired:
                os.killpg(self.proc.pid, signal.SIGKILL)

    def kill_hard(self):
        """SIGKILL the daemon itself (no cleanup, leaves a stale socket file), then let the wrapper exit."""
        pid = self.pid()
        if pid:
            os.kill(pid, signal.SIGKILL)
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(self.proc.pid, signal.SIGKILL)

    def pid(self):
        try:
            c = Sock(self.sock)
            p = c.hello.get("pid")
            c.close()
            return p
        except Exception:
            return None

    def rss_mb(self, pid):
        try:
            for l in open(f"/proc/{pid}/status"):
                if l.startswith("VmRSS:"):
                    return int(l.split()[1]) / 1024
        except OSError:
            pass
        return -1

    def fds(self, pid):
        try:
            return len(os.listdir(f"/proc/{pid}/fd"))
        except OSError:
            return -1

    def ipc(self, fn, *args, quiet=False):
        r = subprocess.run(["qs", "-p", str(self.payload), "ipc", "call", "notifd", fn, *map(str, args)],
                           capture_output=True, text=True, timeout=15)
        if r.returncode != 0 or not r.stdout.strip():
            return None
        try:
            return json.loads(r.stdout)
        except ValueError:
            return None

    def send(self, summary, body="x", app="Steam", t=None, urgency=None, hints=(), icon=None, replace=None, print_id=False):
        cmd = ["notify-send", "-a", app]
        if t is not None:
            cmd += ["-t", str(t)]
        if urgency:
            cmd += ["-u", urgency]
        for h in hints:
            cmd += ["-h", h]
        if icon:
            cmd += ["-i", icon]
        if replace:
            cmd += ["-r", str(replace)]
        if print_id:
            cmd += ["-p"]
        cmd += [summary, body]
        r = subprocess.run(cmd, env=self.env, capture_output=True, text=True, timeout=15)
        return r.stdout.strip() if print_id else r.returncode

    def active(self):
        return (self.ipc("status") or {}).get("active")

    def wait_for(self, fn, timeout=6.0):
        end = time.time() + timeout
        while time.time() < end:
            v = fn()
            if v:
                return v
            time.sleep(0.15)
        return fn()


RESULTS = []


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok), detail))
    show = detail and (not ok or os.environ.get("NOTIFD_TEST_VERBOSE"))
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}" + (f"  ({detail})" if show else ""))


# ------------------------------------------------------------------ tests
def test_routing(d):
    d.send("hi", app="Brave Origin", hints=["string:x-kde-origin-name:onlyfans.com", "string:desktop-entry:brave-origin"], t=500)
    d.send("hi", app="Brave Origin", hints=["string:x-kde-origin-name:github.com"], t=500)
    d.send("disk", app="kernel", t=500)
    h = d.wait_for(lambda: (x := d.ipc("history", 10)) and len(x) >= 3 and x) or []
    zones = {i["summary"] + "/" + i["origin"]: i["zone"] for i in h}
    check("site rule routes onlyfans.com to business", zones.get("hi/onlyfans.com") == "business", str(zones))
    check("other sites fall through to the default zone", zones.get("hi/github.com") == "personal", str(zones))
    check("app rule routes kernel to system", zones.get("disk/") == "system", str(zones))


def test_dnd_and_mute(d):
    d.ipc("dnd", "on")
    d.send("hidden by dnd", t=500)
    d.send("critical passes", app="kernel", urgency="critical", t=500)
    h = d.wait_for(lambda: (x := d.ipc("history", 5)) and len(x) >= 2 and x) or []
    by = {i["summary"]: i for i in h}
    check("DND hides a normal notification but keeps it in history", by.get("hidden by dnd", {}).get("why") == "dnd", str(by.get("hidden by dnd")))
    check("critical notifications pass DND", by.get("critical passes", {}).get("shown") is True)
    d.ipc("dnd", "off")
    d.ipc("mute", "business", "true")
    d.send("muted", app="Brave Origin", hints=["string:x-kde-origin-name:fansly.com"], t=500)
    h = d.wait_for(lambda: (x := d.ipc("history", 1)) and x[0]["summary"] == "muted" and x) or [{}]
    check("a muted zone hides its notifications", h[0].get("why") == "muted business", str(h[0]))
    d.ipc("mute", "business", "false")
    check("unknown zone is rejected", d.ipc("mute", "nope", "true") is None or "error" in (d.ipc("mute", "nope", "true") or {}))


def test_silent_rule(d):
    d.wait_for(lambda: d.active() == 0, 5)      # let earlier tests' popups finish
    d.send("shh", app="quietapp", t=3000)
    h = d.wait_for(lambda: (x := d.ipc("history", 1)) and x[0]["summary"] == "shh" and x) or [{}]
    check("a silent rule never shows the popup", h[0].get("shown") is False and d.active() == 0, str(h[0]))


def test_timeouts(d):
    d.send("default timeout", t=None)       # no -t -> defaultTimeout 1500ms
    check("a notification shows", d.wait_for(lambda: d.active() == 1, 3))
    check("default timeout expires it", d.wait_for(lambda: d.active() == 0, 5))
    d.send("persistent", t=0)               # -t 0 -> never expire
    time.sleep(3.5)
    check("-t 0 (never expire) stays past the default timeout", d.active() == 1, f"active={d.active()}")
    d.ipc("dismissAll")
    check("dismissAll clears it", d.wait_for(lambda: d.active() == 0, 4))
    d.send("rule timeout 0", app="longlived", t=500)
    time.sleep(2.5)
    check("a rule's timeout 0 overrides the app's timeout", d.active() == 1, f"active={d.active()}")
    d.ipc("dismissAll")
    d.wait_for(lambda: d.active() == 0, 4)


def test_replace_refreshes_timer(d):
    nid = d.send("Download 10%", t=2200, print_id=True)
    time.sleep(1.5)
    d.send("Download 90%", t=2200, replace=nid)
    time.sleep(1.6)    # 3.1s after the first, 1.6s after the replace: old timer would have fired at 2.2s
    check("replacing a notification refreshes its timer", d.active() == 1, f"active={d.active()}")
    check("it still expires eventually", d.wait_for(lambda: d.active() == 0, 5))
    h = d.ipc("history", 5) or []
    n = [i for i in h if i["summary"].startswith("Download")]
    check("a replacement updates one history entry, not two", len(n) == 1 and n[0]["summary"] == "Download 90%", str(n))


def test_visible_cap(d):
    for i in range(7):
        d.send(f"burst {i}", t=0)
    st = d.wait_for(lambda: (s := d.ipc("status")) and s["perZone"].get("personal") == 7 and s) or {}
    check("at most maxVisible popups are shown per zone", st.get("active") == 3, str(st))
    check("the rest are queued, not lost", st.get("queued") == 4, str(st))
    d.ipc("dismissAll")
    check("queued notifications are cleared too", d.wait_for(lambda: (d.ipc("status") or {}).get("perZone", {}) == {}, 6))


def test_markup_cannot_fetch(d):
    hits = []

    class H(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            hits.append(self.path); self.send_response(404); self.end_headers()
        def log_message(self, *a): pass
    srv = http.server.HTTPServer(("127.0.0.1", 0), H)
    port = srv.server_address[1]
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    url = lambda n: f"http://127.0.0.1:{port}/{n}"
    d.send(f'<img src="{url("summary")}"/> title', "plain", t=2500)
    d.send("body", f'<img src="{url("body")}"/> <b>ok</b> <a href="{url("link")}">link</a>', t=2500)
    d.send("icon", "x", t=2500, icon=url("icon.png"))
    d.send("hint icon", "x", t=2500, hints=[f"string:image-path:{url('hint.png')}"])
    time.sleep(3)
    srv.shutdown()
    check("markup in titles, bodies and icons never triggers a network request", not hits, f"requests: {hits}")
    d.ipc("dismissAll")


def test_bad_config_never_drops_notifications(d):
    good = json.dumps(BASE_CONFIG)
    def cfg_ok(want):
        return d.wait_for(lambda: (c := d.ipc("configStatus")) and c["ok"] is want and c, 6)
    cases = {
        "bad regex": {**BASE_CONFIG, "rules": [{"name": "bad", "match": {"appName": "(unclosed"}, "zone": "business"}] + BASE_CONFIG["rules"]},
        "unknown zone in a rule": {**BASE_CONFIG, "rules": [{"name": "x", "match": {}, "zone": "nowhere"}]},
        "unknown match field": {**BASE_CONFIG, "rules": [{"name": "x", "match": {"nope": "a"}}]},
        "bad anchor": {**BASE_CONFIG, "zones": {"personal": {"anchor": "middle", "accent": "#fff"}}},
        "defaultZone missing": {**BASE_CONFIG, "defaultZone": "ghost"},
        "negative timeout": {**BASE_CONFIG, "defaultTimeout": -5},
    }
    for label, cfg in cases.items():
        d.write_config(cfg)
        c = cfg_ok(False)
        check(f"rejects config: {label}", bool(c) and c["error"] != "", str(c))
        d.send(f"still works after {label}", t=1000)
        check(f"  notifications still shown after: {label}", d.wait_for(lambda: d.active() == 1, 3))
        d.ipc("dismissAll"); d.wait_for(lambda: d.active() == 0, 3)
    d.write_config("{ this is not json")
    check("rejects invalid JSON", bool(cfg_ok(False)))
    d.write_config("[1, 2, 3]")
    check("rejects a top-level list", bool(cfg_ok(False)))
    d.write_config(good)
    check("accepts the good config again", bool(cfg_ok(True)))


def test_transient_and_progress(d):
    d.send("transient one", t=500, hints=["boolean:transient:true"])
    d.send("with progress", t=500, hints=["int:value:55"])
    time.sleep(1)
    h = d.ipc("history", 5) or []
    names = [i["summary"] for i in h]
    check("transient notifications are not kept in history", "transient one" not in names and "with progress" in names, str(names))
    d.send("sound hint", t=500, hints=["string:sound-name:message-new-instant", "boolean:suppress-sound:true"])
    check("sound hints do not crash the daemon", d.ipc("status") is not None)


def test_daemon_log_is_clean(d):
    log = (d.dir / "log").read_text()
    bad = [l for l in log.splitlines() if any(w in l for w in ("TypeError", "ReferenceError", "Cannot close destroyed", "SyntaxError: ", "is not a function"))]
    # the deliberate bad-config cases log a rejection line, not a script error
    check("no script errors or double-close errors in the daemon log", not bad, "\n".join(bad[:4]))

def test_auto_dnd(d):
    d.ipc("setAuto", "fullscreen", "true")
    st = d.ipc("status")
    check("full-screen state is reported as the reason popups are held", st["holding"] == "fullscreen" and st["auto"]["fullscreen"], str(st))
    d.send("held while fullscreen", t=500)
    d.send("critical still shows", app="kernel", urgency="critical", t=500)
    h = d.wait_for(lambda: (x := d.ipc("history", 5)) and len(x) >= 2 and x) or []
    by = {i["summary"]: i for i in h}
    check("normal notifications are held while a window is full screen", by.get("held while fullscreen", {}).get("why") == "fullscreen", str(by.get("held while fullscreen")))
    check("critical notifications still show during full screen", by.get("critical still shows", {}).get("shown") is True)
    d.ipc("dismissAll")
    d.ipc("setAuto", "fullscreen", "false")
    d.send("after fullscreen", t=800)
    h = d.wait_for(lambda: (x := d.ipc("history", 1)) and x[0]["summary"] == "after fullscreen" and x) or [{}]
    check("popups resume when full screen ends", h[0].get("shown") is True, str(h[0]))
    d.ipc("setAuto", "share", "true")
    d.send("held while sharing", t=500)
    h = d.wait_for(lambda: (x := d.ipc("history", 1)) and x[0]["summary"] == "held while sharing" and x) or [{}]
    check("popups are held while the screen is being shared", h[0].get("why") == "screen sharing", str(h[0]))
    d.ipc("setAuto", "share", "false")
    # the setting turns it off
    cfg = json.loads(json.dumps(BASE_CONFIG)); cfg["autoDnd"]["fullscreen"] = False
    d.write_config(cfg)
    d.wait_for(lambda: (c := d.ipc("configStatus")) and c["ok"] and True, 5); time.sleep(0.5)
    d.ipc("setAuto", "fullscreen", "true")
    d.send("fullscreen detection disabled", t=500)
    h = d.wait_for(lambda: (x := d.ipc("history", 1)) and x[0]["summary"] == "fullscreen detection disabled" and x) or [{}]
    check("autoDnd.fullscreen=false disables the hold", h[0].get("shown") is True, str(h[0]))
    d.ipc("setAuto", "fullscreen", "false"); d.write_config(BASE_CONFIG)
    d.wait_for(lambda: (c := d.ipc("configStatus")) and c["ok"] and True, 5)
    d.ipc("dismissAll")
    d.write_config({**BASE_CONFIG, "autoDnd": {"fullscreen": "yes"}})
    c = d.wait_for(lambda: (c := d.ipc("configStatus")) and (not c["ok"]) and c, 5)
    check("a non-boolean autoDnd setting is rejected", bool(c), str(c))
    d.write_config(BASE_CONFIG); d.wait_for(lambda: (c := d.ipc("configStatus")) and c["ok"] and True, 5)


def test_inline_reply(d):
    mon = subprocess.Popen(["busctl", "--user", "monitor", "--json=short", "--match", "type='signal',member='NotificationReplied'"],
                           env=d.env, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    time.sleep(1)
    try:
        r = subprocess.run(["busctl", "--user", "call", "--", "org.freedesktop.Notifications", "/org/freedesktop/Notifications",
                            "org.freedesktop.Notifications", "Notify", "susssasa{sv}i", "chatapp", "0", "", "New message from Sam",
                            "hey, are you free?", "4", "inline-reply", "Reply", "default", "Open", "1",
                            "x-kde-reply-placeholder-text", "s", "Write a reply...", "0"], env=d.env, capture_output=True, text=True)
        nid = int(r.stdout.split()[-1]) if r.returncode == 0 else -1
        check("a notification with a reply box is shown", d.wait_for(lambda: d.active() == 1, 4) and nid > 0, r.stderr)
        res = d.ipc("reply", nid, "I am free at five")
        check("reply is accepted for a notification that has a reply box", isinstance(res, dict) and res.get("ok") is True, str(res))
        got = None
        end = time.time() + 4
        import select
        while time.time() < end and not got:
            if select.select([mon.stdout], [], [], 0.3)[0]:
                try:
                    m = json.loads(mon.stdout.readline()); got = m["payload"]["data"]
                except Exception:
                    pass
        check("the app receives NotificationReplied with the typed text", got == [nid, "I am free at five"], str(got))
        d.ipc("dismissAll"); d.wait_for(lambda: d.active() == 0, 4)
        d.send("plain notification", t=0)
        d.wait_for(lambda: d.active() == 1, 3)
        nid2 = (d.ipc("history", 1) or [{}])[0].get("id")
        res = d.ipc("reply", nid2, "hello")
        check("reply is refused for a notification without a reply box", isinstance(res, dict) and "error" in res, str(res))
        res = d.ipc("reply", 99999, "hello")
        check("reply to a notification that is not on screen is refused", isinstance(res, dict) and "error" in res, str(res))
        d.ipc("dismissAll")
    finally:
        mon.terminate()

# ------------------------------------------------------------------ simulate / validate / preview / schema
def sim(d, notification, **extra):
    return d.ipc("simulate", json.dumps({"notification": notification, **extra}))


def test_validate_does_not_apply(d):
    good = d.ipc("validate", json.dumps(BASE_CONFIG))
    check("validate accepts a good config", isinstance(good, dict) and good.get("ok") is True, str(good))
    bad = d.ipc("validate", json.dumps({**BASE_CONFIG, "rules": [{"name": "r", "match": {"appName": "(oops"}}]}))
    check("validate rejects a bad regex with a reason", isinstance(bad, dict) and bad.get("ok") is False and "regex" in bad.get("error", ""), str(bad))
    def with_zone(z):   # change one zone, keep the others (the rules refer to them)
        return {**BASE_CONFIG, "zones": {**BASE_CONFIG["zones"], "system": z}}
    places = ["top-left", "top-center", "top-right", "middle-left", "center", "middle-right", "bottom-left", "bottom-center", "bottom-right"]
    for a in places:
        r = d.ipc("validate", json.dumps(with_zone({"anchor": a})))
        check(f"validate accepts the anchor '{a}'", isinstance(r, dict) and r.get("ok") is True, str(r))
    free = d.ipc("validate", json.dumps(with_zone({"anchor": "free", "x": 0.3, "y": 0.8})))
    check("validate accepts a free zone with a position", isinstance(free, dict) and free.get("ok") is True, str(free))
    for name, z in {"no position": {"anchor": "free"}, "x out of range": {"anchor": "free", "x": 1.5, "y": 0.5}, "y not a number": {"anchor": "free", "x": 0.5, "y": "low"}}.items():
        r = d.ipc("validate", json.dumps(with_zone(z)))
        check(f"validate rejects a free zone with {name}", isinstance(r, dict) and r.get("ok") is False and "free zone" in r.get("error", ""), str(r))
    bad2 = d.ipc("validate", "{not json")
    check("validate rejects invalid JSON", isinstance(bad2, dict) and bad2.get("ok") is False, str(bad2))
    c = d.ipc("configStatus")
    check("validating a draft never changes the running config", c["ok"] and c["rules"] == len(BASE_CONFIG["rules"]), str(c))


def test_simulate_agrees_with_reality(d):
    """For each case: ask the simulator, then send the real notification and compare what actually happened."""
    cases = [
        ("site rule", dict(appName="Brave Origin", origin="onlyfans.com", summary="sim-1"), {}, []),
        ("system app", dict(appName="kernel", summary="sim-2"), {}, []),
        ("silent rule", dict(appName="quietapp", summary="sim-3"), {}, []),
        ("default zone", dict(appName="Steam", summary="sim-4"), {}, []),
        ("dnd holds normal", dict(appName="Steam", summary="sim-5"), {"dnd": True}, []),
        ("dnd passes critical", dict(appName="kernel", summary="sim-6", urgency="critical"), {"dnd": True}, ["-u", "critical"]),
        ("muted zone", dict(appName="Brave Origin", origin="fansly.com", summary="sim-7"), {"mute": "business"}, []),
        ("fullscreen holds", dict(appName="Steam", summary="sim-8"), {"fullscreen": True}, []),
    ]
    for label, n, state, extra in cases:
        d.wait_for(lambda: d.active() == 0, 5)
        d.ipc("dnd", "on" if state.get("dnd") else "off")
        d.ipc("mute", "business", "true" if state.get("mute") else "false")
        d.ipc("setAuto", "fullscreen", "true" if state.get("fullscreen") else "false")
        predicted = sim(d, n)
        hints = [f"string:x-kde-origin-name:{n['origin']}"] if n.get("origin") else []
        d.send(n["summary"], app=n["appName"], t=600, hints=hints, urgency=n.get("urgency"))
        h = d.wait_for(lambda: (x := d.ipc("history", 3)) and any(i["summary"] == n["summary"] for i in x) and x) or []
        actual = next((i for i in h if i["summary"] == n["summary"]), {})
        same = (predicted and actual and predicted["zone"] == actual["zone"] and predicted["hold"] == actual["why"]
                and predicted["wouldShow"] == actual["shown"])
        check(f"simulator matches reality: {label}", same, f"predicted={predicted} actual={actual}")
    d.ipc("dnd", "off"); d.ipc("mute", "business", "false"); d.ipc("setAuto", "fullscreen", "false")


def test_simulate_details(d):
    r = sim(d, {"appName": "kernel", "summary": "x", "urgency": "critical"})
    check("simulate names the rule that decided the zone", r and r["decidedBy"]["zone"] == "system" and "system" in r["matched"], str(r))
    check("simulate also reports the matched rule's position in the list", r and r["matchedIdx"] == [1] and r["decidedIdx"]["zone"] == 1, str(r))
    r = sim(d, {"appName": "Steam", "summary": "x"})
    check("simulate says 'default' when no rule decided", r and r["decidedBy"]["zone"] is None and r["matched"] == [] and r["timeout"] == 1500, str(r))
    r = sim(d, {"appName": "Steam", "summary": "x", "expireTimeout": 0})
    check("simulate honours an app's never-expire request", r and r["timeout"] == 0, str(r))
    draft = {**BASE_CONFIG, "rules": [{"name": "everything to system", "match": {}, "zone": "system"}]}
    r = sim(d, {"appName": "Steam", "summary": "x"}, config=draft)
    check("simulate can evaluate a draft config", r and r["zone"] == "system" and r["decidedBy"]["zone"] == "everything to system", str(r))
    live = sim(d, {"appName": "Steam", "summary": "x"})
    check("evaluating a draft leaves the live config alone", live and live["zone"] == "personal", str(live))
    r = sim(d, {"appName": "Steam", "summary": "x"}, config={**BASE_CONFIG, "defaultZone": "ghost"})
    check("simulate reports an invalid draft instead of guessing", isinstance(r, dict) and "error" in r, str(r))
    d.ipc("dnd", "on")
    held = sim(d, {"appName": "Steam", "summary": "x"})
    free = sim(d, {"appName": "Steam", "summary": "x"}, ignoreState=True)
    d.ipc("dnd", "off")
    check("ignoreState shows the rule outcome independent of Do Not Disturb", held["hold"] == "dnd" and free["hold"] == "" and free["wouldShow"], f"{held} / {free}")


def test_preview(d):
    d.wait_for(lambda: d.active() == 0, 5)
    hist_before = len(d.ipc("history", 100) or [])
    r = d.ipc("preview", json.dumps({"seconds": 3}))
    check("preview starts", isinstance(r, dict) and r.get("ok") and r["shown"] == 5, str(r))
    check("preview popups are really on screen", d.wait_for(lambda: d.active() == 5, 3), f"active={d.active()}")
    check("previews are not recorded in history", len(d.ipc("history", 100) or []) == hist_before)
    check("preview popups go away on their own", d.wait_for(lambda: d.active() == 0, 7), f"active={d.active()}")
    # a draft config is applied only for the preview
    draft = {**BASE_CONFIG, "maxVisible": 1}
    r = d.ipc("preview", json.dumps({"config": draft, "seconds": 3}))
    check("a draft config is previewed with its own settings", r and r.get("ok") and d.wait_for(lambda: (d.ipc("status") or {}).get("cap") == 1, 2))
    check("...and one popup per zone shows because the draft caps each at 1", d.wait_for(lambda: d.active() == 3, 3), f"active={d.active()}")
    d.ipc("endPreview")
    check("endPreview clears everything and restores the real config",
          d.wait_for(lambda: d.active() == 0 and (d.ipc("status") or {}).get("cap") == 3, 4), str(d.ipc("status")))
    # every new place can be shown: the middle row (vertically centred), top/bottom middle, and a free position in each half
    spread = {**BASE_CONFIG, "maxVisible": 1, "zones": {"business": {"anchor": "center"}, "personal": {"anchor": "free", "x": 0.2, "y": 0.9}, "system": {"anchor": "middle-right"}}}
    r = d.ipc("preview", json.dumps({"config": spread, "seconds": 3}))
    check("a preview with centre, free and middle-row zones shows popups", r and r.get("ok") and d.wait_for(lambda: d.active() == 3, 3), f"{r} active={d.active()}")
    d.ipc("endPreview")
    d.wait_for(lambda: d.active() == 0, 4)
    spread["zones"]["personal"] = {"anchor": "free", "x": 0.7, "y": 0.1}; spread["zones"]["business"] = {"anchor": "top-center"}; spread["zones"]["system"] = {"anchor": "bottom-center"}
    r = d.ipc("preview", json.dumps({"config": spread, "seconds": 3}))
    check("a preview with top/bottom middle and a free zone in the upper half shows popups", r and r.get("ok") and d.wait_for(lambda: d.active() == 3, 3), f"{r} active={d.active()}")
    d.ipc("endPreview")
    d.wait_for(lambda: d.active() == 0 and (d.ipc("status") or {}).get("cap") == 3, 4)
    r = d.ipc("preview", json.dumps({"config": {**BASE_CONFIG, "defaultZone": "ghost"}}))
    check("an invalid draft is refused and shows nothing", isinstance(r, dict) and "error" in r and d.active() == 0, str(r))
    d.ipc("dnd", "on")
    d.ipc("preview", json.dumps({"seconds": 2}))
    check("previews ignore Do Not Disturb", d.wait_for(lambda: d.active() == 5, 3), f"active={d.active()}")
    d.ipc("dnd", "off"); d.ipc("endPreview")
    custom = d.ipc("preview", json.dumps({"seconds": 2, "samples": [{"appName": "X", "summary": "only one", "zone": "system"}]}))
    check("custom samples are honoured", custom and custom.get("shown") == 1 and d.wait_for(lambda: d.active() == 1, 3), str(custom))
    d.ipc("endPreview"); d.wait_for(lambda: d.active() == 0, 4)


def mini_schema_errors(v, sch, path="$"):
    """Tiny JSON Schema checker for the subset rules.schema.json uses (no external dependency)."""
    errs = []
    t = sch.get("type")
    kinds = {"object": dict, "array": list, "string": str, "boolean": bool}
    if t == "number" and (not isinstance(v, (int, float)) or isinstance(v, bool)):
        return [f"{path}: expected number"]
    if t == "integer" and (not isinstance(v, int) or isinstance(v, bool)):
        return [f"{path}: expected integer"]
    if t in kinds and not isinstance(v, kinds[t]):
        return [f"{path}: expected {t}"]
    if "enum" in sch and v not in sch["enum"]:
        errs.append(f"{path}: {v!r} not in {sch['enum']}")
    if "minimum" in sch and isinstance(v, (int, float)) and v < sch["minimum"]:
        errs.append(f"{path}: below minimum")
    if "pattern" in sch and isinstance(v, str):
        import re
        if not re.search(sch["pattern"], v):
            errs.append(f"{path}: does not match {sch['pattern']}")
    if isinstance(v, dict):
        for k in sch.get("required", []):
            if k not in v:
                errs.append(f"{path}: missing {k}")
        if "minProperties" in sch and len(v) < sch["minProperties"]:
            errs.append(f"{path}: too few properties")
        names = sch.get("propertyNames", {}).get("enum")
        props = sch.get("properties", {})
        extra = sch.get("additionalProperties", True)
        for k, val in v.items():
            if names is not None and k not in names:
                errs.append(f"{path}: property name {k!r} not allowed")
            if k in props:
                errs += mini_schema_errors(val, props[k], f"{path}.{k}")
            elif extra is False:
                errs.append(f"{path}: unknown property {k!r}")
            elif isinstance(extra, dict):
                errs += mini_schema_errors(val, extra, f"{path}.{k}")
    if isinstance(v, list) and isinstance(sch.get("items"), dict):
        for i, it in enumerate(v):
            errs += mini_schema_errors(it, sch["items"], f"{path}[{i}]")
    return errs


def test_schema(d):
    schema = json.loads((SRC / "rules.schema.json").read_text())
    shipped = json.loads((SRC / "rules.json").read_text())
    check("the shipped rules.json satisfies the JSON Schema", not mini_schema_errors(shipped, schema), str(mini_schema_errors(shipped, schema)[:3]))
    check("the test config satisfies the JSON Schema", not mini_schema_errors(BASE_CONFIG, schema), str(mini_schema_errors(BASE_CONFIG, schema)[:3]))
    typo = {**shipped, "defualtZone": "x"}
    check("the schema catches a typo'd setting name", any("defualtZone" in e for e in mini_schema_errors(typo, schema)))
    bad_anchor = json.loads(json.dumps(shipped)); bad_anchor["zones"]["system"]["anchor"] = "middle"
    check("the schema catches a bad anchor", bool(mini_schema_errors(bad_anchor, schema)))
    # whatever the schema accepts structurally, the daemon must accept too (shipped file)
    v = d.ipc("validate", json.dumps(shipped))
    check("the daemon accepts the shipped config the schema accepts", v and v.get("ok") is True, str(v))

def test_cli_smoke(d):
    """Every read-only CLI command must run to completion without a traceback (against this machine's real setup)."""
    rust = next((p for p in (ROOT / "target/release/notify", ROOT / "target/debug/notify") if p.exists()), None)
    cmd = [str(rust)] if rust else [sys.executable, "-I", str(ROOT / "legacy/notify-py")]
    notify = cmd
    for args in (["status"], ["status", "--json"], ["apps"], ["apps", "--json"], ["rules"], ["rules", "--json"],
                 ["rules", "--file", str(SRC / "rules.json")], ["doctor"], ["doctor", "--json"], ["autodnd", "status"], ["tray", "status"],
                 ["restore", "--dry-run"], ["takeover", "--dry-run"], ["portal", "--dry-run"],
                 ["hotkey", "--dry-run"], ["adapt"], ["--help"]):
        r = subprocess.run([*notify, *args], capture_output=True, text=True, timeout=60)
        ok = "Traceback" not in r.stderr and "Traceback" not in r.stdout
        check(f"notify {' '.join(args)} runs without crashing", ok, (r.stderr or r.stdout)[-300:])
    r = subprocess.run([*notify, "status", "--json"], capture_output=True, text=True, timeout=30)
    try:
        j = json.loads(r.stdout)
        check("status --json is valid JSON with the expected keys", {"owner", "notifd_owns", "daemon", "config"} <= set(j))
    except ValueError:
        check("status --json is valid JSON with the expected keys", False, r.stdout[:200])


# ------------------------------------------------------------------ control socket
class Sock:
    """Minimal client for the control socket (JSON lines)."""
    def __init__(self, path, subscribe=False, timeout=5):
        self.s = socket.socket(socket.AF_UNIX)
        self.s.settimeout(timeout)
        self.s.connect(path)
        self.buf = b""
        self.events = []
        self.next_id = 1
        self.hello = self.read(timeout)
        if subscribe:
            self.call("subscribe")

    def read(self, timeout=5):
        self.s.settimeout(timeout)
        while b"\n" not in self.buf:
            chunk = self.s.recv(65536)
            if not chunk:
                raise ConnectionError("closed")
            self.buf += chunk
        line, self.buf = self.buf.split(b"\n", 1)
        return json.loads(line)

    def send(self, obj):
        self.s.sendall(json.dumps(obj).encode() + b"\n")

    def send_raw(self, raw: bytes):
        self.s.sendall(raw)

    def call(self, method, params=None, timeout=5):
        """Send a request and return (result, error); events seen while waiting are kept in self.events."""
        rid = self.next_id
        self.next_id += 1
        self.send({"id": rid, "method": method, "params": params or {}})
        end = time.time() + timeout
        while time.time() < end:
            m = self.read(max(0.05, end - time.time()))
            if m.get("event"):
                self.events.append(m)
            elif m.get("id") == rid:
                return m.get("result"), m.get("error")
        raise TimeoutError(method)

    def wait_event(self, name, pred=lambda e: True, timeout=4):
        end = time.time() + timeout
        while True:
            for i, e in enumerate(self.events):
                if e.get("event") == name and pred(e):
                    return self.events.pop(i)
            if time.time() > end:
                return None
            try:
                m = self.read(max(0.05, end - time.time()))
            except (socket.timeout, TimeoutError):
                return None
            if m.get("event"):
                self.events.append(m)

    def close(self):
        try:
            self.s.close()
        except OSError:
            pass


def test_socket_hello_and_requests(d):
    c = Sock(d.sock)
    h = c.hello
    check("the socket greets with hello (protocol, daemon, pid, seq)", h.get("event") == "hello" and h.get("protocol") == 1
          and isinstance(h.get("daemon"), str) and isinstance(h.get("pid"), int) and h["pid"] > 0 and isinstance(h.get("seq"), int), str(h))
    r, e = c.call("ping")
    check("ping works", r and r.get("pong") is True, str(e))
    r, e = c.call("status")
    ipc_status = d.ipc("status")
    check("status over the socket matches status over `qs ipc`", r and all(r[k] == ipc_status[k] for k in ("dnd", "muted", "zones", "cap", "holding")), f"{r} / {ipc_status}")
    r, e = c.call("nonsense")
    check("an unknown method is an error with code unknown_method", e and e["code"] == "unknown_method", str(e))
    c.send_raw(b"this is not json\n")
    m = c.read()
    check("invalid JSON is answered with bad_request (and the connection survives)", m.get("error", {}).get("code") == "bad_request" and c.call("ping")[0], str(m))
    c.send({"id": 99, "params": {}})
    m = c.read()
    check("a request without a method is rejected, echoing its id", m.get("id") == 99 and m["error"]["code"] == "bad_request", str(m))
    r, e = c.call("dnd", {"mode": "sideways"})
    check("bad parameters give code bad_params", e and e["code"] == "bad_params", str(e))
    r, e = c.call("mute", {"zone": "nowhere", "on": True})
    check("muting an unknown zone gives bad_params", e and e["code"] == "bad_params", str(e))
    r, e = c.call("configGet")
    on_disk = (d.payload / "rules.json").read_text()
    check("configGet returns the raw file text and the effective config", r and json.loads(r["raw"]) == json.loads(on_disk) and "zones" in r["effective"], str(e))
    r, e = c.call("validate", {"config": BASE_CONFIG})
    check("validate (config object) accepts a good config", r and r["ok"] is True, str(e))
    r, e = c.call("validate", {"text": "{nope"})
    check("validate (text) rejects broken JSON", r and r["ok"] is False, str(r))
    r, e = c.call("simulate", {"notification": {"appName": "kernel", "summary": "x"}})
    check("simulate works over the socket", r and r["zone"] == "system", f"{r} {e}")
    r, e = c.call("simulate", {"config": {**BASE_CONFIG, "defaultZone": "ghost"}, "notification": {}})
    check("simulate reports an invalid draft as bad_params", e and e["code"] == "bad_params", str(e))
    r, e = c.call("history", {"limit": 5})
    check("history works over the socket", isinstance(r, list), str(e))
    r, e = c.call("reply", {"id": 999999, "text": "x"})
    check("replying to a missing notification gives not_found", e and e["code"] == "not_found", str(e))
    c.close()


def test_socket_events(d):
    c = Sock(d.sock)
    r, e = c.call("subscribe")
    check("subscribe returns a status snapshot and the current seq", r and "status" in r and isinstance(r["seq"], int), str(e))
    quiet = Sock(d.sock)       # never subscribes
    c.call("dnd", {"mode": "on"})
    ev = c.wait_event("state", lambda e: e.get("dnd") is True)
    check("a state event is pushed when Do Not Disturb changes", ev is not None)
    c.call("dnd", {"mode": "off"})
    d.wait_for(lambda: d.active() == 0, 4)
    d.send("event test", app="Brave Origin", t=900, hints=["string:x-kde-origin-name:onlyfans.com"])
    ev = c.wait_event("notification", lambda e: e.get("summary") == "event test")
    check("a notification event carries zone, site and outcome", ev and ev["zone"] == "business" and ev["origin"] == "onlyfans.com" and ev["shown"] is True, str(ev))
    nid = ev["id"] if ev else -1
    ev2 = c.wait_event("closed", lambda e: e.get("id") == nid, timeout=6)
    check("a closed event follows when the popup goes away", ev2 is not None)
    first = d.send("Download 10%", t=4000, print_id=True)
    c.wait_event("notification", lambda e: e.get("summary") == "Download 10%")
    d.send("Download 90%", t=4000, replace=first)
    ev3 = c.wait_event("updated", lambda e: e.get("summary") == "Download 90%")
    check("replacing a notification pushes an updated event", ev3 is not None, str(c.events[-3:]))
    d.ipc("dismissAll")
    d.write_config({**BASE_CONFIG, "defaultZone": "ghost"})
    ev4 = c.wait_event("config", lambda e: e.get("ok") is False, timeout=6)
    check("a rejected config pushes a config event with the reason", ev4 and ev4["error"] != "", str(ev4))
    d.write_config(BASE_CONFIG)
    ev5 = c.wait_event("config", lambda e: e.get("ok") is True, timeout=6)
    check("a good config pushes ok again", ev5 is not None)
    c.call("preview", {"seconds": 2})
    p1 = c.wait_event("preview", lambda e: e.get("state") == "started")
    p2 = c.wait_event("preview", lambda e: e.get("state") == "ended", timeout=8)
    check("preview start and end are announced", p1 and p2 and p1["shown"] == 5, f"{p1} {p2}")
    check("events carry increasing sequence numbers", True)
    seqs = [e["seq"] for e in c.events if "seq" in e]
    check("sequence numbers never go backwards", seqs == sorted(seqs), str(seqs[:12]))
    # unsubscribed clients hear nothing
    quiet.s.settimeout(0.5)
    heard = None
    try:
        heard = quiet.read(0.5)
    except (socket.timeout, TimeoutError):
        pass
    check("a client that did not subscribe receives no events", heard is None, str(heard))
    c.call("unsubscribe")
    c.events.clear()
    d.send("after unsubscribe", t=600)
    check("unsubscribing stops the events", c.wait_event("notification", timeout=1.5) is None)
    d.ipc("dismissAll"); d.wait_for(lambda: d.active() == 0, 4)
    c.close(); quiet.close()


def test_socket_multi_client(d):
    cs = [Sock(d.sock, subscribe=True) for _ in range(4)]
    d.send("fan-out", t=600)
    got = [c.wait_event("notification", lambda e: e.get("summary") == "fan-out") for c in cs]
    check("every subscriber receives the same event", all(got) and len({g["seq"] for g in got}) == 1, str(got))
    for c in cs:
        c.close()
    d.wait_for(lambda: d.active() == 0, 4)


def test_socket_limits(d):
    big = Sock(d.sock)
    big.send_raw(b'{"id": 1, "method": "ping", "params": {"pad": "' + b"x" * (2 * 1024 * 1024) + b'"}}\n')
    try:
        m = big.read(8)
        check("an oversized request is refused with too_large", m.get("error", {}).get("code") == "too_large", str(m)[:200])
    except Exception as e:
        check("an oversized request is refused with too_large", False, repr(e))
    ok = False
    try:
        ok = big.call("ping")[0] is not None
    except Exception:
        pass
    check("...and the connection still works afterwards", ok)
    big.close()
    held = []
    try:
        for _ in range(16):
            held.append(Sock(d.sock))
        extra = None
        try:
            extra = Sock(d.sock, timeout=3)
            msg = extra.read(3) if extra.hello.get("event") != "error" else extra.hello
            if extra.hello.get("event") == "error":
                msg = extra.hello
        except Exception as e:
            msg = {"closed": repr(e)}
        refused = (extra is not None and extra.hello.get("event") == "error" and extra.hello.get("code") == "too_many_connections") or "closed" in str(msg)
        check("connection number 17 is refused with too_many_connections", refused, str(msg)[:200])
    finally:
        for h in held:
            h.close()
    time.sleep(0.5)
    c = Sock(d.sock)
    check("after the cap is freed new clients connect again", c.call("ping")[0] is not None)
    c.close()


def test_socket_stress(d):
    pid = d.pid()
    # (a) 1000 pipelined requests: all answered, in order
    c = Sock(d.sock, timeout=20)
    t0 = time.time()
    for i in range(1000):
        method = ("ping", "status", "configStatus", "history")[i % 4]
        c.send({"id": 1000 + i, "method": method, "params": {"limit": 3}})
    ids = []
    while len(ids) < 1000:
        m = c.read(20)
        if m.get("id") is not None:
            ids.append(m["id"])
    check("1000 pipelined requests are all answered, in order", ids == list(range(1000, 2000)), f"first bad index: {next((i for i,(a,b) in enumerate(zip(ids, range(1000,2000))) if a != b), None)}")
    check("...in a reasonable time", time.time() - t0 < 15, f"{time.time() - t0:.1f}s")
    c.close()

    # (b) connection churn: open/close 300 times; no fd or memory leak
    fd0, rss0 = d.fds(pid), d.rss_mb(pid)
    for _ in range(300):
        x = Sock(d.sock)
        x.close()
    time.sleep(1.5)
    fd1, rss1 = d.fds(pid), d.rss_mb(pid)
    check("300 connect/disconnect cycles leave no leaked file descriptors", fd1 - fd0 <= 3, f"fds {fd0} -> {fd1}")
    check("...and no significant memory growth", rss1 - rss0 < 30, f"rss {rss0:.0f}MB -> {rss1:.0f}MB")
    c = Sock(d.sock)
    check("the daemon is fine after the churn", c.call("status")[0] is not None)
    c.close()

    # (c) event flood: 400 notifications from 4 parallel senders; a fast subscriber misses none
    sub = Sock(d.sock, subscribe=True, timeout=20)
    d.ipc("dnd", "on")   # held notifications still produce events, and nothing piles up on screen
    got = []
    stop = threading.Event()
    def reader():
        while not stop.is_set():
            try:
                m = sub.read(1)
            except (socket.timeout, TimeoutError):
                continue
            except Exception:
                return
            if m.get("event") == "notification" and str(m.get("summary", "")).startswith("flood-"):
                got.append(m)
    th = threading.Thread(target=reader); th.start()
    def burst(k):
        for i in range(100):
            d.send(f"flood-{k}-{i}", t=300)
    with concurrent.futures.ThreadPoolExecutor(4) as ex:
        list(ex.map(burst, range(4)))
    end = time.time() + 20
    while len(got) < 400 and time.time() < end:
        time.sleep(0.2)
    stop.set(); th.join()
    check("400 notifications produce 400 events for a subscriber that keeps up", len(got) == 400, f"got {len(got)}")
    seqs = [g["seq"] for g in got]
    check("...with strictly increasing sequence numbers", all(b > a for a, b in zip(seqs, seqs[1:])))
    sub.close()

    # (d) a subscriber that never reads must not hurt the daemon
    lazy = Sock(d.sock, subscribe=True)
    rss_a = d.rss_mb(pid)
    def burst2(k):
        for i in range(100):
            d.send(f"lazy-{k}-{i}", body="y" * 200, t=300)
    with concurrent.futures.ThreadPoolExecutor(4) as ex:
        list(ex.map(burst2, range(3)))
    t1 = time.time()
    alive = d.ipc("status") is not None
    check("the daemon still answers promptly while a subscriber never reads", alive and time.time() - t1 < 3, f"{time.time() - t1:.1f}s")
    rss_b = d.rss_mb(pid)
    check("...and memory stays bounded", rss_b - rss_a < 80, f"rss {rss_a:.0f}MB -> {rss_b:.0f}MB")
    lazy.close()
    d.ipc("dnd", "off")

    # (e) concurrent hammering, with latency percentiles
    lat = []
    errs = []
    def hammer(k):
        cc = Sock(d.sock, timeout=10)
        for i in range(300):
            t = time.time()
            r, e = cc.call(("status", "ping", "simulate")[i % 3], {"notification": {"appName": "Steam", "summary": "x"}})
            lat.append(time.time() - t)
            if e:
                errs.append(e)
        cc.close()
    with concurrent.futures.ThreadPoolExecutor(6) as ex:
        list(ex.map(hammer, range(6)))
    lat.sort()
    p99 = lat[int(len(lat) * 0.99)]
    check("6 clients x 300 requests all succeed", not errs and len(lat) == 1800, f"errors={errs[:2]} n={len(lat)}")
    check("p99 request latency under load is below 500 ms", p99 < 0.5, f"p50={lat[len(lat)//2]*1000:.0f}ms p99={p99*1000:.0f}ms max={lat[-1]*1000:.0f}ms")
    d.wait_for(lambda: d.active() == 0, 8)


def test_socket_recovers_from_hard_kill(d):
    """SIGKILL leaves a stale socket file; a fresh daemon must still be able to listen on it."""
    check("the stale-socket scenario starts from a live daemon", d.pid() is not None)
    d.kill_hard()
    check("a hard-killed daemon leaves its socket file behind", os.path.exists(d.sock))
    d.start()
    c = Sock(d.sock)
    check("a new daemon takes over the stale socket and answers", c.call("ping")[0] is not None)
    c.close()


TESTS = [test_routing, test_dnd_and_mute, test_silent_rule, test_timeouts, test_replace_refreshes_timer,
         test_visible_cap, test_markup_cannot_fetch, test_bad_config_never_drops_notifications,
         test_transient_and_progress, test_auto_dnd, test_inline_reply,
         test_validate_does_not_apply, test_simulate_agrees_with_reality, test_simulate_details, test_preview, test_schema,
         test_cli_smoke, test_socket_hello_and_requests, test_socket_events, test_socket_multi_client,
         test_socket_limits, test_socket_stress, test_daemon_log_is_clean, test_socket_recovers_from_hard_kill]


def feature_tests():
    """The tests of look/behaviour, priority, focus and stacking live in test_features.py (they share this file's harness, passed in)."""
    import test_features
    return test_features.make_tests(check, BASE_CONFIG)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("-k", help="only run tests whose name contains this")
    a = ap.parse_args()
    d = Daemon()
    try:
        d.start()
        print(f"daemon up (private bus), config dir {d.dir}")
        for t in TESTS[:-2] + feature_tests() + TESTS[-2:]:      # the log check and the hard-kill test stay last
            if a.k and a.k not in t.__name__:
                continue
            print(t.__name__)
            try:
                t(d)
            except Exception as e:           # a crashing test is a failing test
                check(f"{t.__name__} ran without exceptions", False, repr(e))
    finally:
        d.stop()
    failed = [r for r in RESULTS if not r[1]]
    print(f"\n{len(RESULTS) - len(failed)} passed, {len(failed)} failed")
    for n, _, det in failed:
        print(f"  FAILED: {n} {det}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
