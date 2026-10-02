# /// script
# requires-python = "==3.12.*"
# dependencies = ["mediapipe==0.10.14", "numpy<2"]
# ///
"""Dev-only: golden hand landmarks from Python MediaPipe, for Seam B.

    uv run tools/mediapipe/make_goldens.py

Fetches MediaPipe's own test images (Apache-2.0) into
crates/terrain-hands/tests/fixtures/hands/ and writes, next to them, the
hands MediaPipe finds in each, in its own conventions (image landmarks
normalized to the unmirrored image, world landmarks in meters):

- goldens.json: the image on its own (palm detection, then landmarks).
- tracked.json: the image as the second frame of a video whose first frame
  was the same image (landmarks in regions tracked from the first frame).

Never shipped or run by the app.

Uses MediaPipe's legacy hand pipeline (`solutions.hands`, still in 0.10.14):
it runs the same full models and the same graph terrain reimplements. The
newer tasks HandLandmarker differs from it by up to ~2.5% of the image on
these fixtures, so it can't serve as a tight reference.
"""

import json
import urllib.request
from pathlib import Path

import mediapipe as mp

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "crates" / "terrain-hands" / "tests" / "fixtures" / "hands"

ASSETS = "https://storage.googleapis.com/mediapipe-assets/tasks/testdata/vision"
# name -> generation, as pinned in mediapipe/third_party/external_files.bzl.
IMAGES: dict[str, int] = {
    "thumb_up.jpg": 1782185354353621,
    "pointing_up.jpg": 1782185079090086,
    "pointing_up_rotated.jpg": 1782185093709880,
    "victory.jpg": 1782185383577587,
    "fist.jpg": 1782184710240231,
    "left_hands.jpg": 1782184842333929,
    "right_hands.jpg": 1782185275057115,
    # No hands.
    "burger.jpg": 1782184424868164,
    "cats_and_dogs.jpg": 1782184491144775,
}


def fetch(name: str, generation: int, dest: Path) -> Path:
    path = dest / name
    if not path.exists():
        url = f"{ASSETS}/{name}?generation={generation}"
        print(f"fetching {name}")
        urllib.request.urlretrieve(url, path)
    return path


def hands_in(result) -> list[dict]:
    found = zip(
        result.multi_handedness or [],
        result.multi_hand_landmarks or [],
        result.multi_hand_world_landmarks or [],
    )
    return [
        {
            "handedness": handedness.classification[0].label,
            "score": handedness.classification[0].score,
            "image": [[p.x, p.y, p.z] for p in image_marks.landmark],
            "world": [[p.x, p.y, p.z] for p in world_marks.landmark],
        }
        for handedness, image_marks, world_marks in found
    ]


def hands(still: bool) -> "mp.solutions.hands.Hands":
    return mp.solutions.hands.Hands(
        static_image_mode=still,
        max_num_hands=2,
        model_complexity=1,
        min_detection_confidence=0.5,
        min_tracking_confidence=0.5,
    )


def main() -> None:
    FIXTURES.mkdir(parents=True, exist_ok=True)
    goldens, tracked = {}, {}
    for name, generation in IMAGES.items():
        image = mp.Image.create_from_file(str(fetch(name, generation, FIXTURES)))
        rgb = image.numpy_view()[:, :, :3]
        with hands(still=True) as still:
            goldens[name] = hands_in(still.process(rgb))
        with hands(still=False) as video:
            video.process(rgb)
            tracked[name] = hands_in(video.process(rgb))
        print(f"{name}: {len(goldens[name])} hand(s), {len(tracked[name])} tracked")
    for file, data in [("goldens.json", goldens), ("tracked.json", tracked)]:
        (FIXTURES / file).write_text(json.dumps(data, indent=1) + "\n")


if __name__ == "__main__":
    main()
