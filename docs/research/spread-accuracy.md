# Spread (full screen) accuracy: what the clips show, and what to try

Question: why do live Spreads still miss, what hand data terrain has, and what do people with similar webcam + MediaPipe setups do to make gestures more accurate?

Read 2026-10-22, on branch `slice-20-spread-reach` (the 26 cm `apart` / `parting_ms` fix, uncommitted). Earlier gesture research is in [hand-desktop-control-on-github.md](hand-desktop-control-on-github.md); depth is in [hand-depth.md](hand-depth.md).

**Verdict: the misses are mostly tracking, not the detector.** One hand drops out for 100–630 ms in the middle of the fling, not when the palms meet. The fixes most likely to help are in capture (short exposure, 60 fps) and the tracker (look for a lost hand the next frame, coast through short gaps). Tuning the Spread's thresholds further is fitting on 3–4 frames of 30 fps data.

## What the Spread does now

`crates/terrain-gestures/src/default.ron` (spreads): both palms in Open or Pinch, within 0.20 m for 150 ms (armed), then more than 0.26 m apart within 450 ms, with at most 200 ms of the parting spent with both hands seen. Fires Super+F once, stays armed until 450 ms after the palms were last together. Dropouts and one-frame stray Poses keep it armed but stop the parting clock (ADR 0008).

## Data per frame

- Tracker (`terrain-hands`, `Hand` in `frame.rs`): handedness, score, 21 landmarks in normalized image coordinates and 21 in metric world coordinates (hand-centred).
- Clips (`crates/terrain-desktop/tests/clips/*/frames.json`): exactly that, every ~33 ms (30 fps). Enough to re-derive anything offline.
- Gesture engine (`terrain-gestures`, `Hand` in `lib.rs`): only `id`, `pose`, `in_view` (palm, normalized) and `position` (palm, meters, from the depth estimate). No velocity, no confidence, no "missing since" per hand.

## What the clips show

| Clip | Frames | 2 hands | 1 hand | Dropout runs (2 → fewer) |
|---|---|---|---|---|
| spread_short | 600 | 374 | 41 | 630, 230, 330 ms |
| full_screen_motion | 300 | 217 | 11 | 330 ms |
| spread_in_control | 450 | 290 | 44 | 100, 70, 130 ms |

Where each loss started (last frame with both hands, image coordinates):

- Most: palms already about 0.27–0.32 of the view apart, low in frame (y ≈ 0.6–0.7), well inside the edges. That is mid-fling, fast motion.
- A few: at the bottom edge (y 0.9–1.0) or the side (x 0.08).
- None while the palms were together, so the tracker's converged-region rule (`hand_tracker.rs`, `SAME_HAND_IOU`) is not the cause.
- No frame had two hands with the same handedness label, so identity swaps aren't showing up.

Inferred, not verified: motion blur during the fling loses the landmarks, then the palm detector, which runs only every 3rd frame while one hand is followed (`SEARCH_EVERY = 3`), takes up to 100 ms more to find it again. Also: a 100–133 ms Spread is 3–4 frames at 30 fps, so timing thresholds sit on very few samples.

## Baseline (#77, 2026-10-22)

From `tools/clip_report.py`, before any tracking fix. Later tickets compare against this. Clips without two hands show no dropouts and are left out.

| Clip | 2 hands | 1 hand | none | Dropouts (ms) |
|---|---|---|---|---|
| spread_short | 374 | 41 | 185 | 633, 233, 333 |
| full_screen_motion | 217 | 11 | 72 | 333 |
| spread_in_control | 290 | 44 | 116 | 100, 66, 134 |
| not_full_screen | 13 | 20 | 267 | 3566 (hands leave and come back by design, not a dropout) |

## What to try, best first

