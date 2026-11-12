//! Dev-only, for `tools/autoresearch/metrics.py`: replays a clip through the
//! same estimator and gesture engine `DesktopControl` uses and prints one
//! JSON object per frame: each hand's raw (unsmoothed) and smoothed palm,
//! its pose, who has control, and the actions the frame produced.
//!
//!     dump_trace <frames.json>  > trace.jsonl

use glam::{Vec2, Vec3};
use serde_json::json;
use terrain_gestures::{Action, Engine, GestureFile, Hand, Pose};
use terrain_hands::{CameraModel, HandFrame, HandState, HandStateEstimator};

fn in_view(camera: CameraModel, position: Vec3) -> Vec2 {
    let p = camera.tilt.inverse() * position;
    let p = p.with_z(p.z.max(1e-3));
    let size = camera.view_size_at_1m() * p.z;
    Vec2::new(0.5 + p.x / size.x, 0.5 - p.y / size.y)
}

fn v3(v: Vec3) -> [f32; 3] {
    v.to_array()
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: dump_trace <frames.json>");
    let frames: Vec<HandFrame> =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read clip"))
            .expect("parse clip");
    let camera = CameraModel::default();
    let mut estimator = HandStateEstimator::new(camera);
    let mut engine = Engine::new(GestureFile::default_gestures());
    for frame in &frames {
        // A fresh estimator sees each hand for the first time, so its
        // filters pass the sample straight through: the raw palm.
        let raw = HandStateEstimator::new(camera).update(frame);
        let tracked = estimator.update(frame);
        let hands: Vec<Hand> = tracked
            .hands
            .iter()
            .map(|h: &HandState| Hand {
                id: h.id,
                pose: Pose::of(h),
                in_view: in_view(camera, h.position),
                position: h.position,
            })
            .collect();
        let actions = engine.update(&hands, frame.t_ms);
        let out_hands: Vec<_> = tracked
            .hands
            .iter()
            .zip(&hands)
            .enumerate()
            .map(|(i, (h, g))| {
                let raw_pos = raw.hands.get(i).map(|r| v3(r.position));
                json!({
                    "id": h.id,
                    "handedness": format!("{:?}", h.handedness),
                    "pos": v3(h.position),
                    "raw_pos": raw_pos,
                    "in_image": h.in_image.to_array(),
                    "in_view": g.in_view.to_array(),
                    "pose": format!("{:?}", g.pose),
                })
            })
            .collect();
        let acts: Vec<String> = actions
            .iter()
            .map(|a| match a {
                Action::PointTo(p) => format!("PointTo {:.4} {:.4}", p.x, p.y),
                Action::Scroll(s) => format!("Scroll {:.3} {:.3}", s.x, s.y),
                other => format!("{other:?}"),
            })
            .collect();
        println!(
            "{}",
            json!({
                "t_ms": frame.t_ms,
                "n_raw": frame.hands.len(),
                "hands": out_hands,
                "control": engine.controlling(),
                "status": format!("{:?}", engine.status()),
                "actions": acts,
            })
        );
    }
}
