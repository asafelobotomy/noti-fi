"""Rule validation, simulation, preview, schema and CLI tests. Run through test_notifd.py, which passes in its `check`, BASE_CONFIG and paths."""
import json, os, shutil, signal, socket, subprocess, sys, tempfile, time
from pathlib import Path


def make_tests(check, BASE_CONFIG, ROOT, SRC):
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
            ("site rule", dict(appName="Brave Origin", origin="example-shop.com", summary="sim-1"), {}, []),
            ("system app", dict(appName="kernel", summary="sim-2"), {}, []),
            ("silent rule", dict(appName="quietapp", summary="sim-3"), {}, []),
            ("default zone", dict(appName="Steam", summary="sim-4"), {}, []),
            ("dnd holds normal", dict(appName="Steam", summary="sim-5"), {"dnd": True}, []),
            ("dnd passes critical", dict(appName="kernel", summary="sim-6", urgency="critical"), {"dnd": True}, ["-u", "critical"]),
            ("muted zone", dict(appName="Brave Origin", origin="example-chat.com", summary="sim-7"), {"mute": "business"}, []),
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
        r = sim(d, {"appName": "Brave Origin", "origin": "example-shop.com", "summary": "x"})
        check("simulate names the rule that decided the zone", r and r["decidedBy"]["zone"] == "business sites" and "business sites" in r["matched"], str(r))
        check("simulate also reports the matched rule's position in the list", r and r["matchedIdx"] == [0] and r["decidedIdx"]["zone"] == 0, str(r))
        r = sim(d, {"appName": "kernel", "summary": "x", "urgency": "critical"})
        check("the system's own notifications are in System, built in", r and r["zone"] == "system" and r["decidedBy"]["zone"] == "System (built in)", str(r))
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


    def test_groups(d):
        """Groups (zones): the locked System group, the system's own notifications, and rules that apply to a whole group."""
        no_system = {**BASE_CONFIG, "zones": {k: v for k, v in BASE_CONFIG["zones"].items() if k != "system"}, "rules": []}
        r = sim(d, {"appName": "kernel", "summary": "x"}, config=no_system)
        check("the System group exists even when the config leaves it out", r and r["zone"] == "system", str(r))
        grab = {**BASE_CONFIG, "rules": [{"name": "grab", "match": {"appName": "^(kernel|upower)$"}, "zone": "business", "silent": True}]}
        r = sim(d, {"appName": "kernel", "summary": "low memory"}, config=grab)
        check("a rule cannot move the system's own notifications out of System", r and r["zone"] == "system", str(r))
        check("a rule cannot silence the system's own notifications", r and r["wouldShow"] and r["hold"] == "", str(r))
        d.ipc("mute", "system", "true")
        r = sim(d, {"appName": "systemd", "summary": "x"})
        stock = sim(d, {"appName": "Discover", "summary": "x"}, config={**BASE_CONFIG, "rules": [{"name": "stock", "match": {"appName": "^discover$"}, "zone": "system"}]})
        d.ipc("mute", "system", "false")
        check("muting System never hides the system's own notifications", r and r["hold"] == "", str(r))
        check("a stock app moved into System is muted with it", stock and stock["zone"] == "system" and stock["hold"] == "muted system", str(stock))
        moved = sim(d, {"appName": "Discover", "summary": "x"}, config={**BASE_CONFIG, "rules": [{"name": "out", "match": {"appName": "^discover$"}, "zone": "personal"}]})
        check("a stock app can be taken out of System", moved and moved["zone"] == "personal", str(moved))
        by_group = {**BASE_CONFIG, "rules": BASE_CONFIG["rules"] + [{"name": "business stays", "match": {"group": "^business$"}, "timeout": 0, "width": 460}]}
        r = sim(d, {"appName": "Brave Origin", "origin": "example-shop.com", "summary": "x"}, config=by_group)
        check("a rule can apply to a whole group", r and r["zone"] == "business" and r["timeout"] == 0 and r["decidedBy"]["timeout"] == "business stays", str(r))
        r = sim(d, {"appName": "Steam", "summary": "x"}, config=by_group)
        check("a group rule leaves other groups alone", r and r["zone"] == "personal" and r["timeout"] == 1500, str(r))
        bad = d.ipc("validate", json.dumps({**BASE_CONFIG, "rules": [{"name": "loop", "match": {"group": "^business$"}, "zone": "personal"}]}))
        check("a rule for a group cannot also move it to another group", isinstance(bad, dict) and bad.get("ok") is False and "group" in bad.get("error", ""), str(bad))


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
        if "const" in sch and v != sch["const"]:
            errs.append(f"{path}: {v!r} is not {sch['const']!r}")
        if "enum" in sch and v not in sch["enum"]:
            errs.append(f"{path}: {v!r} not in {sch['enum']}")
        num = isinstance(v, (int, float)) and not isinstance(v, bool)
        if "minimum" in sch and num and v < sch["minimum"]:
            errs.append(f"{path}: below minimum")
        if "maximum" in sch and num and v > sch["maximum"]:
            errs.append(f"{path}: above maximum")
        if isinstance(v, str) and ("minLength" in sch and len(v) < sch["minLength"] or "maxLength" in sch and len(v) > sch["maxLength"]):
            errs.append(f"{path}: length out of range")
        if isinstance(v, list) and ("minItems" in sch and len(v) < sch["minItems"] or "maxItems" in sch and len(v) > sch["maxItems"]):
            errs.append(f"{path}: item count out of range")
        if "if" in sch and not mini_schema_errors(v, sch["if"], path):
            errs += mini_schema_errors(v, sch.get("then", {}), path)
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
        for what, change in (("an opacity above 1", lambda c: c["theme"].__setitem__("opacity", 2)),
                             ("center.keep above 1000", lambda c: c.setdefault("center", {}).__setitem__("keep", 5000)),
                             ("21 ignoreCaptureBy entries", lambda c: c["autoDnd"].__setitem__("ignoreCaptureBy", ["x"] * 21))):
            c = json.loads(json.dumps(shipped)); c.setdefault("theme", {}); change(c)
            check(f"the schema catches {what}", bool(mini_schema_errors(c, schema)))
        # whatever the schema accepts structurally, the daemon must accept too (shipped file)
        v = d.ipc("validate", json.dumps(shipped))
        check("the daemon accepts the shipped config the schema accepts", v and v.get("ok") is True, str(v))

    def test_cli_smoke(d):
        """Every read-only CLI command runs to completion, against the test daemon's private bus and a throwaway home (never your own)."""
        built = [p for p in (ROOT / "target/release/notify", ROOT / "target/debug/notify") if p.exists()]
        if not built:
            check("a built notify exists (cargo build -p notify-cli)", False)
            return
        notify = [str(max(built, key=lambda p: p.stat().st_mtime)), "--no-systemctl"]          # the newest build
        home = Path(tempfile.mkdtemp(prefix="notify-smoke-"))
        env = dict(d.env, HOME=str(home), XDG_CONFIG_HOME=str(home / "config"), XDG_DATA_HOME=str(home / "data"),
                   XDG_STATE_HOME=str(home / "state"))
        try:
            for args in (["status"], ["status", "--json"], ["apps"], ["apps", "--json"], ["rules"], ["rules", "--json"],
                         ["rules", "--file", str(SRC / "rules.json")], ["doctor"], ["doctor", "--json"], ["autodnd", "status"], ["tray", "status"],
                         ["restore", "--dry-run"], ["takeover", "--dry-run"], ["portal", "--dry-run"],
                         ["hotkey", "--dry-run"], ["hotkey", "--center", "--dry-run"], ["adapt"], ["--help"]):
                r = subprocess.run([*notify, *args], capture_output=True, text=True, timeout=60, env=env)
                ok = r.returncode in (0, 1) and "panicked at" not in r.stderr
                check(f"notify {' '.join(args)} runs to completion (exit {r.returncode})", ok, (r.stderr or r.stdout)[-300:])
            r = subprocess.run([*notify, "status", "--json"], capture_output=True, text=True, timeout=30, env=env)
            try:
                j = json.loads(r.stdout)
                check("status --json is valid JSON with the expected keys", {"owner", "notifd_owns", "daemon", "config"} <= set(j) and j["daemon"] is not None, str(j)[:200])
            except ValueError:
                check("status --json is valid JSON with the expected keys", False, r.stdout[:200])
        finally:
            shutil.rmtree(home, ignore_errors=True)


    # ------------------------------------------------------------------ control socket

    return [test_validate_does_not_apply, test_simulate_agrees_with_reality, test_simulate_details, test_groups, test_preview, test_schema, test_cli_smoke]
