#!/usr/bin/env python3
"""Differential parity test: the Rust `notify` must do exactly what the Python `notify` does.

Runs both CLIs through the same lifecycle in two separate fake homes (no systemctl, nothing real is touched) and
compares every file they create and every line they print. Also checks the Rust-only JSON progress stream.

    cargo build -p notify-cli && python3 tests/parity.py
"""
import json, os, re, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PY = [sys.executable, "-I", str(ROOT / "legacy/notify-py")]
RS = [str(ROOT / "target/debug/notify")]
FAILS = []
PASSED = 0


def check(name, ok, detail=""):
    global PASSED
    if ok:
        PASSED += 1
        print(f"  [PASS] {name}")
    else:
        FAILS.append(name)
        print(f"  [FAIL] {name}\n{detail}")


class Home:
    def __init__(self, label):
        self.root = Path(tempfile.mkdtemp(prefix=f"notify-parity-{label}-"))
        self.env = dict(os.environ, XDG_CONFIG_HOME=str(self.root / "config"), XDG_DATA_HOME=str(self.root / "data"),
                        XDG_STATE_HOME=str(self.root / "state"))
        self.env.pop("NOTIFD_SOCKET", None)
        # nothing listens here, so both CLIs see "no daemon" the same way
        self.env["XDG_RUNTIME_DIR"] = str(self.root / "run")
        (self.root / "run").mkdir()

    def run(self, cli, *args):
        r = subprocess.run(cli + ["--no-systemctl", *args], env=self.env, capture_output=True, text=True, timeout=120)
        return r.returncode, self.norm(r.stdout), self.norm(r.stderr)

    def norm(self, text):
        return text.replace(str(self.root), "<HOME>")

    def tree(self):
        out = {}
        for p in sorted(self.root.rglob("*")):
            if p.name == "run" or "run" in p.relative_to(self.root).parts[:1]:
                continue
            rel = str(p.relative_to(self.root))
            if p.is_symlink():
                out[rel] = ("link", self.norm(os.readlink(p)))
            elif p.is_file():
                out[rel] = ("file", self.norm(p.read_text()))
            elif p.is_dir():
                out[rel] = ("dir", "")
        return out


def same_tree(a, b):
    diffs = []
    for k in sorted(set(a) | set(b)):
        if k not in a:
            diffs.append(f"only in rust: {k}")
        elif k not in b:
            diffs.append(f"only in python: {k}")
        elif a[k] != b[k]:
            if k.endswith("takeover.json"):
                if json.loads(a[k][1]) != json.loads(b[k][1]):
                    diffs.append(f"differs (parsed): {k}\n  py={a[k][1]}\n  rs={b[k][1]}")
            elif k.endswith("rules.json") and a[k][0] == "file":
                # key order / indentation may differ; the data must not
                if json.loads(a[k][1]) != json.loads(b[k][1]):
                    diffs.append(f"differs (parsed): {k}")
            else:
                diffs.append(f"differs: {k}\n--- python\n{a[k][1]}\n--- rust\n{b[k][1]}")
    return diffs


def compare_run(name, py_home, rs_home, *args, ignore_lines=()):
    rp = py_home.run(PY, *args)
    rr = rs_home.run(RS, *args)
    def clean(t):
        return "\n".join(l for l in t.splitlines() if not any(i in l for i in ignore_lines))
    def said(r):          # everything the command said; Rust prefixes failures with "error: " on stderr
        return clean((r[1] + "\n" + r[2].removeprefix("error: ")).strip()).strip()
    ok = rp[0] == rr[0] and said(rp) == said(rr)
    check(name, ok, f"exit py={rp[0]} rs={rr[0]}\n--- python\n{said(rp)}\n--- rust\n{said(rr)}")
    return rp, rr


