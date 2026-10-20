# Hand depth from one webcam: what would make it better

Question: how can terrain judge hand depth (distance from the camera) better from one ordinary laptop or USB webcam, on CPU, without adding much latency?

Read 2026-10-18 for terrain. **Verdict: the big error is bias, not noise, and the cheapest fix is in terrain's own solver.** Fit depth from the five rigid palm landmarks only, not all 21. Then detect depth gestures from relative image scale rather than from smoothed metric z. Learned hand-mesh and monocular-depth models don't fix absolute depth on one camera, and their licences or CPU cost rule them out anyway. If depth has to be truly right, a stereo or depth camera is the step that removes the bias; no model does.

## How terrain does it today (read from the code)

The brief described depth as focal × fixed bone length / pixel bone length. The current code does something better than that already:

- `CameraModel::locate` (`crates/terrain-hands/src/state.rs`) takes MediaPipe's **world landmarks** (metric 3D hand shape, origin at the hand's centre) and the **image landmarks**, and solves a linear least-squares problem for the hand's origin in camera space across **all 21 landmarks**: two pinhole equations per landmark. Every landmark has equal weight, fingertips included.
- Palm position = origin + mean of the five `PALM` landmarks (wrist and four MCPs). It's rotated by the calibrated `tilt`. z is then smoothed by a One Euro filter (`DEPTH_SMOOTHING`: min_cutoff 0.5 Hz, beta 3.0), and x/y separately (min_cutoff 1.0, beta 20).
- Calibration (`crates/terrain-app/src/calibration.rs` and `CameraModel::calibrated`): the hand is held upright 50 cm away for 1 s, the FOV is refitted until the mean palm z reads 0.5 m (accepted range 40–110°), and tilt comes from the mean wrist→middle-MCP direction.
- Image-landmark z (`landmarks.rs`, `DEPTH_SCALE` 0.4) is read in but not used for depth.

Two consequences:
1. **Per-user hand size is already calibrated, implicitly.** In a pinhole solve, depth scales with (hand size ÷ focal length). Fitting the FOV at a known distance absorbs the ratio between the user's hand and the model's assumed hand into the FOV. A separate hand-size calibration would add nothing unless the true FOV is known independently.
2. **The fist shift is the world landmarks' shape changing, mainly the fingers.** When fingers curl, the predicted metric finger geometry (and the curled fingertips' image positions) no longer match well, and 16 of the 21 equal-weight equations come from fingers.

## What the sources say

### MediaPipe's own outputs
- Image landmarks: "`z` represents the landmark depth with the depth at the wrist being the origin… The magnitude of `z` uses roughly the same scale as `x`." World landmarks: "real-world 3D coordinates in meters with the origin at the hand's approximate geometric center." [MP-docs]
  → Image z is **wrist-relative**: it can't give distance from the camera. World landmarks give shape, not position. terrain already combines them the right way.
- Model card: world landmarks are "metric scale… based on the GHUM hand model". Screen z and metric x, y, z are "provided using synthetic data, obtained via the GHUM model fitted to 2D point projections." Only 2D accuracy is evaluated, because the 3D values come from synthetic data. [MP-card p.1–2, 4]
  → No published metric accuracy for world landmarks, and the scale is a model prior (an average hand), not a measurement.
- Model card: "per-joint MNAE is the smallest at the base of each finger, and gets larger toward the fingertip… the prediction is easier around the palm which is more rigid than the fingers," and error is larger for blurry or occluded joints. [MP-card p.6]
  → **This is the primary-source reason to fit depth from the palm landmarks.** They're the most accurate in 2D and they don't move when a fist closes.
- Licence: Apache-2.0 (model card). The hand landmarker full pipeline is 17.1 ms on a Pixel 6 CPU. [MP-card p.2, MP-guide]

