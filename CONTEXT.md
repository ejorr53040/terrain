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
