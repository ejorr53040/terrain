#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy>=2"]
# ///
"""Dev-only accuracy metrics for terrain's hand pipeline, over the recorded clips.

Replays every clip in crates/terrain-desktop/tests/clips through the real
estimator + gesture engine (examples/dump_trace.rs) and measures, per clip and
in total:

  A. gestures      matched / missed / misfired events vs tools/autoresearch/expected.json,
                   wrong direction, control errors, buttons held after the hand left
  B. hand loss     dropouts (2 hands -> fewer), single-hand gaps, track breaks
  C. model spikes  bone-length breaks, palm jumps (xy, depth), pinch-gap jumps, low scores
  D. smoothing     jitter kept at rest (smoothed/raw), lag while moving, spike pass-through
  E. delay         smoothing lag + pose-onset-to-action latency per event type
  F. wrong hand    handedness flips in a track, same-label pairs, ID swaps, ghost tracks,
                   extra hands in one-hand clips

    uv run tools/autoresearch/metrics.py          # table + score (or run it directly)
    tools/autoresearch/metrics.py --json out.json # also write everything
    tools/autoresearch/metrics.py --split train   # only training clips (autoresearch loop)
    tools/autoresearch/metrics.py --split holdout

Needs the trace dumper built:
    cargo build --release -p terrain-desktop --example dump_trace
"""

import argparse
import json
import math
import os
import subprocess
from collections import defaultdict
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
CLIPS = ROOT / "crates" / "terrain-desktop" / "tests" / "clips"
EXPECTED = Path(__file__).with_name("expected.json")
DUMP = ROOT / "target" / "release" / "examples" / "dump_trace"

# Clips the autoresearch loop never sees while iterating: checked only to
# accept a change, so tuning to 17 clips can't silently overfit them.
HOLDOUT = {"drag_after_hold", "fist_push", "full_screen_motion",
           "vee_sideways", "leave_mid_drag_after_hold"}
# Clips performed with two hands on purpose; in the rest a 2nd hand is extra.
TWO_HAND = {"approach_pinch", "full_screen_motion", "not_full_screen",
            "spread_in_control", "spread_short", "typing"}

BONES = [(0, 1), (1, 2), (2, 3), (3, 4), (0, 5), (5, 6), (6, 7), (7, 8),
         (5, 9), (9, 10), (10, 11), (11, 12), (9, 13), (13, 14), (14, 15),
         (15, 16), (13, 17), (0, 17), (17, 18), (18, 19), (19, 20)]
PALM = [0, 5, 9, 13, 17]
FINGERS = [(5, 8), (9, 12), (13, 16), (17, 20)]

# Thresholds (documented in docs/research/autoresearch-baseline.md).
BONE_BREAK = 0.30        # a bone more than 30% off its clip median
XY_SPIKE_M = 0.03        # raw palm 2nd difference across the view, per frame
Z_SPIKE_M = 0.06         # ...in depth (a webcam judges depth worse)
PINCH_SPIKE_M = 0.025    # thumb-index gap 2nd difference
LOW_SCORE = 0.8
STILL_MPS = 0.08         # smoothed palm speed under this: at rest
MOVING_MPS = 0.30        # over this: moving
GHOST_FRAMES = 5         # tracks shorter than this are ghosts
SCROLL_MIN_STEPS = 1.0   # a scroll run smaller than this didn't happen
DRAG_MIN_MOVE = 0.03     # screen fraction: less is a click
EARLY_RELEASE_MS = 50    # a drag released this long before the hand left ended early


# ---------------------------------------------------------------- loading

def run_trace(clip):
    out = subprocess.run([str(DUMP), str(CLIPS / clip / "frames.json")],
                         check=True, capture_output=True, text=True,
                         env=os.environ)
    return [json.loads(line) for line in out.stdout.splitlines()]


def load_frames(clip):
    return json.loads((CLIPS / clip / "frames.json").read_text())


# ---------------------------------------------------------------- A. gestures

