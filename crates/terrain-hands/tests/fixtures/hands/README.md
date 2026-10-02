# Hand fixture images

Test images from Google MediaPipe's test data
(`https://storage.googleapis.com/mediapipe-assets/tasks/testdata/vision/`),
licensed Apache-2.0 (Google LLC, MediaPipe). `burger.jpg` and
`cats_and_dogs.jpg` contain no hands.

`goldens.json` holds, for each image, the hands Python MediaPipe finds:
handedness, score, 21 image landmarks (normalized to the unmirrored image,
x right, y down) and 21 world landmarks (meters). They come from MediaPipe's
legacy hand pipeline (`solutions.hands`, MediaPipe 0.10.14), which runs the
same full models through the same graph terrain reimplements. The newer
tasks HandLandmarker disagrees with it by up to ~2.5% of the image on these
images, too much for a tight reference.

**Regenerate:** `uv run tools/mediapipe/make_goldens.py` (dev-only).
