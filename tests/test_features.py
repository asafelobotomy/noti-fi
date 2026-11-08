"""Daemon tests for popup look and behaviour, priority, focus modes and stacking. Run through test_notifd.py (which passes in its `check` and BASE_CONFIG)."""
import json, time


def make_tests(check, BASE_CONFIG):
    def test_style_and_types(d):
        """Per-notification look and behaviour: transparency, click-through, focus, size, font, group; plus what apps tell us about their types."""
        cfg = {**BASE_CONFIG,
               "theme": {**BASE_CONFIG.get("theme", {}), "opacity": 0.9, "fontSize": 13},
               "zones": {**BASE_CONFIG["zones"], "system": {**BASE_CONFIG["zones"]["system"], "opacity": 0.5, "width": 300}},
               "rules": [
                   {"name": "chat mail", "match": {"appName": "^Chatty$", "category": "^email"}, "zone": "business", "group": "Chatty mail", "opacity": 0.7, "fontFamily": "Serif"},
                   {"name": "chatty", "match": {"appName": "^Chatty$"}, "clickThrough": True, "noFocus": True, "width": 500, "opacity": 0.3, "fontSize": 16},
                   *BASE_CONFIG["rules"]]}

        def sim(n, c=cfg):
            r = d.ipc("simulate", json.dumps({"config": c, "notification": n, "ignoreState": True}))
            return r if isinstance(r, dict) else {}
        a = sim({"appName": "Chatty", "category": "email.arrived"})
        st = a.get("style", {})
        check("the first matching rule wins per style key (opacity from the more specific rule)", st.get("opacity") == 0.7 and st.get("fontFamily") == "Serif", str(a))
        check("later rules still supply keys the first one lacks (click-through, width, size)", st.get("clickThrough") is True and st.get("noFocus") is True and st.get("width") == 500 and st.get("fontSize") == 16, str(st))
        check("a group name and a zone come from the matching rule", st.get("group") == "Chatty mail" and a.get("zone") == "business", str(a))
        check("decidedBy names the rule that set each style key", a.get("decidedBy", {}).get("opacity") == "chat mail" and a.get("decidedBy", {}).get("clickThrough") == "chatty", str(a.get("decidedBy")))
        b = sim({"appName": "Chatty", "category": "im.received"})
        check("a different type of the same app gets different settings", b.get("style", {}).get("opacity") == 0.3 and b.get("style", {}).get("group") == "" and b.get("zone") != "business", str(b))
        c2 = sim({"appName": "Other", "urgency": "critical"})
        check("with no rule, the theme's opacity and font size apply", c2.get("style", {}).get("opacity") == 0.9 and c2.get("style", {}).get("fontSize") == 13 and c2["style"]["clickThrough"] is False, str(c2))
        z = sim({"appName": "kernel"})
        check("the zone's own opacity and width apply when no rule sets them", z.get("zone") == "system" and z["style"]["opacity"] == 0.5 and z["style"]["width"] == 300, str(z))
        w = sim({"appName": "Plain"})
        check("without any style the global width is used", w["style"]["width"] == BASE_CONFIG.get("width", 380) and w["style"]["group"] == "", str(w))

        for name, bad in {"opacity 0": {"opacity": 0}, "opacity text": {"opacity": "half"}, "click-through text": {"clickThrough": "yes"}, "width tiny": {"width": 10},
                          "font size huge": {"fontSize": 400}, "group a number": {"group": 5}}.items():
            r = d.ipc("validate", json.dumps({**cfg, "rules": [{"name": "bad", "match": {}, **bad}]}))
            check(f"validate rejects a rule with {name}", isinstance(r, dict) and r.get("ok") is False, str(r))
        r = d.ipc("validate", json.dumps({**cfg, "zones": {**cfg["zones"], "system": {**cfg["zones"]["system"], "group": "x"}}}))
        check("a group cannot be set on a zone", isinstance(r, dict) and r.get("ok") is False and "group" in r.get("error", ""), str(r))
        ts = sim({"appName": "Chatty", "category": "email.arrived"}, c={**cfg, "theme": {**cfg["theme"], "timerStyle": "line"}, "rules": [{"name": "t", "match": {"category": "^email"}, "timerStyle": "circle,frame"}, *cfg["rules"]]})["style"]
        check("a rule's timerStyle wins over the theme's", ts["timerStyle"] == "circle,frame", str(ts))
        check("the theme's timerStyle applies when no rule sets one", sim({"appName": "Other"}, c={**cfg, "theme": {**cfg["theme"], "timerStyle": "line"}})["style"]["timerStyle"] == "line")
        check("with nothing set the time-left indicator is off", sim({"appName": "Other"})["style"]["timerStyle"] == "none")
        for bad in ("sparkle", "line,sparkle", 5):
            r = d.ipc("validate", json.dumps({**cfg, "rules": [{"name": "t", "timerStyle": bad}]}))
            check(f"validate rejects timerStyle {bad!r}", isinstance(r, dict) and r.get("ok") is False and "timerStyle" in r.get("error", ""), str(r))
        r = d.ipc("validate", json.dumps({**cfg, "theme": {**cfg["theme"], "timerStyle": "line,circle,frame"}}))
        check("validate accepts all three indicators on the theme", isinstance(r, dict) and r.get("ok") is True, str(r))
        r = d.ipc("validate", json.dumps({**cfg, "theme": {"opacity": 5}}))
        check("validate rejects a theme opacity out of range", isinstance(r, dict) and r.get("ok") is False and "theme" in r.get("error", ""), str(r))

        # what an app tells us: type (category), how it arrived, what it can do
        d.send("mail one", app="TypeApp", hints=["string:category:email.arrived"], t=300)
        d.send("mail two", app="TypeApp", hints=["string:category:email.arrived"], t=300)
        d.send("chat one", app="TypeApp", urgency="critical", t=300)
        d.send("web one", app="TypeApp", hints=["string:x-kde-origin-name:example.com"], t=300)
        got = d.wait_for(lambda: any(a["app"] == "TypeApp" and len(a.get("types", [])) >= 3 for a in (d.ipc("seenApps") or [])), 6)
        app = next((a for a in (d.ipc("seenApps") or []) if a["app"] == "TypeApp"), {})
        types = {t["key"]: t for t in app.get("types", [])}
        check("an app's notification types are captured (category + channel)", got and "email.arrived|app" in types and "general|app" in types and "general|web" in types, str(types))
        check("each type counts its notifications", types.get("email.arrived|app", {}).get("count") == 2, str(types))
        check("the capabilities seen are listed (urgency critical)", "critical" in types.get("general|app", {}).get("caps", []), str(types))
        check("the type's category and channel are separate fields (usable in rules)", types.get("email.arrived|app", {}).get("category") == "email.arrived" and types.get("general|web", {}).get("channel") == "web", str(types))


    def test_priority_and_focus(d):
        """Priority can be given to an app/site/type by a rule; focus mode lets only important notifications through."""
        cfg = {**BASE_CONFIG,
               "focus": {"modes": {"urgent": {"label": "Urgent only"},
                                   "work": {"label": "Work", "minPriority": "high", "allow": [{"appName": "^Signal$"}, {"origin": "calendar"}]}}},
               "rules": [{"name": "boss", "match": {"appName": "^Boss$"}, "priority": "urgent"},
                         {"name": "mail", "match": {"category": "^email"}, "priority": "high"},
                         {"name": "noise", "match": {"appName": "^Noise$"}, "priority": "low"},
                         *BASE_CONFIG["rules"]]}

        def sim(n, focus="", c=cfg):
            r = d.ipc("simulate", json.dumps({"config": c, "notification": n, "ignoreState": True, "focus": focus}))
            return r if isinstance(r, dict) else {}
        check("an app's urgency maps to a priority (critical = urgent, low = low, otherwise normal)",
              sim({"appName": "x", "urgency": "critical"})["style"]["priority"] == "urgent" and sim({"appName": "x", "urgency": "low"})["style"]["priority"] == "low" and sim({"appName": "x"})["style"]["priority"] == "normal")
        check("a rule gives an app a priority", sim({"appName": "Boss"})["style"]["priority"] == "urgent" and sim({"appName": "Noise", "urgency": "critical"})["style"]["priority"] == "low")
        check("a rule can give a kind of notification a priority", sim({"appName": "Any", "category": "email.arrived"})["style"]["priority"] == "high")
        check("without focus nothing is held", sim({"appName": "Noise"})["wouldShow"] is True)
        check("focus 'urgent' holds a normal notification", (r := sim({"appName": "Chatty"}, "urgent")) and r["wouldShow"] is False and r["hold"] == "focus urgent", str(r))
        check("focus 'urgent' lets an urgent one through (by app urgency)", sim({"appName": "Chatty", "urgency": "critical"}, "urgent")["wouldShow"] is True)
        check("focus 'urgent' lets a rule-urgent app through", sim({"appName": "Boss"}, "urgent")["wouldShow"] is True)
        check("a rule can demote a critical notification so focus holds it", sim({"appName": "Noise", "urgency": "critical"}, "urgent")["wouldShow"] is False)
        check("focus 'work' (min priority high) passes high, holds normal", sim({"appName": "Any", "category": "email.arrived"}, "work")["wouldShow"] is True and sim({"appName": "Any"}, "work")["wouldShow"] is False)
        check("a focus allow entry lets a named app through at any priority", sim({"appName": "Signal"}, "work")["wouldShow"] is True and sim({"appName": "Other", "origin": "calendar.example.com"}, "work")["wouldShow"] is True)
        check("an unknown focus mode holds nothing", sim({"appName": "Chatty"}, "ghost")["wouldShow"] is True)
        media_cfg = {**cfg, "rules": [{"name": "player", "match": {"appName": "^Podcast$"}, "media": True}, {"name": "none", "match": {"appName": "^Plain$"}, "media": False}, *cfg["rules"]]}
        pm = lambda app: sim({"appName": app}, c=media_cfg)["style"]
        check("pictures are on by default but audio/video players are off", pm("Other")["media"] is True and pm("Other")["players"] is False, str(pm("Other")))
        check("a rule can turn the players on for one app", pm("Podcast")["players"] is True and pm("Podcast")["media"] is True, str(pm("Podcast")))
        check("a rule can turn pictures and players off", pm("Plain")["media"] is False and pm("Plain")["players"] is False, str(pm("Plain")))
        long_rule = {**cfg, "rules": [{"name": "long", "match": {"summary": "a" * 501}}]}
        r = d.ipc("validate", json.dumps(long_rule))
        check("a pattern longer than 500 characters is refused", isinstance(r, dict) and r.get("ok") is False and "500" in r.get("error", ""), str(r))
        r = d.ipc("validate", json.dumps({**cfg, "rules": [{"name": "ok", "match": {"summary": "a" * 500}}]}))
        check("a 500-character pattern is accepted", isinstance(r, dict) and r.get("ok") is True, str(r))
        dnd = d.ipc("simulate", json.dumps({"config": cfg, "notification": {"appName": "Boss"}}))
        check("simulate reports the priority", isinstance(dnd, dict) and dnd["style"]["priority"] == "urgent")

        for name, bad in {"bad priority": {"rules": [{"name": "r", "priority": "huge"}]}, "bad media": {"rules": [{"name": "r", "media": "yes"}]},
                          "bad stack": {"rules": [{"name": "r", "stack": 1}]},
                          "bad mode priority": {"focus": {"modes": {"x": {"minPriority": "mega"}}}}, "bad mode name": {"focus": {"modes": {"bad name": {}}}},
                          "bad allow regex": {"focus": {"modes": {"x": {"allow": [{"appName": "(oops"}]}}}}, "bad allow field": {"focus": {"modes": {"x": {"allow": [{"nope": "a"}]}}}},
                          "bad chart": {"transfer": {"chart": "pie"}}}.items():
            r = d.ipc("validate", json.dumps({**cfg, **bad}))
            check(f"validate rejects {name}", isinstance(r, dict) and r.get("ok") is False, str(r))

        # the real thing: a daemon with these modes
        d.write_config(cfg)
        time.sleep(1.2)
        check("the daemon loads the focus config", (d.ipc("configStatus") or {}).get("ok") is True, str(d.ipc("configStatus")))
        st = d.ipc("focus", "urgent")
        check("focus can be turned on and shows in status", isinstance(st, dict) and st.get("focus") == "urgent" and st.get("holding") == "focus", str(st))
        d.send("held by focus", app="Chatty", t=500)
        d.send("urgent passes", app="Chatty", urgency="critical", t=500)
        d.send("boss passes", app="Boss", t=500)
        d.wait_for(lambda: len(d.ipc("history", 10) or []) >= 3, 5)
        by = {i["summary"]: i for i in (d.ipc("history", 10) or [])}
        check("focus holds a normal notification and keeps it in history", by.get("held by focus", {}).get("shown") is False and by["held by focus"].get("why") == "focus urgent", str(by.get("held by focus")))
        check("focus lets urgent and rule-urgent notifications through", by.get("urgent passes", {}).get("shown") is True and by.get("boss passes", {}).get("shown") is True, str(by))
        r = d.ipc("focus", "ghost")
        check("an unknown focus mode is refused with the names that exist", isinstance(r, dict) and "error" in r and "urgent" in r["error"], str(r))
        d.ipc("dnd", "on")
        d.send("boss in dnd", app="Boss", t=500)
        d.wait_for(lambda: any(i["summary"] == "boss in dnd" for i in (d.ipc("history", 5) or [])), 4)
        h = next((i for i in (d.ipc("history", 5) or []) if i["summary"] == "boss in dnd"), {})
        check("a rule-urgent notification passes Do Not Disturb like a critical one", h.get("shown") is True, str(h))
        d.ipc("dnd", "off")
        st = d.ipc("focus", "off")
        check("focus can be turned off", isinstance(st, dict) and st.get("focus") == "" and st.get("holding") == "", str(st))
        d.ipc("dismissAll")
        d.write_config(BASE_CONFIG)
        time.sleep(1.2)


    def test_stacking(d):
        """Notifications of one app/site/type can be stacked: one place with a count, click to expand, scroll when more than a list holds."""
        cfg = {**BASE_CONFIG, "maxVisible": 3,
               "rules": [{"name": "stack chatty mail", "match": {"appName": "^Stacky$", "category": "^email"}, "stack": True},
                         {"name": "stack grouped", "match": {"appName": "^Grouped$"}, "stack": True, "group": "Team"},
                         *BASE_CONFIG["rules"]]}
        d.write_config(cfg)
        time.sleep(1.2)
        d.ipc("dismissAll")
        d.wait_for(lambda: d.active() == 0, 4)
        for i in range(5):
            d.send(f"mail {i}", app="Stacky", hints=["string:category:email.arrived"], t=0)
        d.send("plain one", app="Plain", t=0)
        check("five stacked notifications and a plain one are all received", d.wait_for(lambda: sum((d.ipc("status") or {}).get("perZone", {}).values()) == 6, 6), str(d.ipc("status")))
        st = d.ipc("status")
        stk = [x for x in st.get("stacks", [])]
        check("status lists the stack with its count", len(stk) == 1 and stk[0]["count"] == 5 and stk[0]["expanded"] is False, str(st.get("stacks")))
        check("a collapsed stack takes one place (5 notifications + 1 plain = 2 on screen)", st["active"] == 2 and st["queued"] == 0, str(st))
        z, key = stk[0]["zone"], stk[0]["key"]
        st = d.ipc("stack", z, key, "toggle", 0)
        check("expanding shows up to one list's worth (3 of 5) plus the plain one", st["active"] == 4 and st["stacks"][0]["expanded"] is True, str(st))
        st = d.ipc("stack", z, key, "scroll", 1)
        check("scrolling keeps the same number visible", st["active"] == 4, str(st))
        st = d.ipc("stack", z, key, "toggle", 0)
        check("collapsing returns to one place", st["active"] == 2 and st["stacks"][0]["expanded"] is False, str(st))
        r = d.ipc("stack", "nope", key, "toggle", 0)
        check("an unknown zone or action is refused", isinstance(r, dict) and "error" in r and isinstance(d.ipc("stack", z, key, "explode", 0), dict) and "error" in d.ipc("stack", z, key, "explode", 0), str(r))
        st = d.ipc("stack", z, key, "clear", 0)
        gone = d.wait_for(lambda: not (d.ipc("status") or {}).get("stacks") and sum((d.ipc("status") or {}).get("perZone", {}).values()) == 1, 5)
        check("clearing a stack dismisses all of its notifications and leaves the rest", gone, str(d.ipc("status")))
        d.ipc("dismissAll")
        d.wait_for(lambda: d.active() == 0, 4)
        # a group name is also a stack key: two apps in one group stack together
        d.send("g1", app="Grouped", t=0)
        d.send("g2", app="Grouped", t=0)
        ok = d.wait_for(lambda: any(x["count"] == 2 for x in (d.ipc("status") or {}).get("stacks", [])), 5)
        check("notifications of a group stack under the group's name", ok, str(d.ipc("status")))
        d.ipc("dismissAll")
        d.wait_for(lambda: d.active() == 0, 4)
        d.write_config(BASE_CONFIG)
        time.sleep(1.2)

    def test_center_and_unread(d):
        d.ipc("clearHistory")
        d.send("times out", app="Steam", t=1200)
        d.send("disk full", app="systemd", t=1200)
        st = {}
        ok = d.wait_for(lambda: (st := d.ipc("status") or {}).get("unread") == 2, 5)
        st = d.ipc("status") or {}
        check("new notifications are unread, counted per group", ok and st.get("unreadPerZone", {}).get("system") == 1 and sum(st.get("unreadPerZone", {}).values()) == 2, str(st))
        d.wait_for(lambda: d.active() == 0, 5)
        d.send("dismissed", app="Steam", t=0)
        d.wait_for(lambda: d.active() == 1, 4)
        d.ipc("dismissAll")
        d.wait_for(lambda: d.active() == 0, 4)
        hist = {h["summary"]: h for h in (d.ipc("history", 10) or [])}
        check("closing a popup yourself marks it read; one that timed out stays unread",
              hist.get("dismissed", {}).get("read") is True and hist.get("times out", {}).get("read") is False, str(hist))
        check("...so the unread count only has the ones not seen", (d.ipc("status") or {}).get("unread") == 2, str(d.ipc("status")))
        st = d.ipc("markAllRead") or {}
        check("mark all read clears the count", st.get("unread") == 0 and all(h.get("read") for h in (d.ipc("history", 10) or [])), str(st))
        check("the center opens, toggles and closes", (d.ipc("center", "open") or {}).get("center") is True
              and (d.ipc("center", "toggle") or {}).get("center") is False and (d.ipc("center", "close") or {}).get("center") is False)
        r = d.ipc("center", "explode")
        check("an unknown center action is refused", isinstance(r, dict) and "error" in r, str(r))
        for corner, good in (("top-left", True), ("middle", False)):
            r = d.ipc("validate", json.dumps({**BASE_CONFIG, "center": {"anchor": corner}}))
            check(f"center.anchor {corner!r} is {'accepted' if good else 'refused'}", isinstance(r, dict) and r.get("ok") is good, str(r))
        d.ipc("clearHistory")

    return [test_style_and_types, test_priority_and_focus, test_stacking, test_center_and_unread]
