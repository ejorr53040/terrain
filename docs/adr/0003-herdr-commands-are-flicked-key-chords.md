# herdr commands are key chords sent by Flicks of the one hand with control

herdr (the terminal workspace manager) is driven by Flicks: a fist flicked left, right, up or down moves the pane focus (Ctrl+Alt+h/l/k/j), a pinch flicked left or right steps tabs (Alt+Left/Right) and up or down steps workspaces (Alt+Up/Down). They send herdr's own direct key chords rather than calling its socket API, because ADR 0002 forbids actions that run commands, and the chords keep herdr's keymap the single source of truth. A Flick is told from a Drag by speed (`Flicked(direction, 0.1 screen widths, 200 ms)` checked before `Moved`, with drags waiting out the 200 ms), not by a mode or a second hand: a mode is easy to forget, and a second command hand would mean redesigning Engagement's one hand with control. Depth (`Pushed`) is not used, since the fist push it was built for doesn't work in practice.

## Considered options

- herdr's socket API (`herdr pane focus ...`): exact and focus-independent, but it is running a command (ADR 0002).
- herdr's prefix sequences (`ctrl+space`, then a key): prefix timing to get wrong, where direct chords exist.
- A second hand as the command hand, or a toggled herdr mode.
- Finger-count Poses: deferred. They need per-finger state in `terrain-hands` and are the planned way to reach split, zoom, new tab, next agent and the workspace picker.

## Consequences

- Chords go to the focused window: a Flick over a browser sends Alt+Left there, which is Back.
- Pinch and fist Drags start up to 200 ms later than before.
- Commands that close a pane, tab or workspace are never bound to a gesture: a misread pose must not lose work.