1. **Capture: short fixed exposure, 60 fps.** `v4l2-ctl` with manual exposure (`auto_exposure=1`, `exposure_time_absolute` ~50–100) and the camera's 60 fps MJPEG mode if it has one. Tutorials and vendor docs name motion blur and fast movement as the main cause of landmark loss, and auto-exposure lengthens exposure in dim light [labellerr; warudo]. No controlled study found: measure by re-recording spread_short and counting 1-hand frames.
2. **Re-search immediately for a lost hand.** When a hand followed last frame isn't found, run the palm detector on the next frame instead of waiting for `SEARCH_EVERY`. MediaPipe's own graph re-runs detection when tracking fails [mp-gr]. One-line change at `hand_tracker.rs:72`.
3. **Coast through short gaps.** Constant-velocity (or small Kalman) prediction of a lost hand's palm for ≤150 ms, so the engine sees "apart" rather than "missing". This is SORT's predict + missed-frame tolerance [sort; surgeon]. Hands move erratically, so keep the coast short [cvonline].
4. **Gate on separation speed.** Fire on the palm gap growing faster than a rate, not only on distance plus a parting clock. Velocity gating is the usual dynamic-gesture front end [patent]. Could replace `parting_ms`.
5. **Per-user calibration.** Record 3 Spreads, set `together`/`apart` from them. Per-user fine-tuning took dynamic gestures from 88.1% to 97.5% in one thesis [chalmers].
6. **Learned sequence classifier, only if rules plateau.** GRU / 1D-CNN on landmark sequences with a "no gesture" class [sensors22]. Published dynamic accuracy is ~85–88% [grlib; chalmers], below tuned rules on one gesture. Worth it only with many gestures and many clips, which the clip harness already produces.

Unchanged from the earlier research: no better permissive landmark model has appeared; stay on MediaPipe full.

## Sources

- [derivative] https://forum.derivative.ca/t/making-sure-that-hand-1-is-always-left-hand-with-mediapipe/990357 — hands swap after a dropout
- [mp-gr] https://ai.google.dev/edge/mediapipe/solutions/vision/gesture_recognizer/web_js — detection re-runs only when tracking fails
- [patent] https://istina.fnkcrr.ru/patents/11699009 — velocity threshold gates the dynamic recognizer
- [labellerr] https://labellerr.com/blog/mediapipe-hand-tracking-egocentric-video/ — blur and fast motion lose landmarks
- [warudo] https://docs.warudo.app/docs/mocap/mediapipe — check real fps first
- [sort] https://arxiv.org/pdf/1703.07402 — Kalman prediction + Hungarian association
- [surgeon] https://arxiv.org/pdf/2012.06948 — tolerance to missed detections
- [cvonline] https://homepages.inf.ed.ac.uk/rbf/CVonline/LOCAL_COPIES/GONG3/handTracking.html — hands defeat simple dynamics models
- [chalmers] https://odr.chalmers.se/items/848830bf-774b-4859-b362-4f0401c9ade2 — 88.1% → 97.5% with per-user tuning
- [sensors22] https://mdpi.com/1424-8220/23/1/2 — MediaPipe landmarks + GRU / 1D-CNN
- [grlib] https://ar5iv.labs.arxiv.org/html/2310.14919 — 99% static vs 85% dynamic

## Next steps (grilled 2026-10-22)

Order: land `slice-20-spread-reach` as its own PR → accuracy slice → #73 → #74 → #75 → #76.

**Accuracy slice, staged:**
1. Exposure sweep with `v4l2-ctl`: `exposure_dynamic_framerate=0`, manual `exposure_time_absolute` 50 / 100 / 156 (5 / 10 / 15.6 ms), a short Spread clip at each; keep the value with fewest dropouts. The webcam can't do 60 fps (30 fps max in every mode).
2. Move it into terrain: an opt-in setting; terrain reads the camera's values when it opens it and restores them on exit (a SIGKILL still leaves them set; say so in the setting's doc).
3. Re-find a lost hand: when a tracked hand vanishes, run the palm detector every frame for `FORGET_AFTER_MS` (300 ms), then back to `SEARCH_EVERY`. Check frame time stays under 33 ms live.
4. Measure: re-record spread_short. Only if a dropout is still over 150 ms, add coasting **in the gesture engine** (last position + velocity per hand, ≤150 ms, read by two-hand gestures only), never in the tracker: the pointer and pinch never see a made-up hand.

**Measuring:** `tools/clip_report.py` (stdlib; frames with 2 / 1 / 0 hands, dropout runs per clip), plus a test on spread_short's dropout limit once the slice lands. Before a PR, a live count: 10 tries of each gesture touched.

**Clips:** keep every existing clip as a regression test (blurry input is a free stress test); new clips are recorded at the new exposure. Each new gesture (#73–#76) ships with a near-miss clip of its closest confuser. Two shared desk-motion clips, ~15 s each, nothing fires: keyboard area (typing, mouse reach, both hands lifted and dropped to the keys) and away from it (cup/phone, arms stretched outward, hands rubbed or resting together, face scratch).

**Not now:** speed gate in place of `parting_ms` (only if slow drifts fire again after the tracking fixes); per-user calibration (one user, already tuned on the existing clips); learned classifier (revisit past ~12 gestures or if rules plateau; clips with intent.md + golden logs are already labelled data).
