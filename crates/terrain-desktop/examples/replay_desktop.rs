//! Prints what `DesktopControl` would do over a recorded clip, without
//! touching the desktop: `replay_desktop <clip.frames.json>`.

use glam::Vec2;
use terrain_desktop::{DesktopControl, Input};
use terrain_hands::{CameraModel, HandFrame};

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: replay_desktop <frames.json>");
    let frames: Vec<HandFrame> =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read clip"))
            .expect("parse clip");
    let mut control = DesktopControl::new(CameraModel::default());
    let (mut lo, mut hi) = (Vec2::ONE, Vec2::ZERO);
    let mut at = Vec2::ZERO;
    for frame in &frames {
        for input in control.update(frame) {
            match input {
                Input::PointTo(p) => {
                    lo = lo.min(p);
                    hi = hi.max(p);
                    at = p;
                }
                Input::Scroll(_) => {}
                other => println!(
                    "{:>6} ms  {other:?} at ({:.2}, {:.2})",
                    frame.t_ms, at.x, at.y
                ),
            }
        }
    }
    println!(
        "{} frames; pointer ranged x {:.2}..{:.2}, y {:.2}..{:.2}",
        frames.len(),
        lo.x,
        hi.x,
        lo.y,
        hi.y
    );
}
