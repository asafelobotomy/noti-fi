"""What a desktop shell (uQ) builds on: the socket's greeting and state, the level OSD, hiding text while sharing, and design tokens.
Run through test_notifd.py, which passes in its `check`, BASE_CONFIG and Sock client."""
import json, os, shutil, time


def make_tests(check, BASE_CONFIG, Sock):
    def test_shell_integration(d):
        c = Sock(d.sock, subscribe=True)
        check("the greeting names notifd and its API version", c.hello.get("name") == "notifd" and c.hello.get("protocol") == 1, str(c.hello))
        d.ipc("clearHistory")
        d.send("newest one", app="Steam", t=800)
        st = d.wait_for(lambda: (s := c.call("status")[0]) and (s.get("latest") or {}).get("summary") == "newest one" and s, 5) or {}
        latest = st.get("latest") or {}
        check("state has latest: the newest notification's app, icon and title", latest.get("appName") == "Steam" and "appIcon" in latest and latest.get("read") is False, str(latest))
        check("...and sharing, unread, dnd and center", {"sharing", "unread", "dnd", "center"} <= set(st) and st["sharing"] is False, str(sorted(st)))
        ev = c.wait_event("state", lambda e: (e.get("latest") or {}).get("summary") == "newest one")
        check("state events carry the same fields", ev is not None, str(c.events[-3:]))

        d.ipc("setAuto", "share", "true")
        st = d.wait_for(lambda: (s := c.call("status")[0]) and s.get("sharing") is True and s, 4) or {}
        check("while the screen is shared, sharing is true and latest has no title", st.get("sharing") is True and (st.get("latest") or {}).get("summary") == "" and (st.get("latest") or {}).get("appName") == "Steam", str(st.get("latest")))
        d.ipc("setAuto", "share", "false")

        r, e = c.call("osd", {"kind": "volume", "value": 40, "max": 100, "muted": False})
        check("osd shows a level", r == {"ok": True}, str(e))
        for bad in ({"kind": "loudness", "value": 1}, {"kind": "volume", "value": -1}, {"kind": "mic", "value": 1, "max": 0}, {"kind": "brightness", "value": "lots"}):
            r, e = c.call("osd", bad)
            check(f"osd refuses {bad}", e and e.get("code") == "bad_params", str(r or e))
        start = time.time()
        for v in range(0, 101, 5):             # key repeat: one line per step, no process
            c.call("osd", {"kind": "volume", "value": v, "max": 100})
        check("21 volume steps over one connection take well under a second", time.time() - start < 1.5, f"{time.time() - start:.2f} s")
        d.ipc("clearHistory")

    def test_design_tokens(d):
        theme_dir = d.state / "uq" / "theme"
        effective = lambda: (d.ipc("configStatus") or {}) and Sock(d.sock).call("configGet")[0]["effective"]["theme"]

        def tokens(surface, radius=10, font="Inter"):
            return {"id": "t", "colors": {"surface": surface, "border": "#66686e", "text": "#f8f8f2", "text_muted": "#bababa",
                                          "surface_alt": "#3f414b", "overlay": "#52535c", "accent": "#bd93f9"},
                    "ui": {"radius": radius}, "fonts": {"ui": font, "size": 11}}
        theme_dir.mkdir(parents=True, exist_ok=True)
        (theme_dir / "tokens.json").write_text(json.dumps(tokens("#343641")))
        t = d.wait_for(lambda: (x := effective()) and x.get("card") == "#343641" and x, 6) or {}
        check("uQ's tokens.json becomes the theme (colours, radius, font)", t.get("card") == "#343641" and t.get("subtext") == "#bababa" and t.get("radius") == 10 and t.get("fontFamily") == "Inter", str(t))

        # uQ swaps the whole theme folder: write a new one beside it and rename it into place
        new = d.state / "uq" / "theme.new"
        new.mkdir()
        (new / "tokens.json").write_text(json.dumps(tokens("#123456", radius=4)))
        old = d.state / "uq" / "theme.old"
        os.rename(theme_dir, old)
        os.rename(new, theme_dir)
        shutil.rmtree(old)
        t = d.wait_for(lambda: (x := effective()) and x.get("card") == "#123456" and x, 6) or {}
        check("a folder swap (as uQ writes themes) is picked up", t.get("card") == "#123456" and t.get("radius") == 4, str(t))

        (theme_dir / "tokens.json").write_text('{"colors": {"surface": "#')       # half written
        time.sleep(3)
        check("a broken tokens file keeps the last good theme", (effective() or {}).get("card") == "#123456", str(effective()))
        (theme_dir / "tokens.json").write_text(json.dumps({"colors": {"surface": "url(http://x)", "text": "#ffffff"}, "ui": {"radius": 999}}))
        t = d.wait_for(lambda: (x := effective()) and x.get("text") == "#ffffff" and x, 6) or {}
        check("values that are not plain colours or sizes are ignored", t.get("card") == BASE_CONFIG.get("theme", {}).get("card", "#1d1f27") and t.get("radius") == 12, str(t))

        cfg = {**BASE_CONFIG, "theme": {**BASE_CONFIG.get("theme", {}), "text": "#00ff00"}}
        d.write_config(cfg)
        t = d.wait_for(lambda: (x := effective()) and x.get("text") == "#00ff00" and x, 6) or {}
        check("your own theme settings win over the tokens", t.get("text") == "#00ff00", str(t))
        d.write_config({**BASE_CONFIG, "theme": {**BASE_CONFIG.get("theme", {}), "tokens": ""}})
        t = d.wait_for(lambda: (x := effective()) and x.get("text") == "#f2f3f7" and x, 6) or {}
        check('theme.tokens "" turns the tokens off', t.get("text") == "#f2f3f7", str(t))
        d.write_config(BASE_CONFIG)
        shutil.rmtree(d.state / "uq")
        d.wait_for(lambda: d.ipc("configStatus"), 3)

    return [test_shell_integration, test_design_tokens]
