# Webcam hand control of a desktop: what's on GitHub, and what terrain should take

Question: what do open-source webcam hand-tracking desktop controllers do (click, drag, push toward the camera, Linux/Wayland injection), and what should terrain borrow, first for the fist-push → Super+F gesture and then to make terrain easier to build?

Read 2026-10-20 for terrain. Depth itself (palm-only fit, log palm-span ratio, Kalman on log z, stereo) is in [hand-depth.md](hand-depth.md) and isn't repeated here. This doc covers what sits on top: how a push is told apart from a drag.

**Verdict: no webcam project has a push gesture worth copying. The best push design is Ultraleap TouchFree's AirPush (Apache-2.0, built for a depth sensor), and its main trick isn't depth at all: the drag is decided late.** Every webcam project found starts the drag (or click) the moment the pose closes. That is the same structure that made terrain's fist push read as a window drag. TouchFree inverts it: lateral motion has to escape a dead zone before a drag starts, and the push competes as an accumulated "force" that sideways motion drains.

## Verdict

### For the fist-push → Super+F gesture

1. **Decide the drag late.** When a Fist closes, enter a pending state and send nothing. A window drag starts only once the palm has moved sideways past a dead zone. Super+F fires if push force reaches 1 first. In TouchFree a press becomes a drag only after the cursor leaves a 30 mm radius around the press point (`_dragStartDistanceThresholdMm`, `CheckForStartDrag`). The cursor dead zone also *grows* while push force rises, so a push doesn't drift the pointer [TF-push L42–47, L219–225, L246–247, L280–301; TF-stab]. diyor-ai uses the same idea for clicks: the cursor is frozen 0.85 s after a pinch [diyor-cfg L60]. Cost: drag start is delayed by the dead-zone travel. TouchFree hides the jump by decaying the offset ×0.9 per frame [TF-stab L35–38].
2. **Accumulate push "force" from the motion angle of each frame.** Each frame, take the angle between the motion and the camera axis. Under 65° (toward the camera), add force: `k·v·dt`, where stiffness k rises with speed, so a fast push needs 8 mm and a slow one 42 mm. Between 65° and 135° (sideways), drain force: `−v_perp·dt / 50 mm`. Multiply by tracking confidence. Fire at 1.0. Re-arm only after force decays below 0.87 [TF-push L21–35, L185–188, L233–264, L303–389]. In terrain's units this maps onto hand-depth.md's dimensionless signals: forward = Δlog(palm span), sideways = Δpalm-centre / span. The mm constants need rescaling, because TouchFree tunes for finger pokes and terrain wants 8–15 cm. A force of 1 at about Δlog span 0.12 is an engineering guess, to be tuned on clips.
3. **Fire on the return, from a slow baseline, with hysteresis.** mudra's `DepthTapDetector` keeps a baseline EMA (α 0.04) that is frozen while armed. It arms when deviation passes 0.14, *fires when the value comes back* under 0.055, and has a 220 ms cooldown [mudra-g L4–44]. A drag never returns to its start depth. A push-and-return does. This adds ~200–400 ms of latency, which is fine for a fullscreen toggle. It's the cheapest separator if (1)–(2) still confuse.
4. **Bound the duration, and measure only after the pose has settled.** HaGRID's dynamic-gesture controller accepts a gesture only within min/max frame counts (`check_duration`, `check_duration_max`). Its drag also needs a distinct pre-pose ("grabbing" claw) for ≥3 frames before the fist [dyn-ac L52–90, L316–322]. For terrain: a push has to finish within about 250 ms, and slower depth changes are drags or drift. Start the push window only after the Fist has been stable for a few frames (diyor-ai: 3 frames [diyor-cfg L47]; Viral-Doshi: 5 [VD L209–218]; hand-gesture-3d: 90 ms [hg3d]). That puts the fist-closing depth shift from hand-depth.md outside the window.
5. **Don't copy the only webcam push detector found.** HandCam-Control's `PushDetector` uses the **bounding-box area of all 21 landmarks**. It fires when area grows >15% (normalised by current area) above a velocity threshold, with a 500 ms debounce, and has *no* lateral-motion rejection [HC-push L35–80; HC-cfg]. Curling fingers shrinks that box, and nothing separates push from move. terrain's palm-span ratio (hand-depth.md) already beats it. No README or issue reports how well it works.

