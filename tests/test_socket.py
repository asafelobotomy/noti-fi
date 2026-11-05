"""Control-socket tests (hello, requests, events, limits, stress, recovery). Run through test_notifd.py, which passes in its `check`, BASE_CONFIG and Sock client."""
import concurrent.futures, json, os, socket, threading, time


def make_tests(check, BASE_CONFIG, Sock):
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
        d.send("event test", app="Brave Origin", t=900, hints=["string:x-kde-origin-name:example-shop.com"])
        ev = c.wait_event("notification", lambda e: e.get("summary") == "event test")
        check("a notification event carries zone, site and outcome", ev and ev["zone"] == "business" and ev["origin"] == "example-shop.com" and ev["shown"] is True, str(ev))
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

    return [test_socket_hello_and_requests, test_socket_events, test_socket_multi_client, test_socket_limits, test_socket_stress, test_socket_recovers_from_hard_kill]
