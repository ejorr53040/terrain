# /// script
# requires-python = "==3.12.*"
# dependencies = ["mediapipe==0.10.*"]
# ///
"""Dev-only: golden hand landmarks from Python MediaPipe, for Seam B.

    uv run tools/mediapipe/make_goldens.py

Fetches MediaPipe's own test images (Apache-2.0) into
crates/terrain-hands/tests/fixtures/hands/ and writes goldens.json next to
them: for each image, the hands MediaPipe's HandLandmarker finds, in its own
conventions (image landmarks normalized to the unmirrored image, world
landmarks in meters). Never shipped or run by the app.
"""

import json
import urllib.request
from pathlib import Path

import mediapipe as mp
from mediapipe.tasks.python import BaseOptions
from mediapipe.tasks.python.vision import HandLandmarker, HandLandmarkerOptions, RunningMode

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "crates" / "terrain-hands" / "tests" / "fixtures" / "hands"
CACHE = ROOT / "target" / "mediapipe"

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
TASK = "hand_landmarker.task", 1782184791153758


def fetch(name: str, generation: int, dest: Path) -> Path:
    path = dest / name
    if not path.exists():
        url = f"{ASSETS}/{name}?generation={generation}"
        print(f"fetching {name}")
        urllib.request.urlretrieve(url, path)
    return path


def main() -> None:
    FIXTURES.mkdir(parents=True, exist_ok=True)
    CACHE.mkdir(parents=True, exist_ok=True)
    options = HandLandmarkerOptions(
        base_options=BaseOptions(model_asset_path=str(fetch(*TASK, CACHE))),
        running_mode=RunningMode.IMAGE,
        num_hands=2,
    )
    goldens = {}
    with HandLandmarker.create_from_options(options) as landmarker:
        for name, generation in IMAGES.items():
            image = mp.Image.create_from_file(str(fetch(name, generation, FIXTURES)))
            result = landmarker.detect(image)
            goldens[name] = [
                {
                    "handedness": handedness[0].category_name,
                    "score": handedness[0].score,
                    "image": [[p.x, p.y, p.z] for p in image_marks],
                    "world": [[p.x, p.y, p.z] for p in world_marks],
                }
                for handedness, image_marks, world_marks in zip(
                    result.handedness, result.hand_landmarks, result.hand_world_landmarks
                )
            ]
            print(f"{name}: {len(goldens[name])} hand(s)")
    (FIXTURES / "goldens.json").write_text(json.dumps(goldens, indent=1) + "\n")


if __name__ == "__main__":
    main()
