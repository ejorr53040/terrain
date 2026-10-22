#!/usr/bin/env python3
"""Dev-only: how well the hands were tracked in recorded clips: frames with 2, 1 and
0 hands, and each dropout (a hand lost between two frames with both).
Never shipped or run by the app.

    tools/clip_report.py [CLIP...]   # default: every clip
    tools/clip_report.py --self-check
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CLIPS = ROOT / "crates" / "terrain-desktop" / "tests" / "clips"


def tracking(frames):
    """Frame counts by hands seen, and how long each dropout lasted (ms):
    from the first frame missing a hand to the first with both again. A
    hand that never comes back (hands leaving at the end) isn't one."""
    counts = [len(f["hands"]) for f in frames]
    dropouts, lost_at, seen_two = [], None, False
    for f, n in zip(frames, counts):
        if n >= 2:
            if lost_at is not None:
                dropouts.append(f["t_ms"] - lost_at)
            lost_at, seen_two = None, True
        elif seen_two and lost_at is None:
            lost_at = f["t_ms"]
    return {
        "two": sum(n >= 2 for n in counts),
        "one": counts.count(1),
        "zero": counts.count(0),
        "dropouts_ms": dropouts,
    }


def report(names):
    print(f"{'clip':<26}{'2 hands':>8}{'1 hand':>8}{'none':>6}  dropouts (ms)")
    for name in names:
        t = tracking(json.loads((CLIPS / name / "frames.json").read_text()))
        drops = ", ".join(map(str, t["dropouts_ms"])) or "-"
        print(f"{name:<26}{t['two']:>8}{t['one']:>8}{t['zero']:>6}  {drops}")


def self_check():
    # 33 ms frames: hands appear, both at 66, one lost at 132 and back at
    # 231, both lost at 297 and back at 330, then a hand leaves for good.
    counts = [0, 1, 2, 2, 1, 1, 1, 2, 2, 0, 2, 2, 1, 1]
    frames = [{"t_ms": 33 * i, "hands": [{}] * n} for i, n in enumerate(counts)]
    got = tracking(frames)
    assert got == {"two": 6, "one": 6, "zero": 2, "dropouts_ms": [99, 33]}, got
    print("self-check ok")


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-check"]:
        self_check()
    else:
        report(sys.argv[1:] or sorted(p.parent.name for p in CLIPS.glob("*/frames.json")))
