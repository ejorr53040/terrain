# Models

MediaPipe's **full** hand models, converted from TFLite to ONNX for `ort`:

| File | What | Input |
| --- | --- | --- |
| `palm_detection_full.onnx` | Palm detector (SSD, 2016 anchors) | 1×192×192×3 RGB, 0..1, NHWC |
| `hand_landmark_full.onnx` | Hand landmarks (21 points, image + world) | 1×224×224×3 RGB, 0..1, NHWC |

**Source:** Google MediaPipe model assets, the same files MediaPipe pins in
`mediapipe/third_party/external_files.bzl`. Exact URLs and SHA-256 of both
the original `.tflite` and the converted `.onnx` are in `SOURCES.json`.

**License:** Apache-2.0 (Google LLC, MediaPipe). See `LICENSE-APACHE` at the
repo root. The conversion only changes the file format, not the weights.

**Regenerate:** `uv run tools/mediapipe/convert_models.py` (dev-only; needs
network access and pulls TensorFlow into uv's cache).
