#!/usr/bin/env bash
# One autoresearch experiment: build, score, log a row to results.tsv.
#   tools/autoresearch/run.sh "<hypothesis>" [--split train|holdout|all]
#   tools/autoresearch/run.sh --split holdout
set -euo pipefail
cd "$(dirname "$0")/../.."
desc="${1:-unnamed}"; shift || true
split="train"
[[ "${1:-}" == "--split" ]] && split="$2"
[[ "$desc" == "--split" ]] && { split="$1"; desc="holdout check"; }

# Sandboxes without cdn.pyke.io: a system ONNX Runtime (e.g. from the
# onnxruntime wheel) in /opt/ort/lib. On a normal PC ort downloads its own.
if [[ -d /opt/ort/lib && -z "${ORT_LIB_LOCATION:-}" ]]; then
  export ORT_LIB_LOCATION=/opt/ort/lib ORT_PREFER_DYNAMIC_LINK=1
  export LD_LIBRARY_PATH=/opt/ort/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
fi

cargo build --release -q -p terrain-desktop --example dump_trace
out=$(mktemp)
uv run -q tools/autoresearch/metrics.py --split "$split" --json "$out" >/dev/null
row=$(python3 - "$out" <<'EOF'
import json, sys
s = json.load(open(sys.argv[1]))["summary"]
g = lambda k: s.get(k, 0)
print("\t".join(f"{v:.3f}" if isinstance(v, float) else str(v) for v in [
    g("SCORE"), g("event_f1"), g("gestures.missed"), g("gestures.misfires"),
    g("gestures.wrong_dir"), g("gestures.stuck_or_early_release"),
    g("loss.dropout_total_ms"), g("smoothing.lag_ms_moving"),
    g("smoothing.jitter_kept_xy")]))
EOF
)
log=tools/autoresearch/results.tsv
[[ -f $log ]] || printf "commit\tsplit\tscore\tf1\tmissed\tmisfires\twrong_dir\tstuck\tdropout_ms\tlag_ms\tjitter_kept\tdescription\n" > "$log"
printf "%s\t%s\t%s\t%s\n" "$(git rev-parse --short HEAD)$(git diff --quiet || echo +)" "$split" "$row" "$desc" >> "$log"
tail -1 "$log"
rm -f "$out"