def events_from_trace(trace):
    """Turn the action stream into logical events with times."""
    events, held, at = [], {}, (0.0, 0.0)
    scroll = None  # [t0, t1, sx, sy]

    def flush_scroll():
        nonlocal scroll
        if scroll and math.hypot(scroll[2], scroll[3]) >= SCROLL_MIN_STEPS:
            sx, sy = scroll[2], scroll[3]
            d = ("right" if sx > 0 else "left") if abs(sx) > abs(sy) else ("down" if sy > 0 else "up")
            events.append({"type": "scroll", "dir": d, "t": scroll[0], "t_end": scroll[1]})
        scroll = None

    for f in trace:
        t = f["t_ms"]
        acts = f["actions"]
        scrolled = False
        for a in acts:
            if a.startswith("PointTo"):
                _, x, y = a.split()
                at = (float(x), float(y))
            elif a.startswith("Scroll"):
                _, x, y = a.split()
                if scroll is None:
                    scroll = [t, t, 0.0, 0.0]
                scroll[1] = t
                scroll[2] += float(x)
                scroll[3] += float(y)
                scrolled = True
        if not scrolled:
            flush_scroll()
        presses = [a[6:-1] for a in acts if a.startswith("Press(")]
        releases = [a[8:-1] for a in acts if a.startswith("Release(")]
        for b in presses:
            held[b] = (t, at)
        # chords that press and release in one frame
        if "F" in presses and "Super" in presses:
            events.append({"type": "fullscreen", "t": t})
        for k, d in (("H", "left"), ("L", "right"), ("K", "up"), ("J", "down")):
            if k in presses and "Ctrl" in presses:
                events.append({"type": "pane", "dir": d, "t": t})
        for k, d in (("ArrowLeft", "left"), ("ArrowRight", "right"),
                     ("ArrowUp", "up"), ("ArrowDown", "down")):
            if k in presses and "Alt" in presses:
                events.append({"type": "nav", "dir": d, "t": t})
        if "X" in presses:
            events.append({"type": "dictation", "t": t})
        for b in releases:
            if b not in held:
                continue
            t0, p0 = held.pop(b)
            if b == "Left":
                dx, dy = at[0] - p0[0], at[1] - p0[1]
                window = "Super" in held or "Super" in releases
                moved = math.hypot(dx, dy) >= DRAG_MIN_MOVE
                d = (("right" if dx > 0 else "left") if abs(dx) > abs(dy)
                     else ("down" if dy > 0 else "up"))
                if window:
                    events.append({"type": "window_drag", "dir": d if moved else None, "t": t0, "t_end": t})
                elif moved:
                    events.append({"type": "drag", "dir": d, "t": t0, "t_end": t})
                else:
                    events.append({"type": "click", "t": t0, "t_end": t})
            elif b == "Right":
                events.append({"type": "right_click", "t": t0, "t_end": t})
    flush_scroll()
    events.sort(key=lambda e: e["t"])
    return events, held


def label(e):
    return e["type"] + (":" + e["dir"] if e.get("dir") else "")


def align(expected, got):
    """Edit-distance alignment. A same-type event in the wrong direction is
    'wrong' (cost 1) rather than a miss plus a misfire (cost 2)."""
    n, m = len(expected), len(got)

    def cost(e, g):
        et, _, ed = e.partition(":")
        if et != g["type"]:
            return None
        return 0 if (not ed or ed == g.get("dir")) else 1

    D = np.zeros((n + 1, m + 1))
    D[:, 0] = np.arange(n + 1)
    D[0, :] = np.arange(m + 1)
    for i in range(1, n + 1):
        for j in range(1, m + 1):
            c = cost(expected[i - 1], got[j - 1])
            best = min(D[i - 1, j] + 1, D[i, j - 1] + 1)
            if c is not None:
                best = min(best, D[i - 1, j - 1] + c)
            D[i, j] = best
    i, j = n, m
    matched, wrong, missed, extra = [], [], [], []
    while i > 0 or j > 0:
        if i > 0 and j > 0:
            c = cost(expected[i - 1], got[j - 1])
            if c is not None and D[i, j] == D[i - 1, j - 1] + c:
                (matched if c == 0 else wrong).append((expected[i - 1], got[j - 1]))
                i, j = i - 1, j - 1
                continue
        if i > 0 and D[i, j] == D[i - 1, j] + 1:
            missed.append(expected[i - 1])
            i -= 1
        else:
            extra.append(got[j - 1])
            j -= 1
    return matched[::-1], wrong[::-1], missed[::-1], extra[::-1]