### Scale ambiguity is fundamental
- Iqbal et al. (ECCV 2018), 2.5D heatmaps: the method "estimates pose up to a scaling factor", which "can be estimated additionally if a prior of the hand size is given." [Iqbal]
  → Every single-camera method gets absolute depth from a hand-size prior, either explicit (terrain's world landmarks plus calibration) or learned (an average hand inside a network). A learned prior can't know that *this* user's hand is 8% small. terrain's calibration already can.

### Learned hand-mesh models
| Model | What it is | Absolute depth | CPU / speed | Licence | Verdict |
|---|---|---|---|---|---|
| HaMeR (CVPR 2024) | ViT-H backbone → MANO | Camera translation under "fixed camera intrinsics K" [HaMeR-paper] | None reported; CUDA install [HaMeR-repo] | Code MIT, **needs MANO** | No: huge, GPU |
| WiLoR (2024/25) | Fast detector + ViT-L refiner → MANO | **Weak-perspective** camera [WiLoR-paper] | Detector 138–175 FPS on an RTX 4090; reconstruction speed unreported [WiLoR-paper] | **CC-BY-NC-ND** + MANO [WiLoR-repo] | No: licence |
| Fast-HaMeR (Mar 2026) | HaMeR distilled to MobileNet/ResNet etc. | Same as HaMeR | ~35% the size, 1.5× faster than HaMeR; hardware unstated [FastHaMeR] | Code licence unstated; MANO-based | No: still far from CPU real time |
| MobRecon (CVPR 2022) | Lightweight 2D→mesh | Root-relative mesh | **83 FPS on Apple A14 CPU** [MobRecon] | Repo MIT, **needs MANO** [HandMesh] | Only one fast enough; licence blocks it |
| MeshGraphormer, Hamba | Transformer / Mamba → MANO | Same family | GPU | MANO | No |

- **MANO licence**: non-commercial only. The model files "shall not be copied, shared, distributed", and derivative works carry the same restrictions. [MANO]
  → Any MANO-output model can't ship with an MIT/Apache project. Users would have to fetch MANO themselves under a research-only licence.
- None of these fixes absolute depth on one camera. They use a weak-perspective or fixed-focal camera plus a learned average hand (above). Their gain is better root-relative articulation, and terrain only needs the palm for depth.
- A 2026 Knowledge-Based Systems paper (Guo et al., "pseudo stereo perception") targets camera-space metric hand depth with a depth-map head. Full text wasn't reachable, there's no code or speed figure, so it's unevaluated. [Guo]

### Fitting a hand model (MANO-style) to the 21 landmarks yourself
- This gives metric pose only up to the same scale prior, and MANO itself has the licence above. A licence-free alternative is a rigid **palm template** (the five palm landmarks' metric layout, recorded per user at calibration), solved as translation-only PnP using MediaPipe's palm rotation. That's a few dozen flops, it's rigid by construction, and fingers can't move it. This is recommendation 1b below.

### Monocular depth networks
- Depth Anything V2: **Small (24.8M params) is Apache-2.0**; Base/Large/Giant are CC-BY-NC-4.0. The base models give "relative depth". [DAv2]
- Metric checkpoints exist for Small/Base/Large, trained on Hypersim (indoor, max 20 m) and Virtual KITTI 2. Their licence isn't stated in `metric_depth/`. [DAv2-metric]
- No CPU latency is published. A ViT-S encoder plus DPT head at its usual 518 px input is far heavier than the 224 px CNN landmark model terrain runs in 8–13 ms. Expect it to cost several times the whole current tracking budget (unmeasured here).
- Relative depth is affine-ambiguous per frame, so it can't fix scale. The metric indoor model is trained for rooms up to 20 m, not centimetre accuracy on a hand 40–70 cm away. **Not worth it.**

### Filtering
- The One Euro filter (Casiez, Roussel, Vogel, CHI 2012) adapts its cutoff to speed: low jitter when still, low lag when fast. [1euro] It treats each axis alone and has no notion of measurement confidence.
- terrain's `DEPTH_SMOOTHING` (min_cutoff 0.5 Hz, beta 3) is heavily smoothed by design. **A quick 8 cm push is attenuated and delayed by this filter.** The gesture test then compares a lagged, shrunken z change against x/y drift, which is filtered much more loosely (beta 20). That's a likely contributor to the 25 failed push attempts, on top of the noise.
- A constant-velocity Kalman filter on **log depth** (log so noise is proportional, as scale noise is) can take a per-frame measurement variance, such as the least-squares residual of `locate`. Bad frames (blur, fist mid-close) then count less instead of jerking z. Cost is microseconds. Fusing image-landmark z adds nothing, since it's wrist-relative (above).

### Hardware
| Option | Depth source | Linux / licence | Notes |
|---|---|---|---|
| Synced dual-lens UVC "stereo webcam" | Triangulate MediaPipe landmarks seen in both views | Plain V4L2/UVC, no SDK | Removes the scale prior and fist bias entirely. Costs a second landmark pass (+8–13 ms unless run in parallel) and a one-off stereo calibration. |
| RealSense D405 / D435 | Active/passive stereo depth map | librealsense Apache-2.0, Ubuntu supported; repo moved to the `realsenseai` org [RS] | D405 is built for close range (about 7–50 cm per RealSense support community answers). D435 minimum distance is listed as 0.105–0.28 m by distributors, unofficial [RS-range]. Sample depth at the palm landmarks: near-zero compute. |
| Orbbec Gemini 2 / 330, Femto | Stereo / ToF depth | OrbbecSDK v2 MIT, Ubuntu 20.04–24.04 x86_64 [Orbbec] | Same use as RealSense. |
| Luxonis OAK-D | Stereo + on-device NN | depthai-core MIT, Linux [OAK] | Could even run the hand model on-device. |
| Ultraleap Leap Motion Controller 2 | IR stereo hand tracker | Official Linux target is Ubuntu 22.04 [UL]; proprietary SDK | Replaces the whole tracker, not just depth. Out of step with terrain's own pipeline. |

Stereo noise vs monocular noise, worked for a 640 px wide 65° camera (f ≈ 500 px):
- Monocular: palm span about 8 cm → 80 px at 0.5 m. 1 px of span noise ≈ 1.25% ≈ **6 mm** (12 mm at 0.7 m).
- Stereo, 6 cm baseline: σz = z²·σd / (f·B) ≈ **8 mm** per 1 px of disparity noise at 0.5 m (17 mm at 0.7 m).
- So **stereo doesn't cut noise much. It removes bias.** The fist shift (3–5 cm at ~50 cm, i.e. 6–10%) is about 5–10× the per-frame noise. Bias is the thing to fix first, and fixing it in software comes first.

## The push gesture specifically

An 8 cm push from 60 to 52 cm grows the palm's image span by about 15% (67 → 77 px). That's a large, clean image signal. It is lost because:
1. it's read through heavily smoothed metric z (lag), and
2. it's compared against x/y drift in metres, where x/y is far less noisy.

Detect the push from the **raw ratio of palm span over a short window** (say 150–250 ms), e.g. log(span_now / span_then) > 0.1, and require the palm's image-centre drift over the same window to be small *relative to the span* (drift / span), not in metres. A ratio needs no calibration, ignores hand size and FOV errors, and the palm span doesn't change when the fingers curl.

## Tilt

"Raising a hand reads as pushing away" is what `tilt` already corrects. But tilt is fitted from the wrist→middle-MCP direction of a hand the user *believes* is vertical, so a few degrees of hand lean become camera tilt. A sturdier fit is to ask for a slow vertical raise during calibration and take the direction of the palm's 3D path as "up". Path direction averages out world-landmark shape error. (Engineering suggestion, not from a source.)

## Recommendation (cheapest first that actually helps)

1. **Fit depth from the palm only.** In `CameraModel::locate`, use the five `landmark::PALM` landmarks, not all 21. Variant 1b: replace their per-frame world positions with a per-user palm template recorded at calibration, rotated by `palm_rotation()`. Zero latency, a few lines. Backed by the model card's finding that palm landmarks are the most accurate and rigid [MP-card p.6]. It should remove most of the fist shift. Check with `crates/terrain-hands/examples/replay_clip.rs` on a recorded open→fist→open clip at fixed distance: palm z range before vs after. Fewer equations will raise noise slightly, which item 2 handles.
2. **Detect gestures from image-scale ratio, and filter z with confidence.** Push detection: short-window log palm-span ratio, with drift normalised by span (above). Position: swap the z One Euro for a constant-velocity Kalman on log z, with measurement variance from the `locate` residual. Microseconds of CPU.
3. **If depth must be truly right: add stereo.** The cheapest route is a synced dual-lens UVC webcam with the landmarks triangulated: no SDK, no licence issues, +one landmark pass. The low-compute route is a RealSense D405/D435 or an Orbbec Gemini (Apache/MIT SDKs, Linux), sampling depth at the palm landmarks.

Not recommended: HaMeR / WiLoR / MeshGraphormer / Hamba / MobRecon (MANO licence, GPU or no absolute-depth gain), Depth Anything V2 (relative only, heavy on CPU), MediaPipe image z (wrist-relative), separate hand-size calibration (already absorbed by the FOV fit).

## Sources
- [MP-docs] MediaPipe Hands legacy solution docs: https://github.com/google-ai-edge/mediapipe/blob/master/docs/solutions/hands.md
- [MP-guide] MediaPipe Hand Landmarker guide (models, Pixel 6 latency): https://developers.google.com/edge/mediapipe/solutions/vision/hand_landmarker
- [MP-card] Model Card, MediaPipe Hands (Lite/Full), Oct 2021: https://storage.googleapis.com/mediapipe-assets/Model%20Card%20Hand%20Tracking%20(Lite_Full)%20with%20Fairness%20Oct%202021.pdf
- [Iqbal] Iqbal et al., Hand Pose Estimation via Latent 2.5D Heatmap Regression, ECCV 2018: https://arxiv.org/abs/1804.09534
- [HaMeR-paper] Pavlakos et al., Reconstructing Hands in 3D with Transformers, CVPR 2024: https://arxiv.org/abs/2312.05251 ; [HaMeR-repo] https://github.com/geopavlakos/hamer
- [WiLoR-paper] Potamias et al., WiLoR: https://arxiv.org/abs/2409.12259 ; [WiLoR-repo] https://github.com/rolpotamias/WiLoR
- [FastHaMeR] Fast-HaMeR, arXiv 2603.16444 (Mar 2026): https://arxiv.org/abs/2603.16444
- [MobRecon] Chen et al., MobRecon, CVPR 2022: https://arxiv.org/abs/2112.02753 ; [HandMesh] https://github.com/SeanChenxy/HandMesh
- [Guo] Guo et al., Camera-space hand mesh reconstruction via pseudo stereo perception, Knowledge-Based Systems 339 (2026), doi 10.1016/j.knosys.2026.115583: https://researchportal.hw.ac.uk/en/publications/camera-space-hand-mesh-reconstruction-from-a-monocular-image-via-/
- [MANO] MANO licence: https://mano.is.tue.mpg.de/license.html
- [DAv2] Depth Anything V2: https://github.com/DepthAnything/Depth-Anything-V2 ; [DAv2-metric] https://github.com/DepthAnything/Depth-Anything-V2/tree/main/metric_depth
- [1euro] Casiez, Roussel, Vogel, 1€ Filter, CHI 2012: https://gery.casiez.net/1euro/
- [RS] librealsense: https://github.com/IntelRealSense/librealsense ; [RS-range] RealSense support community / distributor listings (not an official datasheet): https://support.realsenseai.com/hc/en-us/community/posts/31267175599251
- [Orbbec] OrbbecSDK v2: https://github.com/orbbec/OrbbecSDK_v2
- [OAK] depthai-core: https://github.com/luxonis/depthai-core
- [UL] Ultraleap on Linux: https://docs.ultraleap.com/linux/index.html
