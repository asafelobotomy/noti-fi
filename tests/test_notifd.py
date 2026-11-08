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
        {"name": "business sites", "match": {"origin": "example-shop|example-chat"}, "zone": "business"},
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
        for f in sorted(SRC.glob("*.qml")):       # every QML file of the daemon: new files need no list here
            shutil.copy2(f, self.payload / f.name)
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
    d.send("hi", app="Brave Origin", hints=["string:x-kde-origin-name:example-shop.com", "string:desktop-entry:brave-origin"], t=500)
    d.send("hi", app="Brave Origin", hints=["string:x-kde-origin-name:github.com"], t=500)
    d.send("disk", app="kernel", t=500)
    h = d.wait_for(lambda: (x := d.ipc("history", 10)) and len(x) >= 3 and x) or []
    zones = {i["summary"] + "/" + i["origin"]: i["zone"] for i in h}
    check("site rule routes example-shop.com to business", zones.get("hi/example-shop.com") == "business", str(zones))
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
    d.send("muted", app="Brave Origin", hints=["string:x-kde-origin-name:example-chat.com"], t=500)
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
    nid = d.send("Download 10%", t=3000, print_id=True)
    time.sleep(1.5)
    d.send("Download 90%", t=3000, replace=nid)
    time.sleep(1.8)    # 3.3s after the first, 1.8s after the replace: the old timer would have fired at 3.0s, the new one at 4.5s (1.2s of margin each side, for a loaded machine)
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


def more_tests():
    """Rules/simulation and input-handling tests live in test_rules.py and test_inputs.py (they share this file's harness, passed in)."""
    import test_inputs, test_rules
    return test_inputs.make_tests(check, BASE_CONFIG, ROOT, SRC) + test_rules.make_tests(check, BASE_CONFIG, ROOT, SRC)


TESTS = [test_routing, test_dnd_and_mute, test_silent_rule, test_timeouts, test_replace_refreshes_timer,
         test_visible_cap, test_transient_and_progress, test_daemon_log_is_clean]


def socket_tests():
    """The control-socket tests live in test_socket.py (they share this file's harness, passed in)."""
    import test_socket
    return test_socket.make_tests(check, BASE_CONFIG, Sock)


def feature_tests():
    """The tests of look/behaviour, priority, focus and stacking live in test_features.py (they share this file's harness, passed in)."""
    import test_desktop, test_features
    return test_features.make_tests(check, BASE_CONFIG) + test_desktop.make_tests(check, BASE_CONFIG, Sock)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("-k", help="only run tests whose name contains this")
    a = ap.parse_args()
    d = Daemon()
    try:
        d.start()
        print(f"daemon up (private bus), config dir {d.dir}")
        sock = socket_tests()
        for t in TESTS[:-1] + more_tests() + feature_tests() + sock[:-1] + TESTS[-1:] + sock[-1:]:      # the log check, then the hard-kill test, stay last
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
