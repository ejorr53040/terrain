# Hand fixture images

Test images from Google MediaPipe's test data
(`https://storage.googleapis.com/mediapipe-assets/tasks/testdata/vision/`),
licensed Apache-2.0 (Google LLC, MediaPipe). `burger.jpg` and
`cats_and_dogs.jpg` contain no hands.

`goldens.json` holds, for each image, the hands Python MediaPipe's
HandLandmarker finds: handedness, score, 21 image landmarks (normalized to
the unmirrored image, x right, y down) and 21 world landmarks (meters).

**Regenerate:** `uv run tools/mediapipe/make_goldens.py` (dev-only).
