# terrain autoresearch

An agent loop in the style of karpathy/autoresearch: change one thing, score it
on recorded clips, keep it only if the score improves, repeat. Here the
"training run" is a replay of the gesture clips, so one experiment takes
about 15 s to build and 2 s to score.

## Fixed: never edit while looping

- `tools/autoresearch/metrics.py`: the scorer
- `tools/autoresearch/expected.json`: answer keys (what the user meant)
- `crates/terrain-desktop/tests/clips/*/frames.json`: the data
- `crates/terrain-desktop/examples/dump_trace.rs`

If the scorer looks wrong, stop and tell the human. Don't fix it mid-run.

## Mutable: what experiments may change

Stage 1 (landmarks → actions; works on the clips in the repo):
- `crates/terrain-hands/src/state.rs`: pinch, fist and Vee thresholds and
  hysteresis, `OPEN_FRAMES`, One Euro smoothing parameters,
  `SAME_HAND_MAX_MOVE`, `FORGET_AFTER_MS`, track association
- `crates/terrain-hands/src/smoothing.rs`: the filter itself (One Euro,
  Kalman, constant-velocity coasting through short gaps)
- `crates/terrain-gestures/src/default.ron` and `lib.rs`: gesture timings,
  Spread distances, flick speed and time windows, engage and disengage rules
- New pose features: e.g. normalising pinch gap by hand size, or using
  image landmarks where world z is unreliable

Stage 2 (pixels → landmarks; needs raw takes, see below):
- `crates/terrain-hands/src/hand_tracker.rs`: `SEARCH_EVERY`,
  `REFIND_FRAMES`, `SAME_HAND_IOU`, `REGION_SCALE`/`SHIFT`, `LABEL_WEIGHT`
- `palm.rs` / `landmarks.rs`: score thresholds, NMS, input preprocessing
  (exposure/gamma normalisation, deblur)
- `models/`: alternative landmark models (lite vs full, other ONNX hand models)

## Loop

1. `git checkout -b autoresearch/<tag>`. Run `tools/autoresearch/run.sh baseline`
   once, and record its SCORE.
2. Pick ONE idea. Write a one-line hypothesis.
3. Edit, then run `tools/autoresearch/run.sh "<hypothesis>"`. It builds,
   scores the train split and appends a row to `results.tsv`.
4. Keep the change (commit it) if SCORE dropped and the guardrails hold:
   - `smoothing.lag_ms_moving` up by no more than 5 ms
   - `smoothing.jitter_kept_xy` up by no more than 0.05
   - `cargo test -p terrain-hands -p terrain-gestures` still passes
     (`terrain-desktop`'s golden tests are expected to change; see below)
   Otherwise `git checkout -- .` to revert.
5. After every 5 kept changes, run `run.sh --split holdout`. If holdout
   SCORE got worse than at the last check, revert to that check: the train
   gains are overfitting.
6. Repeat. Don't stop to ask; log reasoning in `results.tsv`.

When done: regenerate the goldens of the clips whose behaviour improved
on purpose, update intent.md if needed, and write up what changed and why.

## Score

`SCORE = 10·(missed + misfires + wrong_dir + control_err + stuck/early release)
       + dropout_ms/100 + id_swaps + ghost_tracks + lag_ms/10`, lower is better.
Gesture errors dominate: they're what the user feels. The rest breaks
ties and rewards tracking fixes that the clips' events can't show yet.

## Stage 2: tracker and model changes

`STAGE=2 tools/autoresearch/run.sh "<hypothesis>"` first re-tracks every
clip's raw webcam take (`retrack.sh`: JPEGs cached by
`record-gesture-clips.sh` in `~/.cache/terrain/clips`, not in git) through
today's `HandTracker` with `--every-frame`, then scores those frames instead
of the repo's `frames.json`. One run takes about 2 minutes. Use it for any
change to stage-2 files; compare only against other `[s2]` rows.

Stage-2 baseline at 75589c4: train 213.8, holdout 3.5 (stage 1 reads 168.8
on the saved frames). 17 clips re-track byte-identical; `fist_flicks`,
`full_screen_motion`, `slow_drags`, `spread_in_control` and `spread_short`
were saved by an older tracker, and today's loses more hands on them.
Re-tracked frames never replace the repo's `frames.json` during the loop.
