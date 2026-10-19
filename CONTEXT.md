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
A pinch or fist moved quickly in one direction (left, right, up, down), however long it was held still first, as opposed to the slower move of a Drag. Flicks send herdr's key chords.
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

### Blocks

**Block**:
A file, inline module, `impl`, `trait` or item of a Rust crate, shown with its kind and name. An item's block includes its doc comments, attributes and leading comments.
_Avoid_: Node, card, tile

**Container**:
A block that holds other blocks: a file, an inline module, an `impl` or a `trait`.
_Avoid_: Parent, group

**Stack**:
The blocks directly inside a container, in source order. A block's place in its stack is its place in the file.
_Avoid_: List, children, column

**Canvas**:
The 2D surface on which file blocks float. Only file blocks have a free position; every other block lives in a stack.
_Avoid_: Board, workspace, scene

**Sidecar**:
The file in the crate root that remembers where each file block sits on the canvas, and nothing else.
_Avoid_: Layout file, project file, config
