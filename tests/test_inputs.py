"""Input-handling tests: markup, bad config, automatic Do Not Disturb, inline reply. Run through test_notifd.py, which passes in its `check`, BASE_CONFIG and paths."""
import http.server, json, os, re, signal, socket, subprocess, sys, tempfile, threading, time


def make_tests(check, BASE_CONFIG, ROOT, SRC):
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
        # escaped markup (decoded to tags by a careless renderer), in the app name, the title, the text and a button's label
        esc = lambda n: f'&lt;img src="{url(n)}"/&gt;'
        d.send(esc("esc-summary") + " title", esc("esc-body"), app=f'<img src="{url("app")}"/>App', t=2500)
        waiter = subprocess.Popen(["notify-send", "-a", "Steam", "-t", "1500", f'--action=go=<img src="{url("button")}"/>Go', "--wait", "button", "x"],
                                  env=d.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(2.5)
        d.ipc("center", "open")              # the notification list shows all of these again, buttons included
        time.sleep(1.5)
        d.ipc("center", "close")
        waiter.kill()
        srv.shutdown()
        check("markup in titles, bodies, icons, app names and button labels never triggers a network request, in popups or the notification list", not hits, f"requests: {hits}")
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
        # while sharing, history hands out no titles: the newest entry is hidden, and held
        h = d.wait_for(lambda: (x := d.ipc("history", 1)) and x[0].get("hidden") is True and x) or [{}]
        check("popups are held while the screen is being shared", h[0].get("why") == "screen sharing", str(h[0]))
        check("...and history gives out no title or text meanwhile", h[0].get("summary") == "" and h[0].get("body") == "", str(h[0]))
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

    def test_share_detection(d):
        """The screen-sharing probe in AutoDnd.qml, run on PipeWire graphs like the ones KWin really makes (recorded during an OBS capture)."""
        src = (SRC / "AutoDnd.qml").read_text()
        block = src[src.index("readonly property string shareScript:"):src.index("id: shareWatch")]
        script = "".join(bytes(p, "utf-8").decode("unicode_escape") for p in re.findall(r'"((?:[^"\\]|\\.)*)"', block))
        node = lambda i, props: {"id": i, "type": "PipeWire:Interface:Node", "info": {"props": props}}
        client = lambda i, binary: {"id": i, "type": "PipeWire:Interface:Client", "info": {"props": {"application.process.binary": binary}}}
        link = lambda i, out, inp: {"id": i, "type": "PipeWire:Interface:Link", "info": {"output-node-id": out, "input-node-id": inp}}
        # recorded live: OBS's capture, and the Claude app's (its computer use) read through a stream named after it
        kwin = node(3, {"media.class": "Stream/Output/Video", "media.name": "kwin-screencast-eDP-1", "node.name": "kwin_wayland"})
        obs = node(4, {"media.class": "Stream/Input/Video", "media.name": "OBS Studio", "node.name": "obs", "client.id": 20})
        cam = node(5, {"media.class": "Video/Source", "node.name": "v4l2_input.usb", "node.description": "HD Webcam"})
        claude_cast = node(6, {"media.class": "Stream/Output/Video", "media.name": "kwin-screencast-com.anthropic.Claude", "node.name": "kwin_wayland"})
        claude_in = node(7, {"media.class": "Stream/Input/Video", "node.name": "lamco-pw-76", "client.id": 21})
        # recorded live: hovering a taskbar entry; KWin captures that window and plasmashell reads it to draw the preview
        preview = node(8, {"media.class": "Stream/Output/Video", "media.name": "kwin-screencast-org.kde.dolphin", "node.name": "kwin_wayland"})
        preview_in = node(9, {"media.class": "Stream/Input/Video", "node.name": "plasma-screencast-node-8", "client.id": 22})
        gone = {"id": 3, "info": None}                   # how `pw-dump --monitor` reports a removed object
        cases = (([[client(20, "obs"), cam, obs, kwin, link(30, 3, 4)]], '["obs"]', "KWin's capture read by OBS names OBS"),
                 ([[cam, obs]], "[]", "no KWin stream (a webcam and a reader)"),
                 ([[cam], [kwin], [gone]], "[]", "a KWin stream nobody reads is not a capture (KDE's remote-control sessions keep one open)"),
                 ([[client(20, "obs"), obs], [kwin], [link(30, 3, 4)], [gone]], '[]\n["obs"]\n[]', "a share starting and ending is reported as it happens"),
                 ([[client(22, "plasmashell"), preview, preview_in, link(32, 8, 9)]], '["plasmashell"]', "a taskbar preview names plasmashell (which notifd then ignores)"),
                 ([[client(21, "claude"), claude_cast, claude_in, link(31, 6, 7)]], '["claude"]', "a capture by the Claude app (computer use) names it"),
                 (None, "[]", "pw-dump failing"))
        for updates, want, what in cases:
            with tempfile.TemporaryDirectory() as bin_dir:
                fake = os.path.join(bin_dir, "pw-dump")
                body = "".join("cat <<'EOF'\n" + json.dumps(u, indent=2) + "\nEOF\n" for u in updates) if updates is not None else "exit 1\n"
                with open(fake, "w") as f:
                    f.write("#!/bin/sh\n" + body)
                os.chmod(fake, 0o755)
                r = subprocess.run(["sh", "-c", script], env=dict(os.environ, PATH=bin_dir + ":" + os.environ["PATH"]), capture_output=True, text=True, timeout=20)
            check(f"screen sharing: {what}", r.stdout.strip() == want, r.stdout + r.stderr[-200:])
        for bad in (["claude", 3], "claude", [""]):
            v = d.ipc("validate", json.dumps({**BASE_CONFIG, "autoDnd": {**BASE_CONFIG["autoDnd"], "ignoreCaptureBy": bad}}))
            check(f"autoDnd.ignoreCaptureBy {bad!r} is refused", isinstance(v, dict) and v.get("ok") is False, str(v))
        v = d.ipc("validate", json.dumps({**BASE_CONFIG, "autoDnd": {**BASE_CONFIG["autoDnd"], "ignoreCaptureBy": ["claude"]}}))
        check("autoDnd.ignoreCaptureBy [\"claude\"] is accepted", isinstance(v, dict) and v.get("ok") is True, str(v))

    return [test_markup_cannot_fetch, test_bad_config_never_drops_notifications, test_auto_dnd, test_inline_reply, test_share_detection]