def gesture_metrics(clip, trace, key):
    events, still_held = events_from_trace(trace)
    optional = set(key.get("optional", []))
    exp = key["events"]
    matched, wrong, missed, extra = align(exp, events)
    # optional events (at most one each) are not misfires
    kept = []
    for e in extra:
        if e["type"] in optional:
            optional.discard(e["type"])
        else:
            kept.append(e)
    extra = kept
    took = any(f["control"] is not None for f in trace)
    control_err = 0
    if key.get("control") is True and not took:
        control_err = 1
    if key.get("control") is False and took:
        control_err = 1
    # buttons still held at the end, or released late/early around the hand leaving
    stuck = len(still_held)
    late_ms = None
    limit = key.get("release_within_ms_of_vanish")
    if limit:
        # first frame of the final absence: when the hand has left for good
        gone = next((f["t_ms"] for i, f in enumerate(trace)
                     if f["n_raw"] == 0 and all(g["n_raw"] == 0 for g in trace[i:])), None)
        rel = [e for e in events if e["type"] == "drag"]
        if rel and gone is not None:
            late_ms = rel[-1]["t_end"] - gone   # negative = released while still in view
            if late_ms > limit or late_ms < -EARLY_RELEASE_MS:
                stuck += 1
    return {
        "expected": len(exp), "got": len(events),
        "matched": len(matched), "wrong_dir": len(wrong),
        "missed": len(missed), "misfires": len(extra),
        "control_err": control_err, "stuck_or_early_release": stuck,
        "release_vs_vanish_ms": late_ms,
        "events": [label(e) + f"@{e['t']}" for e in events],
        "missed_list": missed, "misfire_list": [label(e) + f"@{e['t']}" for e in extra],
        "matched_pairs": [(e, g) for e, g in matched + wrong],
    }


# ---------------------------------------------------------------- raw-landmark helpers

def palm_img(h):
    return np.mean([h["image"][i][:2] for i in PALM], axis=0)


def pinch_gap(h):
    return float(np.linalg.norm(np.subtract(h["world"][4], h["world"][8])))


def finger_reach(h):
    w = np.array(h["world"][0])
    return max(np.linalg.norm(np.array(h["world"][t]) - w) /
               max(np.linalg.norm(np.array(h["world"][k]) - w), 1e-6) for k, t in FINGERS)


def tracks_from_trace(trace, frames):
    """id -> list of samples: smoothed pos, raw pos, in_image, handedness, pose, raw hand."""
    tracks = defaultdict(list)
    for f, fr in zip(trace, frames):
        raw_hands = fr["hands"]
        used = set()
        for h in f["hands"]:
            best, bi = 9, None
            for i, rh in enumerate(raw_hands):
                if i in used:
                    continue
                d = np.hypot(*(np.array(h["in_image"]) - palm_img(rh)))
                if d < best:
                    best, bi = d, i
            used.add(bi)
            tracks[h["id"]].append({
                "t": f["t_ms"], "pos": np.array(h["pos"]),
                "raw": np.array(h["raw_pos"]) if h["raw_pos"] else np.array(h["pos"]),
                "img": np.array(h["in_image"]), "hand": h["handedness"],
                "pose": h["pose"], "rawhand": raw_hands[bi] if bi is not None else None,
            })
    return tracks


def contiguous(samples, max_gap_ms=40):
    """Split a track into runs of consecutive frames."""
    runs, cur = [], []
    for s in samples:
        if cur and s["t"] - cur[-1]["t"] > max_gap_ms:
            runs.append(cur)
            cur = []
        cur.append(s)
    if cur:
        runs.append(cur)
    return runs


# ---------------------------------------------------------------- B. hand loss

