#!/usr/bin/env bash
# Stage 2: re-track every clip's raw webcam take through today's HandTracker,
# so tracker and model changes reach the scorer.
#   tools/autoresearch/retrack.sh [OUT_DIR]   (default target/autoresearch/frames)
# Then: uv run tools/autoresearch/metrics.py --frames OUT_DIR
# Raw takes are the JPEGs tools/record-gesture-clips.sh caches (not in git).
set -euo pipefail
cd "$(dirname "$0")/../.."
RAW=${TERRAIN_RAW_CLIPS:-$HOME/.cache/terrain/clips}
OUT=${1:-target/autoresearch/frames}
mkdir -p "$OUT"
cargo build --release -q -p terrain-hands --example replay_clip

# Clips whose take is cached under another name.
raw_for() {
  case $1 in
    approach_pinch) echo "$RAW/sweep2/auto_fixedfps" ;;
    spread_short)   echo "$RAW/gestures/spread_misses" ;;
    *)              echo "$RAW/gestures/$1" ;;
  esac
}

jobs=()
for dir in crates/terrain-desktop/tests/clips/*/; do
  clip=$(basename "$dir")
  src=$(raw_for "$clip")
  [[ -d $src ]] || { echo "missing raw take for $clip: $src" >&2; exit 1; }
  jobs+=("$src" "$OUT/$clip.json")
done
# Each replay uses about 2 cores; --every-frame keeps it deterministic.
printf '%s\0%s\0' "${jobs[@]}" |
  xargs -0 -n2 -P "$(( $(nproc) / 2 ))" sh -c \
    'target/release/examples/replay_clip "$0" --every-frame --save-frames "$1" >/dev/null'
echo "re-tracked $(( ${#jobs[@]} / 2 )) clips into $OUT"
