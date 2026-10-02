//! Writes `fixtures/demo-drag.json`: a hand that pinches, drags the cube
//! around a circle back to where it started, and lets go.
//!
//! cargo run -p terrain-app --example make_demo_fixture

#[path = "../tests/support/mod.rs"]
mod support;

use std::f32::consts::TAU;

use bevy::math::Vec3;
use support::{Pose, frame};

fn main() {
    let start = Vec3::new(0.0, 0.0, 0.5);
    let mut poses = vec![Pose::open(start); 30];
    poses.push(Pose::pinched(start));
    for i in 1..=120 {
        let a = TAU * i as f32 / 120.0;
        let offset = Vec3::new(a.cos() - 1.0, a.sin(), 0.0) * 0.08;
        poses.push(Pose::pinched(start + offset));
    }
    poses.extend([Pose::open(start); 30]);

    let frames: Vec<_> = poses
        .iter()
        .enumerate()
        .map(|(i, pose)| {
            let mut f = frame(&[*pose]);
            f.t_ms = i as u64 * 33;
            f
        })
        .collect();
    let path = terrain_app::DEMO_FIXTURE;
    std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_string(&frames).unwrap()).unwrap();
    println!("wrote {} frames to {path}", frames.len());
}
