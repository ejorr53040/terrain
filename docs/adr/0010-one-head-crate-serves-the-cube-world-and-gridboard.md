# One shared head crate serves the cube world and gridboard

Head look lives in one crate, `terrain-head`, used by the cube world (orbiting the camera) and by gridboard (peeking at the board, as a git dependency). It owns the daemon connection and the orientation math, and hands out one value, `HeadLook`: Off with a reason, Starting, or On with yaw and pitch in degrees. Each consumer decides what the angles do (gains, clamps, gliding back when Off). The crate depends only on glam, serde and serde_json; no Bevy, egui or GTK, and no glam types in its interface.

## Considered options

- Head look inside terrain-app, copied into gridboard: the junk first sample, Straight ahead, Recenter and the one body-axis assumption would drift apart.
- A Bevy plugin: gridboard is not a Bevy app.

## Consequences

- The Max's ear axis (`EAR_AXIS`) is one constant in the crate, to be verified on hardware once for both consumers.
- Consumers poll `look()` once a frame; it never blocks, the connection runs on a background thread.
