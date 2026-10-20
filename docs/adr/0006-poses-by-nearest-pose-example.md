# Poses are recognised by nearest pose example, not thresholds

A hand's Pose is the label of the pose examples its landmarks are closest to, not a set of hand-measured cut-offs on finger distances. The examples are frames from clips recorded holding one pose each, stored in the repo as landmark numbers. Thresholds needed re-measuring for every new pose and every pair of poses that overlapped (a closed fist read as a Pinch because the thumb touches the index finger), while finger counts (#29, #44) would have multiplied them. Adding a pose is now recording a clip, and every decision can be traced to the example that won.

## Considered options

- Keep thresholds: worked for Pinch alone, but Fist, Vee and four finger counts would mean a dozen interacting constants tuned by hand, and Pinch and Fist already disagreed.
- Train a small network and export it to ONNX: more compact and may generalise better, but adds a training pipeline and gives answers nobody can trace to a recorded frame.

## Consequences

- Pinch no longer has distance thresholds (4 cm / 6.5 cm). Only the pinch point is still measured.
- A pose changes only after 2 frames agree. A frame unlike every example keeps the previous pose, so a blurred frame doesn't end a drag. A newly seen hand starts Open.
- Examples are compared in the palm's own frame, with left hands mirrored, so one set serves both hands in any direction.
- Revisit if accuracy on a held-out take falls under 95% for any pose, or the example set grows past about 50k frames.
