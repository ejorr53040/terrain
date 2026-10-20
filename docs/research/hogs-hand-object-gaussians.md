# HOGS: hand-object modelling from monocular RGB via 3D Gaussians

Liu, Ren, Qi, Sun, Zhuang, Wang, Liao, Wang (BUPT). NeurIPS 2025.
Paper: https://proceedings.neurips.cc/paper_files/paper/2025/file/b914837ae7d2e3dcf33afd75180ed346-Paper-Conference.pdf
Code: https://github.com/ru1ven/HOGS

Read 2026-10-15 for terrain. **Verdict: background reading, not something to build on now.** It reconstructs and re-renders a real hand holding a real object, offline, on a GPU. terrain tracks a bare hand in real time on CPU and moves virtual shapes. A few ideas carry over (below).

## What it does

From single-camera RGB images of a hand holding an object, it builds 3D Gaussian Splatting models of the hand and the object that can be re-posed and re-rendered photo-realistically, and that generalise to new scenes, lighting and motions without per-video retraining.

- **Representation.** Hand: 5,000 Gaussians sampled on the canonical MANO hand surface, moved by linear blend skinning from hand pose θ. Object: 5,000 Gaussians in its bounding box, moved rigidly by a 6-DoF pose. An off-the-shelf regressor (HOISDF) supplies the starting hand pose and object pose.
- **V-PM (vision-driven perception).** A ViT encodes the 224×224 hand-object crop. Each Gaussian samples the image feature under its projection, and an MLP predicts a blend between its learned colour and an image-driven colour, plus small offsets to position, scale and rotation. This separates lasting geometry from passing appearance (shadows, highlights, blur).
- **G-PM (geometry-driven pose refinement).** Treats Gaussian centres as a point cloud. PointNet++ (pretrained with ULIP) plus a transformer and an MLP output a correction to the object's translation and rotation relative to the hand. Contact and penetration losses between hand and object points keep the grasp physically plausible, without ground-truth contact labels.
- **Supervision.** 2D only: per-component RGB, mask and LPIPS losses, plus SmoothL1 on 6-DoF pose (rotation, translation, box corners) when available. No dense 3D surface labels.
- **Cost.** About 10 h on one RTX 4090 for DexYCB (360k iterations). No inference speed is reported anywhere in the paper.

## Results (as reported)

- DexYCB object reconstruction: best F@10 0.918 (Chamfer 0.24 cm²), ahead of 2D-supervised GOF and MOHO and of 3D-supervised gSDF. Hand reconstruction trails the 3D-supervised gSDF (Chamfer 0.481 vs 0.302).
- HO3D: object pose error CD_pose 2.7 vs HOLD's 11.3; shape quality about equal to HOLD.
- Rendering on DexYCB: PSNR 31.12 vs 3DGS-Avatar's 29.71. Under colour jitter it loses less than the baselines.
- Ablations: dropping G-PM hurts pose most (ADD-S 11.87 → 13.31). Dropping V-PM hurts rendering most (PSNR 31.12 → 29.57).
- Stated limit: object categories must be seen in training, so it doesn't synthesise new categories zero-shot.

## Relevance to terrain

Where it does **not** fit:
- Real-time budget. terrain's pipeline targets p95 < 25 ms on CPU (#15). HOGS needs a ViT, PointNet++ and Gaussian rasterisation on a GPU, and reports no inference speed.
- Inputs. It assumes a tracked MANO hand pose and an object 6-DoF pose from a separate regressor, and is trained on RGB-D datasets with masks. terrain has 21 MediaPipe landmarks and no depth camera.
- Goal. It reconstructs real objects. terrain's shapes are virtual and already known exactly (Arena, Shape).

Ideas worth borrowing:
1. **Contact and penetration as plausibility checks** (G-PM losses, from Hasson et al. 2019). For grabs in the cube world: treat the fingertip landmarks as points, and judge "is this hand really holding the Shape" by distance to the Shape's surface and by penetration. That's an alternative or tie-breaker to the pinch-point-inside-outline rule, and possibly a way to tell a real two-hand stretch from a duplicate hand.
2. **Split what lasts from what's passing.** The paper's core move separates geometry from transient appearance. For terrain's pose examples (ADR 0006), the analogue is comparing landmarks in the palm's own frame (lasting shape) rather than raw image coordinates (camera angle, distance).
3. **Robustness test by colour jitter.** They apply ±0.3 colour jitter to the inputs and measure the drop. The same check could stress MediaPipe on terrain's clips (jitter the JPEGs, replay, compare pose accuracy and dropouts) before relying on poses in dim rooms.
4. **Hand rendering.** If terrain ever draws a real-looking hand instead of a landmark skeleton, MANO plus linear blend skinning of Gaussians is the approach; LiveHand (ref 31) is the real-time option to read first.

Pointers: DexYCB (582K RGB-D frames, 10 subjects, 20 objects) and HO3D v3 are the standard hand-object benchmarks, if terrain ever wants grasp data beyond its own clips.