### For making terrain easier to build

1. **Stay on uinput. libei isn't available on Hyprland.** xdg-desktop-portal-hyprland 1.4.x provides Screenshot, ScreenCast, GlobalShortcuts and InputCapture (added 2026-07). It does **not** provide RemoteDesktop, which is the libei injection route; issue #252 has been open since 2024-08 [xdph-portal; xdph-252]. The only Wayland projects found that inject input (mudra, diyor-ai, K-P-D-7) all use uinput with an absolute "tablet" pointer [mudra-m L108–147; diyor-in L5–8, L111–129; KPD]. Hyprland also implements `zwlr_virtual_pointer` and `zwp_virtual_keyboard`, a no-`/dev/uinput` fallback [hypr-proto]. The `reis` crate (pure-Rust libei, 0.7.1, 2026-07) is the one to use if xdph ever adds RemoteDesktop [reis].
2. **Don't swap the landmark model. Nothing better and permissive appeared in 2025–2026.** MediaPipe releases v0.10.25 → v1.1.1 (to 2026-10-14) contain C-API, LiteRT and One-Euro-smoother fixes, and no new hand model [MP-rel]. RTMPose-hand (rtmlib, Apache-2.0) is **2D only** (21 kpt, 256×256). It has no world landmarks, so `CameraModel::locate` would lose its metric hand shape [rtmlib]. Ultralytics hand-keypoint models are AGPL-3.0 [ultra]. Keep PINTO's MediaPipe "full".
3. **Put duration bounds and pose sequences into the gesture file.** dynamic_gestures (Apache-2.0, ONNX, 2025) describes every dynamic gesture as a deque of (pose, position) events plus min/max frame durations and required start poses [dyn-ac]. That's the same "gestures are data" shape terrain already has. Adding `max_ms` and `after_pose` fields would make (4) above, and future gestures, data rather than code. hand-gesture-3d (TypeScript, 2026) replays recorded landmark fixtures under Playwright [hg3d]. That confirms terrain's clip harness is the right tool; there's nothing new to take from it.

## Projects

Stars and last push are from the GitHub API on 2026-10-20. "Push" means a toward-the-camera gesture.

