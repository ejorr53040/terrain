# terrain

Hands seen by a webcam drive things on screen: 3D objects in the app, and the real desktop's pointer, buttons and windows.

## Language

### Hands

**Pose**:
The shape a hand is in during one frame: Open, Pinch, Fist or Vee.
_Avoid_: Grip, hand shape

**Vee**:
The pose with the index and middle fingers extended and the others curled, used to scroll.
_Avoid_: Two-finger, peace sign

**Gesture**:
A recognized pattern of poses and movement over time, such as a Tap, Drag or LongPress.
_Avoid_: Command, motion

**Flick**:
A pinch or fist moved quickly in one direction (left, right, up, down) just after it closes, as opposed to the slower move of a Drag. Flicks send herdr's key chords.
_Avoid_: Swipe

**Engagement**:
Whether a hand has control. A hand gains it by being raised open into the reach and loses it by being lowered. Engagement is not a gesture and can't be rebound.
_Avoid_: Control mode, activation

**Reach**:
The part of the camera's view that an engaged hand sweeps to cover the whole screen.

### Desktop

**Action**:
What the desktop receives: point, press, release or scroll.
_Avoid_: Input event, command

**Binding**:
A gesture paired with the action it causes.
_Avoid_: Mapping, shortcut

**Gesture file**:
The user's definitions of gestures, their bindings and the assist tuning. A gesture file is only used once it passes its checks.
_Avoid_: Config, profile, keymap

**Target**:
A thing on screen that can be selected, such as a button, a text field or a window's title bar.
_Avoid_: Widget, element, hotspot

**Assist**:
Help in landing on targets: friction slows the pointer over a target while the hand is aiming, and a tap near a target snaps to it.
_Avoid_: Aim assist, magnetism, snapping

### Cube world

**Arena**:
The box shapes live in: its walls, the grid placed shapes snap to (0.15 m cells) and the most shapes it holds (64). Walls and stretch limits follow it when it changes.
_Avoid_: Box, room, bounds

**Shape**:
A solid in the Arena that hands can grab: a Primitive with its parameters. New ones are placed on the Arena's grid.
_Avoid_: Cube, block, object

**Selection**:
The shape edits act on: the one under a cursor (a hand's or the mouse's), else the one grabbed last. It is outlined.
_Avoid_: Focus, active shape

**Primitive**:
The kind of solid a Shape is (box, rounded box, cylinder, frustum, cone), given by its parameters: per-axis size, roundness and taper. A Shape is never a mesh you edit (ADR 0004).
_Avoid_: Mesh, model

**Tool**:
What edits on the Selection do: Move (grab, turn, stretch), Round, Taper or Copy. Keys 1–4 pick one; `[` and `]` lower and raise its value.
_Avoid_: Mode, brush

**Material**:
What a Shape is made of: a preset (glass, wood, rubber, plastic, metal) with a colour, density, static and dynamic friction, restitution, whether it conducts, and an inert charge. Editing a preset changes every shape made of it. `M` gives the Selection the next one.
_Avoid_: Substance, physics material

**Template**:
A saved build: shapes on grid cells relative to the first, each with its parameters, turn and Material name. One RON file each in `~/.config/terrain/templates`. Stamping places it on the grid, all or nothing; its shapes are loose, not glued.
_Avoid_: Prefab, blueprint

**Library**:
The built-in templates (line, L, block, pyramid) and the user's saved ones. A file that fails to parse is skipped with a message.
_Avoid_: Catalog, palette
