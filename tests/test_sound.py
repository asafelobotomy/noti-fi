"""Sound playback: the daemon must call a player for sound-name / sound-file hints, and must not for suppress-sound or when sound is off.
A recording stand-in for pw-play is put first on the PATH of a private daemon, so nothing is heard. Run: python3 tests/test_sound.py"""
import json, os, stat, sys, tempfile, time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import test_notifd as T


def main():
    stub = Path(tempfile.mkdtemp(prefix="notifd-sound-"))
    log = stub / "played.log"
    player = stub / "pw-play"
    player.write_text(f'#!/bin/sh\necho "$@" >> {log}\n')
    player.chmod(player.stat().st_mode | stat.S_IEXEC)
    # pw-play is tried first by the daemon's script, but only a named sound goes to canberra first on some setups: hide the others
    for other in ("paplay", "canberra-gtk-play"):
        (stub / other).write_text(f'#!/bin/sh\necho "{other} $@" >> {log}\n')
        (stub / other).chmod(0o755)
    d = T.Daemon()
    d.env["PATH"] = f"{stub}:{os.environ['PATH']}"
    played = lambda: log.read_text().strip().splitlines() if log.exists() else []
    try:
        d.start()
        d.send("file sound", t=300, hints=["string:sound-file:/usr/share/sounds/freedesktop/stereo/message.oga"])
        T.check("a sound-file hint is played", d.wait_for(lambda: any("message.oga" in l for l in played()), 5), str(played()))
        d.send("named sound", t=300, hints=["string:sound-name:message-new-instant"])
        T.check("a sound-name hint is played", d.wait_for(lambda: any("message-new-instant" in l for l in played()), 5), str(played()))
        n = len(played())
        d.send("suppressed", t=300, hints=["string:sound-name:dialog-warning", "boolean:suppress-sound:true"])
        time.sleep(1.5)
        T.check("suppress-sound plays nothing", len(played()) == n, str(played()))
        d.send("evil name", t=300, hints=["string:sound-name:x; touch " + str(stub / "pwned")])
        time.sleep(1.5)
        T.check("a sound name with shell characters is never run", not (stub / "pwned").exists())
        # a flood plays at most two sounds at once: the player now lasts a few seconds
        player.write_text(f'#!/bin/sh\necho "$@" >> {log}\nsleep 3\n')
        n = len(played())
        for i in range(5):
            d.send(f"flood {i}", t=300, hints=[f"string:sound-name:flood-{i}"])
        time.sleep(1.5)
        T.check("a flood of sounds plays at most two at once", len(played()) - n == 2, str(played()[n:]))
        time.sleep(3)
        d.send("after the flood", t=300, hints=["string:sound-name:after-flood"])
        T.check("sounds play again once the flood's players exit", d.wait_for(lambda: any("after-flood" in l for l in played()), 5), str(played()))
        cfg = json.loads(json.dumps(T.BASE_CONFIG)); cfg["sound"] = {"enabled": False, "defaultName": ""}
        d.write_config(cfg); time.sleep(1.5)
        n = len(played())
        d.send("muted by config", t=300, hints=["string:sound-name:bell"])
        time.sleep(1.5)
        T.check("sound.enabled=false plays nothing", len(played()) == n, str(played()))
    finally:
        d.stop()
    failed = [r for r in T.RESULTS if not r[1]]
    print(f"\n{len(T.RESULTS) - len(failed)} passed, {len(failed)} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
