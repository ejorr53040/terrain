# terrain

Move 3D objects on screen with your real hands. Rust + Bevy + MediaPipe hand models (ONNX).

## Agent skills

### Issue tracker

Issues and specs live in GitHub Issues on ejorr53040/terrain (via `gh`). See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: needs-triage, needs-info, ready-for-agent, ready-for-human, wontfix. See `docs/agents/triage-labels.md`.

### Accuracy and autoresearch

Accuracy is measured on the recorded clips by `tools/autoresearch/metrics.py`
(gesture misses and misfires against `tools/autoresearch/expected.json`, hand
loss, model output spikes, smoothing jitter and lag, delay, wrong-hand
detection). Baseline and parameter definitions:
`docs/research/autoresearch-baseline.md`.

- Build the trace dumper first: `cargo build --release -p terrain-desktop --example dump_trace`
- Score: `uv run tools/autoresearch/metrics.py [--split train|holdout] [--json out.json]`
- One experiment: `tools/autoresearch/run.sh "<hypothesis>"` (logs to `tools/autoresearch/results.tsv`)
- Loop rules (what's fixed, what may change, keep/revert, holdout checks):
  `tools/autoresearch/program.md`
- `expected.json` holds what the user *meant* (from each clip's `intent.md`);
  `golden.log` holds what the code does today. Don't edit `expected.json`,
  `metrics.py` or clip data while running the loop.
- When a change improves a clip on purpose, regenerate that clip's
  `golden.log` and say so in the PR.

### Domain docs

Single-context: one `CONTEXT.md` + `docs/adr/` at the repo root, created lazily. See `docs/agents/domain.md`.