def main():
    if not Path(RS[0]).exists():
        sys.exit("build the Rust CLI first: cargo build -p notify-cli")
    py, rs = Home("py"), Home("rs")

    print("dry runs create nothing and print the same plan")
    compare_run("takeover --dry-run prints the same plan", py, rs, "takeover", "--dry-run", "--now")
    check("...and creates no files (python)", not py.tree())
    check("...and creates no files (rust)", not rs.tree())
    compare_run("restore --dry-run prints the same plan", py, rs, "restore", "--dry-run")
    compare_run("restore --dry-run --purge prints the same plan", py, rs, "restore", "--dry-run", "--purge")
    compare_run("portal --dry-run prints the same plan", py, rs, "portal", "--dry-run")
    compare_run("portal --remove --dry-run prints the same plan", py, rs, "portal", "--remove", "--dry-run")
    compare_run("hotkey --dry-run prints the same line", py, rs, "hotkey", "--dry-run")
    compare_run("hotkey Ctrl+Alt+F5 --dry-run", py, rs, "hotkey", "Ctrl+Alt+F5", "--dry-run")

    print("install")
    rp, rr = compare_run("takeover prints the same output", py, rs, "takeover")
    diffs = same_tree(py.tree(), rs.tree())
    check("takeover creates identical files (units, D-Bus override, portal route, config, state record)", not diffs, "\n".join(diffs))
    compare_run("a second takeover refuses the same way", py, rs, "takeover")

    print("update")
    for h in (py, rs):                      # same drift on both sides: an old config missing newer settings, one user edit
        p = h.root / "config/notifd/rules.json"
        d = json.loads(p.read_text())
        d.pop("sound"); d.pop("theme"); d["defaultTimeout"] = 9999
        p.write_text(json.dumps(d))
    ru = py.run(PY, "update"), rs.run(RS, "update")
    check("update succeeds on both", ru[0][0] == 0 and ru[1][0] == 0, f"{ru[0]} / {ru[1]}")
    diffs = same_tree(py.tree(), rs.tree())
    check("update leaves identical files (payload, previous version, refreshed units, merged config)", not diffs, "\n".join(diffs))
    for h, label in ((py, "python"), (rs, "rust")):
        d = json.loads((h.root / "config/notifd/rules.json").read_text())
        check(f"update merges new defaults and keeps user values ({label})", "sound" in d and "theme" in d and d["defaultTimeout"] == 9999)

    print("portal on an existing foreign file")
    compare_run("portal --remove removes what notify made", py, rs, "portal", "--remove")
    for h in (py, rs):
        (h.root / "config/xdg-desktop-portal").mkdir(parents=True, exist_ok=True)
        (h.root / "config/xdg-desktop-portal/kde-portals.conf").write_text("[preferred]\n")
    compare_run("portal refuses to overwrite a file it did not create", py, rs, "portal", ignore_lines=("portal routing was not enabled",))   # Rust adds a clearer closing line; same exit code
    diffs = same_tree(py.tree(), rs.tree())
    check("...and leaves the foreign file alone, identically", not diffs, "\n".join(diffs))

    print("restore")
    for h in (py, rs):
        (h.root / "config/xdg-desktop-portal/kde-portals.conf").unlink()
    compare_run("restore prints the same output", py, rs, "restore")
    diffs = same_tree(py.tree(), rs.tree())
    check("restore removes exactly the same things", not diffs, "\n".join(diffs))
    check("...and keeps the config and installed daemon (both)", (rs.root / "config/notifd/rules.json").exists() and (rs.root / "data/notifd/shell.qml").exists())
    compare_run("restore --purge removes the daemon files the same way", py, rs, "restore", "--purge")
    check("...identically", not same_tree(py.tree(), rs.tree()), "\n".join(same_tree(py.tree(), rs.tree())))

    print("lint and config checks")
    good = ROOT / "notifd/rules.json"
    compare_run("rules --file on the shipped config", py, rs, "rules", "--file", str(good))
    bad = json.loads(good.read_text())
    bad["zones"]["system"]["anchor"] = "middle"
    bad["defualtZone"] = "x"
    bad["rules"] += [{"name": "named group", "match": {"appName": "(?P<x>steam)"}},
                     {"name": "inline flag", "match": {"appName": "(?i)steam"}},
                     {"name": "ghost zone", "match": {}, "zone": "ghost"},
                     {"name": "bad field", "match": {"nope": "a"}}]
    bp = Path(tempfile.mkdtemp(prefix="notify-parity-bad-")) / "bad.json"
    bp.write_text(json.dumps(bad))
    rp = py.run(PY, "rules", "--file", str(bp))
    rr = rs.run(RS, "rules", "--file", str(bp))
    check("both reject the bad config (exit 1)", rp[0] == 1 and rr[0] == 1, f"{rp[0]} {rr[0]}")
    for needle in ("anchor must be", "unknown zone 'ghost'", "unknown match field 'nope'", "unknown setting 'defualtZone'", "named groups", "inline flags"):
        check(f"both report: {needle}", needle in rp[1] and needle in rr[1], f"py:{needle in rp[1]} rs:{needle in rr[1]}")

    print("Rust-only: JSON progress events")
    h = Home("json")
    r = subprocess.run(RS + ["--no-systemctl", "--json", "takeover", "--dry-run"], env=h.env, capture_output=True, text=True)
    lines = [json.loads(l) for l in r.stdout.splitlines() if l.strip()]
    plan = next((l for l in lines if l.get("type") == "plan"), None)
    check("takeover --json --dry-run emits a plan event listing every step", plan and plan["dry_run"] is True and plan["total"] == len(plan["steps"]) >= 9, str(plan))
    check("...and a closing result event", lines and lines[-1].get("type") == "result" and lines[-1]["ok"] is True, str(lines[-1:]))
    r = subprocess.run(RS + ["--no-systemctl", "--json", "takeover"], env=h.env, capture_output=True, text=True)
    lines = [json.loads(l) for l in r.stdout.splitlines() if l.strip()]
    steps = [l for l in lines if l.get("type") == "step"]
    total = lines[0]["total"] if lines else 0
    check("a real run reports every step as running then done, in order",
          [ (s["n"], s["status"]) for s in steps ] == [ (n, st) for n in range(1, total + 1) for st in ("running", "done") ], str(steps[:6]))
    check("...ending with a successful result", lines[-1] == {"type": "result", "ok": True, "message": "\nDone. Undo any time with: notify restore"}, str(lines[-1]))
    r = subprocess.run(RS + ["--no-systemctl", "--json", "takeover"], env=h.env, capture_output=True, text=True)
    last = json.loads(r.stdout.strip().splitlines()[-1])
    check("a refused command reports a failed result event and exits 1", r.returncode == 1 and last["type"] == "result" and last["ok"] is False, f"{r.returncode} {last}")
    r = subprocess.run(RS + ["--no-systemctl", "--json", "restore"], env=h.env, capture_output=True, text=True)
    lines = [json.loads(l) for l in r.stdout.splitlines() if l.strip()]
    check("restore --json also streams plan, steps and a result", lines[0]["type"] == "plan" and lines[-1]["ok"] is True and sum(1 for l in lines if l.get("status") == "done") == lines[0]["total"], str(lines[:2]))

    print(f"\n{PASSED} passed, {len(FAILS)} failed")
    for f in FAILS:
        print("  FAILED:", f)
    sys.exit(1 if FAILS else 0)


if __name__ == "__main__":
    main()