def loss_metrics(clip, frames, tracks, intentional=False):
    counts = [len(f["hands"]) for f in frames]
    dropouts, lost_at, seen_two = [], None, False
    for f, n in zip(frames, counts):
        if n >= 2:
            if lost_at is not None:
                dropouts.append(f["t_ms"] - lost_at)
            lost_at, seen_two = None, True
        elif seen_two and lost_at is None:
            lost_at = f["t_ms"]
    # single-track gaps, and track breaks: a track ends and a new one starts
    # within 1 s, near where it ended
    gaps = []
    for samples in tracks.values():
        for a, b in zip(samples, samples[1:]):
            if b["t"] - a["t"] > 40:
                gaps.append(b["t"] - a["t"])
    ends = sorted(((s[-1]["t"], s[-1]["img"]) for s in tracks.values()), key=lambda x: x[0])
    starts = sorted(((s[0]["t"], s[0]["img"]) for s in tracks.values()), key=lambda x: x[0])
    breaks, break_ms = 0, []
    for te, pe in ends:
        for ts, ps in starts:
            if 0 < ts - te <= 1000 and np.hypot(*(ps - pe)) < 0.25:
                breaks += 1
                break_ms.append(ts - te)
                break
    # loss while moving: raw palm speed over the 2 frames before a gap
    moving_losses = 0
    for samples in tracks.values():
        for i, (a, b) in enumerate(zip(samples, samples[1:])):
            if b["t"] - a["t"] > 40 and i >= 2:
                dt = max((samples[i]["t"] - samples[i - 2]["t"]) / 1000, 1e-3)
                v = np.linalg.norm(samples[i]["raw"] - samples[i - 2]["raw"]) / dt
                moving_losses += v > MOVING_MPS
    return {
        "frames_2": sum(n >= 2 for n in counts), "frames_1": counts.count(1),
        "frames_0": counts.count(0),
        # hands leaving on purpose (not_full_screen) aren't tracking losses
        "dropouts_ms": dropouts, "dropout_total_ms": 0 if intentional else sum(dropouts),
        "dropout_max_ms": 0 if intentional else max(dropouts, default=0),
        "track_gaps_ms": gaps, "track_breaks": breaks, "track_break_ms": break_ms,
        "losses_while_moving": int(moving_losses),
    }


# ---------------------------------------------------------------- C. model spikes

def spike_metrics(frames, tracks):
    bone_dev, bone_breaks, n = [], 0, 0
    xy_spikes = z_spikes = pinch_spikes = 0
    low = 0
    second_xy, second_z = [], []
    for samples in tracks.values():
        hands = [s["rawhand"] for s in samples if s["rawhand"]]
        if len(hands) < 3:
            continue
        L = np.array([[np.linalg.norm(np.subtract(h["world"][a], h["world"][b])) for a, b in BONES] for h in hands])
        med = np.median(L, axis=0)
        dev = np.max(np.abs(L / np.maximum(med, 1e-6) - 1), axis=1)
        bone_dev += list(dev)
        bone_breaks += int((dev > BONE_BREAK).sum())
        low += sum(h["score"] < LOW_SCORE for h in hands)
        n += len(hands)
        for run in contiguous(samples):
            if len(run) < 3:
                continue
            R = np.array([s["raw"] for s in run])
            d2 = R[2:] - 2 * R[1:-1] + R[:-2]
            sxy = np.linalg.norm(d2[:, :2], axis=1)
            sz = np.abs(d2[:, 2])
            second_xy += list(sxy)
            second_z += list(sz)
            xy_spikes += int((sxy > XY_SPIKE_M).sum())
            z_spikes += int((sz > Z_SPIKE_M).sum())
            G = np.array([pinch_gap(s["rawhand"]) for s in run if s["rawhand"]])
            if len(G) >= 3:
                pinch_spikes += int((np.abs(G[2:] - 2 * G[1:-1] + G[:-2]) > PINCH_SPIKE_M).sum())

    def q(a, p):
        return float(np.percentile(a, p)) if len(a) else 0.0

    return {
        "hand_frames": n,
        "bone_breaks": bone_breaks, "bone_dev_p95": q(bone_dev, 95),
        "xy_spikes": xy_spikes, "z_spikes": z_spikes, "pinch_spikes": pinch_spikes,
        "xy_jerk_p99_cm": 100 * q(second_xy, 99), "z_jerk_p99_cm": 100 * q(second_z, 99),
        "low_score_frames": int(low),
    }


