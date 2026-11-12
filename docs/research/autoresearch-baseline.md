# Autoresearch: accuracy metrics and baseline

Question: how accurate is terrain end to end, where does it go wrong, and how can an
agent loop (in the style of karpathy/autoresearch) improve it by trial and error?

Measured 2026-11-11 on `main` at f72c2ef, over all 22 clips in
`crates/terrain-desktop/tests/clips`. Tooling is in `tools/autoresearch/`
(see its `program.md` for the loop rules).

**Verdict:** about 85% of gestures do what was meant (event F1 0.85: 42 of 49
expected actions, 7 missed, 7 misfires). The errors cluster in fist flicks,
one pinch flick and Vee down. Tracking loses 1.8 s of two-hand time,
mostly mid-motion, which costs `spread_short` one Spread.

## How it's measured

- `crates/terrain-desktop/examples/dump_trace.rs` replays a clip through the
  same estimator and gesture engine as `DesktopControl` and prints per frame:
  each hand's raw and smoothed palm, pose, handedness, who has control, and
  the actions.
- `tools/autoresearch/expected.json`: answer keys, written from each clip's
  `intent.md`, **not** from `golden.log` (goldens record what the code does
  today, including the known-wrong PENDING ones).
- `tools/autoresearch/metrics.py` (a `uv` script; it brings its own numpy)
  scores the traces. One pass over every clip takes about 2 s.
- `tools/autoresearch/run.sh "<hypothesis>"` builds, scores the train split
  and appends a row to `tools/autoresearch/results.tsv`.
- Train/holdout: 5 clips are held out (`drag_after_hold`, `fist_push`,
  `full_screen_motion`, `vee_sideways`, `leave_mid_drag_after_hold`), checked
  only when accepting changes, so tuning to 17 clips can't silently overfit.

## Parameters tracked, with baseline

| Group | Metric | Baseline (all clips) |
|---|---|---|
| Button misfires | matched / expected | 42 / 49 (F1 0.85) |
| | missed, misfires, wrong direction | 7, 7, 0 |
| | stuck or early release (hand leaving mid-drag) | 1 (`fast_drag_blur`, released 133 ms before the hand left) |
| | control errors (took control when it shouldn't, or the reverse) | 0 |
| Model output spikes | bone-length breaks (>30% off the clip median) | 593 of 5829 hand frames: fingertip bones and palm width |
| | palm 2nd-difference spikes, across view (>3 cm) / depth (>6 cm) | 54 / 106 |
| | thumb–index gap spikes (>2.5 cm): false-click risk | 156 |
| | frames with score < 0.8 | 58 |
| Smoothing (One Euro) | jitter kept at rest, xy / depth (smoothed ÷ raw) | 0.27 / 0.12 |
| | lag while moving (best time shift) | 18 ms |
| | distance between smoothed and raw palm while moving | 0.7 cm |
| Delay behind hands | Spread: palms start parting → Super+F | 67 ms |
| | click / right click: pinch opens → fires | 200 / 167 ms |
| | pinch flick / fist flick: pose → chord | 333 / 233 ms |
| | drag / window drag: pose → press (mostly the deliberate wait for motion) | 500 / 467 ms |
| Wrong hand | handedness flips within a track | 25 (mostly rapid flicker in `fist_flicks` at 5.4–5.9 s) |
| | frames with two hands labelled the same | 3 |
| | ID swaps / ghost tracks (<5 frames) | 0 / 2 |
| | extra-hand frames in one-hand clips | 4 |
| Hand loss | two-hand dropout time (excl. `not_full_screen`, which drops hands on purpose) | 1832 ms; worst `spread_short` 1199 ms |
| | losses while the palm moved >0.3 m/s | 21 |
| | track breaks (lost, then re-found as a new hand within 1 s) | 4 |

Per-clip misses: `fist_flicks` (1 of 4 pane moves; the rest become window
drags), `pinch_flicks` (Alt+Right read as a drag), `vee_down` (no scroll),
`spread_in_control` (4 of 5), `spread_short` (6 of 7).

## Score the loop drives down

`SCORE = 10·(missed + misfires + wrong_dir + control_err + stuck)
+ dropout_ms/100 + id_swaps + ghost_tracks + lag_ms/10`

Baseline: train 168.8, holdout 5.1, all 172.1. Guardrails for keeping a
change: lag up ≤ 5 ms, jitter kept up ≤ 0.05, `terrain-hands` and
`terrain-gestures` tests pass.

Smoke test of the loop: `PINCH_OPEN_M` 6.5 → 7.5 cm fixed the early release
in `fast_drag_blur` but caused 2 misses (train 178.8): rejected and reverted.

## Stage 2 baseline: today's tracker on the raw takes

`tools/autoresearch/retrack.sh` re-tracks all 22 raw takes (about 2 min) and
`metrics.py --frames` scores them. 17 clips come out byte-identical to the
repo's `frames.json`; 5 were saved by an older tracker and today's does worse
on them:

| | Saved frames (stage 1) | Re-tracked (stage 2) |
|---|---|---|
| SCORE train / holdout / all | 168.8 / 5.1 / 172.1 | 213.8 / 3.5 / 215.4 |
| event F1 (all) | 0.85 | 0.83 |
| missed / misfires (all) | 7 / 7 | 9 / 6 |
| dropout (all) | 1832 ms | 5265 ms |
| handedness flips (all) | 25 | 24 |

Worst: `fist_flicks` 0 of 4 pane moves with 2.1 s of dropout,
`spread_short` 2.7 s of dropout and 2 missed Spreads. So the tracker
regressed on fast fists and Spreads since those clips were saved; stage-2
work should start there (`SEARCH_EVERY`, `REFIND_FRAMES`, region sizing).

## Limits

- Clips hold the tracker's output, not camera frames, so stage 1 can only
  change what happens after landmarks (thresholds, smoothing, coasting,
  gesture rules). Stage 2 needs the raw takes `tools/record-gesture-clips.sh`
  caches locally, so it only runs on the PC that recorded them.
- 22 clips, one user: thresholds are coarse. Bone-break and spike thresholds
  are heuristics: use them to compare runs, not as absolute error rates.
- Latency here is decision latency from the recorded frames; model compute
  time per frame must be measured on the target PC.

## Next

1. Stage 2: find which tracker change since 2026-10-22 lost the hands in
   `fist_flicks` and `spread_short`, then try `SEARCH_EVERY`,
   `REFIND_FRAMES`, region sizing, label voting and alternative models.
2. Stage 1 ideas, in order: coasting through ≤150 ms gaps in the gesture
   engine (ADR 0008 / spread-accuracy.md step 4), pinch gap normalised by hand
   size, flick-vs-drag speed rule for fists (#28), Vee down threshold.
