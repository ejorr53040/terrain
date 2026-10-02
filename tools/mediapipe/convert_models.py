# /// script
# requires-python = "==3.12.*"
# dependencies = ["tensorflow-cpu==2.17.*", "tf2onnx==1.16.*", "onnx<1.17"]
# ///
"""Dev-only: fetch MediaPipe's full hand models and convert them to ONNX.

    uv run tools/mediapipe/convert_models.py

Writes models/<name>.onnx and records where each came from in
models/SOURCES.json. Never shipped or run by the app.
"""

import hashlib
import json
import subprocess
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODELS = ROOT / "models"
CACHE = ROOT / "target" / "mediapipe"
OPSET = 17

# Google's MediaPipe model assets (Apache-2.0), as pinned in
# mediapipe/third_party/external_files.bzl.
SOURCES = {
    "palm_detection_full": "https://storage.googleapis.com/mediapipe-assets/modules/palm_detection/palm_detection_full.tflite?generation=1782183635662937",
    "hand_landmark_full": "https://storage.googleapis.com/mediapipe-assets/modules/hand_landmark/hand_landmark_full.tflite?generation=1782183530499255",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    MODELS.mkdir(exist_ok=True)
    CACHE.mkdir(parents=True, exist_ok=True)
    record = {}
    for name, url in SOURCES.items():
        tflite = CACHE / f"{name}.tflite"
        if not tflite.exists():
            print(f"fetching {name}")
            urllib.request.urlretrieve(url, tflite)
        onnx = MODELS / f"{name}.onnx"
        subprocess.run(
            [sys.executable, "-m", "tf2onnx.convert", "--tflite", str(tflite),
             "--output", str(onnx), "--opset", str(OPSET)],
            check=True,
        )
        record[name] = {
            "source": url,
            "license": "Apache-2.0 (Google MediaPipe)",
            "tflite_sha256": sha256(tflite),
            "onnx_sha256": sha256(onnx),
            "converted_with": f"tf2onnx 1.16, opset {OPSET}",
        }
    (MODELS / "SOURCES.json").write_text(json.dumps(record, indent=2) + "\n")


if __name__ == "__main__":
    main()