# ---------------------------------------------------------------- D/E. smoothing + delay

def smoothing_metrics(tracks):
    jit_raw_xy, jit_s_xy, jit_raw_z, jit_s_z = [], [], [], []
    lag_err = defaultdict(list)   # shift_ms -> squared errors while moving
    err_moving = []
    spike_pass = []
    shifts = list(range(0, 201, 11))
    for samples in tracks.values():
        for run in contiguous(samples):
            if len(run) < 8:
                continue
            t = np.array([s["t"] for s in run], float)
            R = np.array([s["raw"] for s in run])
            S = np.array([s["pos"] for s in run])
            dt = np.diff(t) / 1000
            v = np.linalg.norm(np.diff(S, axis=0), axis=1) / np.maximum(dt, 1e-3)
            for k in range(1, len(run) - 1):
                d2r = R[k + 1] - 2 * R[k] + R[k - 1]
                d2s = S[k + 1] - 2 * S[k] + S[k - 1]
                if v[k - 1] < STILL_MPS and v[k] < STILL_MPS:
                    jit_raw_xy.append(np.linalg.norm(d2r[:2]) ** 2)
                    jit_s_xy.append(np.linalg.norm(d2s[:2]) ** 2)
                    jit_raw_z.append(d2r[2] ** 2)
                    jit_s_z.append(d2s[2] ** 2)
                if np.linalg.norm(d2r[:2]) > XY_SPIKE_M:
                    spike_pass.append(np.linalg.norm(d2s[:2]) / np.linalg.norm(d2r[:2]))
            moving = np.concatenate([[False], v > MOVING_MPS])
            for k in np.where(moving)[0]:
                err_moving.append(np.linalg.norm(S[k, :2] - R[k, :2]))
                for sh in shifts:
                    tt = t[k] - sh
                    if tt < t[0]:
                        continue
                    rx = [np.interp(tt, t, R[:, d]) for d in range(2)]
                    lag_err[sh].append(np.sum((S[k, :2] - rx) ** 2))

    def rms(a):
        return float(np.sqrt(np.mean(a))) if a else 0.0

    lag = None
    if lag_err:
        n = min(len(v) for v in lag_err.values())
        lag = min(shifts, key=lambda s: np.mean(lag_err[s][:n]) if lag_err[s] else 9e9)
    return {
        "jitter_raw_xy_mm": 1000 * rms(jit_raw_xy), "jitter_smooth_xy_mm": 1000 * rms(jit_s_xy),
        "jitter_kept_xy": rms(jit_s_xy) / max(rms(jit_raw_xy), 1e-9),
        "jitter_raw_z_mm": 1000 * rms(jit_raw_z), "jitter_smooth_z_mm": 1000 * rms(jit_s_z),
        "jitter_kept_z": rms(jit_s_z) / max(rms(jit_raw_z), 1e-9),
        "lag_ms_moving": lag, "err_moving_cm": 100 * float(np.mean(err_moving)) if err_moving else 0.0,
        "spike_pass_through": float(np.mean(spike_pass)) if spike_pass else None,
        "_still_n": len(jit_raw_xy), "_moving_n": len(err_moving),
    }


def _palms_apart(hs):
    """Palm separation in metres: image distance scaled by hand size."""
    a, b = palm_img(hs[0]), palm_img(hs[1])
    size_img = np.mean([np.linalg.norm(np.subtract(h["image"][0][:2], h["image"][9][:2])) for h in hs[:2]])
    size_m = np.mean([np.linalg.norm(np.subtract(h["world"][0], h["world"][9])) for h in hs[:2]])
    return float(np.linalg.norm(a - b) / max(size_img, 1e-6) * size_m)


