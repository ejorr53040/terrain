# terrain

Hands seen by a webcam drive things on screen: 3D objects in the app, and the real desktop's pointer, buttons and windows.

## Language

### Hands

**Pose**:
The shape a hand is in during one frame: Open, Pinch, Fist, One, Vee, Three or Four. A Pose is only the shape, the same whichever way the hand faces and whichever hand it is; each world decides what it does (the cube world grabs with both Pinch and Fist, the desktop drags windows with a Fist).
_Avoid_: Grip, hand shape

**Vee**:
The pose with the index and middle fingers extended and the others curled. It is also the "two" of any finger count.
_Avoid_: Two-finger, Two, peace sign

**Finger count**:
The poses One (index finger), Vee, Three (index, middle, ring) and Four (all but the thumb), held up to pick something by number.
_Avoid_: Number pose, digits

**Pose example**:
One recorded frame of a hand labelled with the Pose it was held in. A hand's Pose is the label of the examples it is closest to.
_Avoid_: Sample, training data, template

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
The 2D surface on which file blocks (Files view) or Systems (Functionality view) float. Only those have a free position; every other block lives in a stack.
_Avoid_: Board, workspace, scene

**Sidecar**:
The file in the crate root that remembers the user's view of it: where blocks sit on the canvas, the Capabilities, the Outside systems and which plumbing crates are pinned. Never code.
_Avoid_: Layout file, project file, config

**Files view**:
The canvas showing file blocks and their stacks. The only view in which dragging moves code.

**Functionality view**:
The canvas showing Systems, their Capabilities and their Libraries. Dragging here only regroups.
_Avoid_: Infrastructure view, overview

**System**:
One crate of the workspace, shown as a block holding its Capabilities, its Unsorted items and its Libraries.
_Avoid_: Crate block, package, service

**Capability**:
A named set of a System's items that together do one thing, such as "VM lifecycle". Made by the user, view-only; an item is in at most one.
_Avoid_: Feature (a Cargo term), group, functionality, module

**Unsorted**:
A System's items that are in no Capability yet.
_Avoid_: Ungrouped, misc

**Library**:
Something outside the workspace that a System relies on: a Crate dependency or an Outside system. Each System shows its own copy.
_Avoid_: Package, external, dependency (alone)

**Crate dependency**:
A Library the System links against, read from its Cargo.toml. Plumbing crates are hidden unless pinned.
_Avoid_: Dep, package

**Outside system**:
A Library the System drives at run time rather than links, such as Firecracker, declared in the sidecar with the files that drive it.
_Avoid_: Service, binary, tool

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

**Overlay**:
The builder's menu: tools, the selected shape's sizes, the Library, Scene buttons (reset, calibrate, preview), physics and Materials. Every control there is also reachable by hand through the Overlay pointer, so rare settings need no gesture of their own.
_Avoid_: HUD, settings window

**Hot edge**:
The screen's right edge. A hand cursor resting there about 300 ms opens the Overlay; it closes once no cursor has been over it for about a second.
_Avoid_: Hot corner, drawer

**Overlay pointer**:
A hand cursor over the Overlay working it like a mouse: a pinch clicks, a pinch-drag moves a slider at half the hand's speed. A pinch that starts over the Overlay never grabs a Shape. The first hand in owns it until it leaves.
_Avoid_: Virtual mouse