| Project | ★ / last push | Licence | Stack | Click / pinch | Push / depth | Smoothing | False-trigger handling |
|---|---|---|---|---|---|---|---|
| [ultraleap/TouchFree](https://github.com/ultraleap/TouchFree) | 28 / 2023-04 (archived) | Apache-2.0 | C#, Leap IR stereo | AirPush, AirClick, Hover-hold, TouchPlane, Grab | **Force accumulator** on z velocity + motion angle (sourced above) | Extrapolation + position filter + growing dead zone | Entry cooldown 300 ms; drag needs 30 mm escape; unclick hysteresis; force × confidence |
| [crakxx/mudra](https://github.com/crakxx/mudra) | 0 / 2026-10 | AGPL-3.0 | Python, MediaPipe ONNX via OpenCV DNN, uinput | Air: thumb–middle pinch, on/off 0.62/0.80 of palm scale, median of 3 | Desk mode: per-finger relative z vs learned rest, **fires on return** | One Euro; median of last points | Hysteresis, cooldown, frozen baseline while armed; fist flip needs 2 frames |
| [fikriaf/HandCam-Control](https://github.com/fikriaf/HandCam-Control) | 0 / 2025-11 | MIT | JS, MediaPipe Hands, Electron | Pinch detector | **bbox-area growth >15%** + velocity | Moving average of 3 | 500 ms debounce only |
| [ayus0109/hand_tracker](https://github.com/ayus0109/hand_tracker) | 0 / 2026-10 | none stated | Python + JS, MediaPipe | — (drawing app) | "Touch glass": index **tip z − MCP z** ≤ −0.04 [ayus L161–165] | EMA | None; this is finger tilt, not hand translation |
| [diyor-ai/gesture-control-system](https://github.com/diyor-ai/gesture-control-system) | 1 / 2026-10 | MIT | Python, MediaPipe, uinput abs pointer on Wayland | Thumb–tip pinch < 0.20 | None | — | Pose stable 3 frames; 5-frame hand-loss grace; cursor frozen 0.85 s after a click |
| [Viral-Doshi/Gesture-Controlled-Virtual-Mouse](https://github.com/Viral-Doshi/Gesture-Controlled-Virtual-Mouse) | 876 / 2023-11 | GPL-3.0 | Python, MediaPipe, PyAutoGUI (X11) | Finger-state poses (tip/base distance ratios) | None | Cursor at landmark 9 (middle MCP); dead zone ≤5 px, gain grows with speed [VD L339–361] | Pose must repeat 5 frames [VD L209–218] |
| [takeyamayuki/NonMouse](https://github.com/takeyamayuki/NonMouse) | 202 / 2026-08 (code 2023) | Apache-2.0 | Python, MediaPipe, pynput | Thumb tip ↔ index PIP / (wrist→thumb CMC) < 0.7 [NM L124–129, L160] | None | Moving average, fps/10 frames | Right-click = click held still 1.5 s; move only if index–middle spread |
| [ai-forever/dynamic_gestures](https://github.com/ai-forever/dynamic_gestures) | 129 / 2026-10 | Apache-2.0 (licence file) | Python, ONNX detector + crop classifier (HaGRIDv2), OC-SORT Kalman | Static-pose sequences ("clicks" gif) | Zoom via pose sequences, not depth | Kalman box tracker | Min/max frame durations; required pre-poses |
| [K-P-D-7/hand-gestures](https://github.com/K-P-D-7/hand-gestures) | 0 / 2026-10 | MIT | Python, MediaPipe, uinput, KDE Wayland | Scroll, tab switch, pause | None | — | Not read beyond README [KPD] |
| [MOBSAD/gesturedeck](https://github.com/MOBSAD/gesturedeck) | 2 / 2026-09 | MIT | Python, MediaPipe, Hyprland | Media and volume only, via `wpctl`/`playerctl`, no pointer | None | — | Open palm toggles control (like terrain's Engagement) |
| [leonardo-carnevalli/hand-gesture-3d](https://github.com/leonardo-carnevalli/hand-gesture-3d) | 0 / 2026-10 | MIT | TS, MediaPipe, Three.js | Classifier + debouncer | None | One Euro | New pose must hold 90 ms [hg3d]; Playwright replay fixtures |
| [kinivi/hand-gesture-recognition-mediapipe](https://github.com/kinivi/hand-gesture-recognition-mediapipe) (EN fork of Kazuhito00) | 813 / 2024-07 | Apache-2.0 | Python, MediaPipe, small MLPs | Keypoint MLP | None | — | "Finger gesture" MLP over a fingertip point history [kinivi] |

Searched and found nothing usable for push: `gh search repos` for virtual mouse, hand tracking mouse/Linux/Wayland/Rust, touchless, air touch, gesture desktop control; web searches for webcam push/press-toward-camera. Most hits are 0–5★ student MediaPipe + PyAutoGUI clones.

## Notes on the relevant ones

### Ultraleap TouchFree: AirPush (`AirPushInteraction.cs`, commit 7452704)
- A single scalar `_appliedForce` ∈ [0,1] drives everything. Press at 1.0, release below `_unclickThreshold` 0.97, and no re-press until force has decayed under 0.87 (`_decayingForce`) [TF-push L33–37, L197–264].
- Motion is classified each frame by `atan2(|Δxy|, Δz)`: <65° counts as toward, >135° as away, and anything between as horizontal. With dragging or swiping disabled, θ₁ tightens to 15° [TF-push L27–31, L117, L334–338].
- Stiffness `k = 1/42mm + r²·(1/8mm − 1/42mm)`, where r is the speed clamped to 150–500 mm/s. Fast pushes therefore trigger in a short distance and slow drift needs a long one [TF-push L345–354].
- Horizontal motion *drains* force (decay distance 50 mm) unless already pressing [TF-push L356–371]. This is the explicit push-vs-move separator.
- Past a virtual touch plane, force only rises, and slow creep can't trigger it [TF-push L320–331]. terrain has no plane, so skip that.
- It assumes metric z from a stereo IR sensor. On a webcam the input is the noisier log-span signal (hand-depth.md), so expect to raise the angle and force thresholds. **Not verified on webcam data by anyone.**

### mudra: out-and-back depth taps (`gestures.py`, commit 60c8d24)
- Desk mode (camera overhead) uses MediaPipe's relative z (tip − base, divided by hand scale) per finger. That is articulation, not hand distance, so the numbers don't carry over to a fist push. The state machine does: baseline → arm on large deviation → fire on return under a smaller threshold → cooldown [mudra-g L4–44].
- Air mode treats the fist as "grab = hold left button" immediately after a 2-frame flip debounce [mudra-m L695–716]. That is the same immediate-commit structure terrain has.
- The palm-only scale `max(|idxMCP−pinkyMCP|, |wrist−midMCP|)` normalises pinch distance [mudra-m L687–688]. It agrees with hand-depth.md's palm-only advice.
- The README says shortcuts go through the XDG GlobalShortcuts portal via an Electron helper, so the Python process never opens keyboard devices [mudra-readme].

### HandCam-Control: the only webcam "push to click" found (`PushDetector.js`, commit 0d1dd5b)
- Depth proxy = area of the 21-landmark bounding box. It takes a 3-frame moving average and a 10-sample history, then fires when `(newest − oldest)/current > 0.15` and velocity > 0.3 units/s, with a 500 ms debounce [HC-push L35–80; HC-cfg].
- Nothing checks lateral motion, and nothing checks pose change. A fist closing shrinks the box, so it would *suppress* a fist push. Effectiveness is unreported.

### ayus0109/hand_tracker: "push to touch glass"
- Uses `index_tip.z − index_mcp.z ≤ −0.04` from MediaPipe image z [ayus L161–165; ayus-cfg L81–82]. That measures the finger bending toward the camera, which a fist doesn't do. Not applicable.

### Click detection that holds up (for Pinch, not push)
- Scale-normalised distances: NonMouse divides by wrist→thumb-CMC [NM L124–129], and mudra by palm span [mudra-m L687–688]. Both add on/off hysteresis or a hold-still rule. terrain's pose examples already cover this.

## Wayland / Hyprland input injection (2026-10)

| Route | Works on Hyprland now? | Notes |
|---|---|---|
| uinput (`evdev` crate) | Yes | What terrain does. Every Wayland project found does the same [mudra-m; diyor-in; KPD]. Needs `/dev/uinput` access via a udev rule. |
| wlr virtual-pointer + virtual-keyboard | Yes: `VirtualPointer.cpp` and `VirtualKeyboard.cpp` are in Hyprland [hypr-proto] | No root or udev. Compositor-specific (wlroots-family). Keymap upload needed for keys. |
| libei via RemoteDesktop portal | **No.** xdph has no RemoteDesktop [xdph-portal]; issue open since 2024-08 [xdph-252] | A third-party AUR "xdg-desktop-portal-hypr-remote" exists; comments there report it not working (anecdotal, unverified). |
| Hyprland IPC dispatcher | Yes | `fullscreen({action = toggle})` exists in the current wiki [hypr-disp]. It could replace injecting Super+F, but it bypasses the user's binding and ties terrain to Hyprland, so it's optional. |

## New in 2025–2026

- **MediaPipe v1.0.0 (2026-07-28) and v1.1.x**: no new hand model. They add per-landmark presence scores (Web), fix a One-Euro landmark-smoother drift, refactor the Hand Landmarker C API and add a configurable empty-landmark policy for smoothers [MP-rel].
- **HaGRIDv2** (2024-09, paper 2412.01508): 1.09M FullHD images, 33 gestures + `no_gesture` [HaGRIDv2]. The licence is a *variant* of CC BY-SA 4.0 (custom PDF) [hagrid-lic]; read it before deriving Pose examples from it. These are images, not landmarks, so terrain would have to run its own model over them.
- **dynamic_gestures** (2025-02, Apache-2.0): ONNX hand detector + crop classifier + OC-SORT, rule-based dynamic gestures (swipes, zoom, drag-and-drop, clicks) [dyn; dyn-ac]. Useful as a gesture-grammar reference. Its models replace landmarks, so terrain shouldn't adopt them.
- **Rust**: no maintained Rust hand-tracking crate. `mediapipe` 0.1.0 (2026-07) wraps the Tasks C API for **face only** [mp-crate]. `mediapipe-rs` (WasmEdge) runs on WASI-NN inside WasmEdge, not natively [wasmedge]. `one-euro` 0.9.0 exists, but terrain has its own filter. `reis` 0.7.1 is pure-Rust libei [reis]. `ort` and `rten` are both active (2026-10) [ort; rten].
- **SPLite Hand** (arXiv 2510.16396, Oct 2025): reports a 2.98× speed-up on a Raspberry Pi 5 CPU and ONNX export [splite]. Code release and licence are **unverified**. It's evaluated on FreiHAND (MANO-based), so the MANO licence issue in hand-depth.md probably applies.

## Not worth it
- **HandCam's bbox-area push**: fingers corrupt the depth proxy, and it has no drag rejection (above).
- **Fingertip-vs-MCP image z** (ayus0109, mudra desk mode): this is articulation, and a fist has none.
- **Viral-Doshi, AirCtrl, NonMouse as code**: GPL or old, PyAutoGUI/pynput on X11, no push. Only the small ideas above are worth taking.
- **RTMPose-hand / RTMW**: 2D only, so they would break `locate`. Weight licences across the training datasets are unverified.
- **Ultralytics hand keypoints**: AGPL-3.0 [ultra].
- **Kinect-style "press" (Kinect for Windows interactions)**: not open source. Not checked.
- **libei now**: not on Hyprland (above).

## Unverified / gaps
- Nobody publishes accuracy or false-trigger rates for any webcam push gesture. TouchFree's tuning is for a stereo IR sensor.
- terrain's `terrain-desktop` crate isn't in this checkout (`crates/` holds only `terrain-app` and `terrain-hands`). The fist-drag wiring is described from the brief, not read.
- RTMPose hand weight licence and CPU latency; SPLite code; how well the third-party Hyprland RemoteDesktop portal works.

## Sources
- [TF-push] Ultraleap TouchFree `AirPushInteraction.cs` @7452704: https://github.com/ultraleap/TouchFree/blob/7452704dfc0faf50353c716fa8acf11e363975e0/TF_Service_dotNet/TouchFree/Interactions/InteractionModules/AirPushInteraction.cs
- [TF-stab] TouchFree `PositionStabiliser.cs`: https://github.com/ultraleap/TouchFree/blob/7452704dfc0faf50353c716fa8acf11e363975e0/TF_Service_dotNet/TouchFree/Interactions/PositionStabiliser.cs
- [mudra-g] mudra `gestures.py` @60c8d24: https://github.com/crakxx/mudra/blob/60c8d24b55286cdb6820f29eb050d1fa191655c6/gestures.py ; [mudra-m] `mudra.py`: https://github.com/crakxx/mudra/blob/60c8d24b55286cdb6820f29eb050d1fa191655c6/mudra.py ; [mudra-readme] https://github.com/crakxx/mudra#readme
- [HC-push] HandCam-Control `PushDetector.js` @0d1dd5b: https://github.com/fikriaf/HandCam-Control/blob/0d1dd5b0aee89263694d30df8e8d3907e5159c34/src/js/recognition/detectors/PushDetector.js ; [HC-cfg] `gestureConfig.js` (push block L20–26): https://github.com/fikriaf/HandCam-Control/blob/0d1dd5b0aee89263694d30df8e8d3907e5159c34/src/js/config/gestureConfig.js
- [ayus] ayus0109/hand_tracker `hand_tracker.py` @6cc58dc: https://github.com/ayus0109/hand_tracker/blob/6cc58dc8c6c6ad2ef2dbeb6a45be977336468a41/hand_tracker.py ; [ayus-cfg] `config.py`: https://github.com/ayus0109/hand_tracker/blob/6cc58dc8c6c6ad2ef2dbeb6a45be977336468a41/config.py
- [diyor-cfg] diyor-ai `modules/config.py` @21fc760: https://github.com/diyor-ai/gesture-control-system/blob/21fc7605de4d8ccd100a3e2775877003b6b4fb9f/modules/config.py ; [diyor-in] `modules/input_backend.py`: https://github.com/diyor-ai/gesture-control-system/blob/21fc7605de4d8ccd100a3e2775877003b6b4fb9f/modules/input_backend.py
- [VD] Viral-Doshi `src/Gesture_Controller.py` @da75197: https://github.com/Viral-Doshi/Gesture-Controlled-Virtual-Mouse/blob/da75197b10c2c48019488fbb76ca4be1332901e4/src/Gesture_Controller.py
- [NM] NonMouse `nonmouse/__main__.py` @0aaea67: https://github.com/takeyamayuki/NonMouse/blob/0aaea67039930be3a1e20d37f758db3c55042ca0/nonmouse/__main__.py
- [dyn] ai-forever/dynamic_gestures: https://github.com/ai-forever/dynamic_gestures ; [dyn-ac] `utils/action_controller.py` @557a836: https://github.com/ai-forever/dynamic_gestures/blob/557a836bf82fce8d36494d0c17e95ca236f5611c/utils/action_controller.py
- [hg3d] leonardo-carnevalli/hand-gesture-3d `src/gesture/debouncer.ts`: https://github.com/leonardo-carnevalli/hand-gesture-3d/blob/6f53ef67ded4b70dbc50d99f534d63d464e5c308/src/gesture/debouncer.ts
- [KPD] K-P-D-7/hand-gestures (README): https://github.com/K-P-D-7/hand-gestures
- [kinivi] kinivi/hand-gesture-recognition-mediapipe (README): https://github.com/kinivi/hand-gesture-recognition-mediapipe
- [xdph-portal] xdg-desktop-portal-hyprland `hyprland.portal`: https://github.com/hyprwm/xdg-desktop-portal-hyprland/blob/master/hyprland.portal ; InputCapture added in #268 (2026-07-14)
- [xdph-252] "Support for org.freedesktop.portal.RemoteDesktop?" (open): https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/252
- [hypr-proto] Hyprland `src/protocols/` (VirtualPointer, VirtualKeyboard): https://github.com/hyprwm/Hyprland/tree/main/src/protocols
- [hypr-disp] Hyprland wiki, dispatchers (source): https://github.com/hyprwm/hyprland-wiki/blob/main/content/configuring/core/dispatchers.md
- [reis] reis crate: https://crates.io/crates/reis
- [MP-rel] MediaPipe releases v0.10.25–v1.1.1: https://github.com/google-ai-edge/mediapipe/releases
- [rtmlib] rtmlib README (hand models table): https://github.com/Tau-J/rtmlib
- [ultra] Ultralytics licence (AGPL-3.0): https://github.com/ultralytics/ultralytics
- [HaGRIDv2] Nuzhdin et al., HaGRIDv2, arXiv 2412.01508: https://arxiv.org/abs/2412.01508 ; [hagrid-lic] https://github.com/hukenovs/hagrid/blob/master/license/en_us.pdf
- [mp-crate] `mediapipe` crate (nikicat, face only): https://github.com/nikicat/mediapipe-rs ; [wasmedge] WasmEdge/mediapipe-rs: https://github.com/WasmEdge/mediapipe-rs
- [ort] https://github.com/pykeio/ort ; [rten] https://github.com/robertknight/rten
- [splite] SPLite Hand, arXiv 2510.16396: https://arxiv.org/abs/2510.16396