def onset_latency(gm, tracks, frames):
    """For each matched event, ms from the raw frames showing its cause
    (within 1.5 s before) to the event firing."""
    out = defaultdict(list)
    raw = [(f["t_ms"], f["hands"]) for f in frames]

    def first_true(pred, t_event):
        """Start of the latest run of frames where pred holds, at or before t_event."""
        t_first, run_start = None, None
        for t, hands in raw:
            if t > t_event:
                break
            if t < t_event - 1500:
                continue
            if pred(hands):
                if run_start is None:
                    run_start = t
                t_first = run_start
            else:
                run_start = None
        return t_first

    def last_true(pred, t_event):
        t_last = None
        for t, hands in raw:
            if t > t_event:
                break
            if t >= t_event - 1500 and pred(hands):
                t_last = t
        return t_last

    for e, g in gm["matched_pairs"]:
        typ = g["type"]
        if typ in ("click", "right_click"):
            # these fire as the pinch opens: from the last frame it read closed
            t0 = last_true(lambda hs: any(pinch_gap(h) < 0.04 for h in hs), g["t"])
        elif typ == "fullscreen":
            # from the palms starting to part (last frame within 20 cm)
            t0 = last_true(lambda hs: len(hs) >= 2 and _palms_apart(hs) < 0.20, g["t"])
        elif typ in ("drag", "nav"):
            t0 = first_true(lambda hs: any(pinch_gap(h) < 0.04 for h in hs), g["t"])
        elif typ in ("window_drag", "pane"):
            t0 = first_true(lambda hs: any(finger_reach(h) < 1.1 for h in hs), g["t"])
        else:
            continue
        if t0 is not None:
            out[typ].append(g["t"] - t0)
    return dict(out)


# ---------------------------------------------------------------- F. wrong hand

def identity_metrics(clip, frames, tracks):
    flips = 0
    for s in tracks.values():
        flips += sum(a["hand"] != b["hand"] for a, b in zip(s, s[1:]))
    same_label = sum(1 for f in frames if len(f["hands"]) >= 2 and
                     f["hands"][0]["handedness"] == f["hands"][1]["handedness"])
    ghosts = sum(1 for s in tracks.values() if len(s) < GHOST_FRAMES)
    # swaps: a track jumps > 0.15 view widths in one frame onto where
    # another track just was
    swaps = 0
    by_t = defaultdict(dict)
    for tid, s in tracks.items():
        for x in s:
            by_t[x["t"]][tid] = x["img"]
    ts = sorted(by_t)
    for t0, t1 in zip(ts, ts[1:]):
        for tid, p1 in by_t[t1].items():
            p0 = by_t[t0].get(tid)
            if p0 is None or np.hypot(*(p1 - p0)) < 0.15:
                continue
            for oid, q0 in by_t[t0].items():
                if oid != tid and np.hypot(*(p1 - q0)) < 0.05:
                    swaps += 1
    extra = 0 if clip in TWO_HAND else sum(1 for f in frames if len(f["hands"]) >= 2)
    return {"handedness_flips": flips, "same_label_frames": same_label,
            "ghost_tracks": ghosts, "id_swaps": swaps, "extra_hand_frames": extra,
            "tracks": len(tracks)}


# ---------------------------------------------------------------- driver

def evaluate(clips):
    expected = json.loads(EXPECTED.read_text())
    per = {}
    for clip in clips:
        trace, frames = run_trace(clip), load_frames(clip)
        tracks = tracks_from_trace(trace, frames)
        g = gesture_metrics(clip, trace, expected[clip])
        per[clip] = {
            "gestures": {k: v for k, v in g.items() if k != "matched_pairs"},
            "loss": loss_metrics(clip, frames, tracks, expected[clip].get("intentional_dropouts", False)),
            "spikes": spike_metrics(frames, tracks),
            "smoothing": smoothing_metrics(tracks),
            "latency_ms": onset_latency(g, tracks, frames),
            "identity": identity_metrics(clip, frames, tracks),
        }
    return per, summarize(per)


SKIP_SUM = {"release_vs_vanish_ms", "bone_dev_p95", "xy_jerk_p99_cm", "z_jerk_p99_cm"}


