# Gestures are statically checked data, not code

Gestures are defined in a gesture file as small state machines (one per starting pose) whose transitions may only test pose, time held, distance moved, hand speed and leaving the reach. Every state that holds a button or key must name its release. We chose this over recognizers written in Rust, or guards with arbitrary expressions, because the worst failures are stuck buttons and unintended clicks, and only a closed language lets a checker prove, before the file is loaded, that each frame takes exactly one transition and that every press can reach a release. The language is deliberately limited: extend it by adding a primitive the checker understands, never an escape hatch.

## Consequences

A gesture file that fails its checks is never used. The running set stays on the last file that passed, and the built-in default is compiled in.