def summarize(per):
    s = defaultdict(float)
    lat = defaultdict(list)
    for c in per.values():
        for grp in ("gestures", "loss", "spikes", "identity"):
            for k, v in c[grp].items():
                if isinstance(v, (int, float)) and not k.startswith("_") and k not in SKIP_SUM:
                    s[grp + "." + k] += v
        for k, v in c["latency_ms"].items():
            lat[k] += v
    sm = [c["smoothing"] for c in per.values() if c["smoothing"]["_still_n"] > 20]
    mv = [c["smoothing"] for c in per.values()
          if c["smoothing"]["_moving_n"] > 10 and c["smoothing"]["lag_ms_moving"] is not None]

    def w(arr, k, n):
        return float(np.average([a[k] for a in arr], weights=[a[n] for a in arr])) if arr else 0.0

    out = dict(s)
    out["smoothing.jitter_kept_xy"] = w(sm, "jitter_kept_xy", "_still_n")
    out["smoothing.jitter_kept_z"] = w(sm, "jitter_kept_z", "_still_n")
    out["smoothing.lag_ms_moving"] = w(mv, "lag_ms_moving", "_moving_n")
    out["smoothing.err_moving_cm"] = w(mv, "err_moving_cm", "_moving_n")
    out["latency_ms"] = {k: float(np.median(v)) for k, v in lat.items()}
    exp, got, m = s["gestures.expected"], s["gestures.got"], s["gestures.matched"]
    out["event_precision"] = m / got if got else 1.0
    out["event_recall"] = m / exp if exp else 1.0
    p, r = out["event_precision"], out["event_recall"]
    out["event_f1"] = 2 * p * r / (p + r) if p + r else 0.0
    # The one number the autoresearch loop drives down. Gesture errors are
    # what the user feels; tracking terms break ties and reward fixes the
    # clips' events can't yet show.
    out["SCORE"] = (10 * (s["gestures.missed"] + s["gestures.misfires"] + s["gestures.wrong_dir"]
                          + s["gestures.control_err"] + s["gestures.stuck_or_early_release"])
                    + s["loss.dropout_total_ms"] / 100
                    + s["identity.id_swaps"] + s["identity.ghost_tracks"]
                    + out["smoothing.lag_ms_moving"] / 10)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--json")
    ap.add_argument("--split", choices=["all", "train", "holdout"], default="all")
    ap.add_argument("clips", nargs="*")
    a = ap.parse_args()
    clips = a.clips or sorted(p.parent.name for p in CLIPS.glob("*/frames.json"))
    if a.split == "train":
        clips = [c for c in clips if c not in HOLDOUT]
    elif a.split == "holdout":
        clips = [c for c in clips if c in HOLDOUT]
    per, summary = evaluate(clips)
    print(f"{'clip':<26}{'exp':>4}{'got':>4}{'ok':>4}{'miss':>5}{'xtra':>5}{'dir':>4}{'ctl':>4}{'stk':>4}"
          f"{'drop ms':>9}{'brk':>4}{'bone':>5}{'xyS':>4}{'zS':>4}{'pnS':>4}{'flip':>5}{'swap':>5}{'ghst':>5}{'lag':>5}")
    for c, r in per.items():
        g, l, sp, i, sm = r["gestures"], r["loss"], r["spikes"], r["identity"], r["smoothing"]
        print(f"{c:<26}{g['expected']:>4}{g['got']:>4}{g['matched']:>4}{g['missed']:>5}{g['misfires']:>5}{g['wrong_dir']:>4}"
              f"{g['control_err']:>4}{g['stuck_or_early_release']:>4}{l['dropout_total_ms']:>9}{l['track_breaks']:>4}"
              f"{sp['bone_breaks']:>5}{sp['xy_spikes']:>4}{sp['z_spikes']:>4}{sp['pinch_spikes']:>4}"
              f"{i['handedness_flips']:>5}{i['id_swaps']:>5}{i['ghost_tracks']:>5}{(sm['lag_ms_moving'] or 0):>5}")
    print()
    for k in sorted(summary):
        v = summary[k]
        print(f"  {k:<36} {v:.3f}" if isinstance(v, float) else f"  {k:<36} {v}")
    if a.json:
        Path(a.json).write_text(json.dumps({"per_clip": per, "summary": summary}, indent=1, default=str))


if __name__ == "__main__":
    main()
