//! Scripts of synthetic `HandFrame`s (terrain-hands' synthetic hand, open,
//! pinching, in a fist or a Vee, placed in front of the default camera in
//! meters), and what `DesktopControl` does over them.

#![allow(dead_code)]

use glam::{Vec2, Vec3};
use terrain_desktop::{Action, DesktopControl, Status};
use terrain_hands::{CameraModel, Hand, HandFrame};

pub use terrain_hands::synthetic::Fingers;
use terrain_hands::synthetic::Pose;

/// Time between camera frames in tests.
pub const FRAME_MS: u64 = 33;

/// A right hand making `fingers` with its palm `at` (meters, camera space:
/// x right, y up, z away), as the default camera's tracker reports it.
pub fn hand(at: Vec3, fingers: Fingers) -> Hand {
    terrain_hands::synthetic::hand(Pose::new(at, fingers))
}

/// A scripted run of frames, `FRAME_MS` apart.
#[derive(Default)]
pub struct Script {
    pub frames: Vec<HandFrame>,
}

impl Script {
    fn t(&self) -> u64 {
        self.frames.len() as u64 * FRAME_MS
    }

    /// The hand making `fingers` at `at` for `n` frames.
    pub fn hold(mut self, at: Vec3, fingers: Fingers, n: usize) -> Self {
        for _ in 0..n {
            let t_ms = self.t();
            self.frames.push(HandFrame {
                t_ms,
                hands: vec![hand(at, fingers)],
            });
        }
        self
    }

    /// Two hands, `a` and `b`, each at its place making its fingers, for `n` frames.
    pub fn hold_two(mut self, a: (Vec3, Fingers), b: (Vec3, Fingers), n: usize) -> Self {
        for _ in 0..n {
            let t_ms = self.t();
            self.frames.push(HandFrame {
                t_ms,
                hands: vec![hand(a.0, a.1), hand(b.0, b.1)],
            });
        }
        self
    }

    /// The hand moving in a straight line from `from` to `to` over `n` frames.
    pub fn sweep(mut self, from: Vec3, to: Vec3, fingers: Fingers, n: usize) -> Self {
        for i in 1..=n {
            let t_ms = self.t();
            self.frames.push(HandFrame {
                t_ms,
                hands: vec![hand(from.lerp(to, i as f32 / n as f32), fingers)],
            });
        }
        self
    }

    /// One hand held open at `still` while the other, making `fingers`, moves
    /// in a straight line from `moving.0` to `moving.1` over `n` frames.
    pub fn sweep_two(
        mut self,
        still: Vec3,
        moving: (Vec3, Vec3),
        fingers: Fingers,
        n: usize,
    ) -> Self {
        for i in 1..=n {
            let t_ms = self.t();
            let at = moving.0.lerp(moving.1, i as f32 / n as f32);
            self.frames.push(HandFrame {
                t_ms,
                hands: vec![hand(still, Fingers::Open), hand(at, fingers)],
            });
        }
        self
    }

    /// Two hands making `fingers`, each moving in a straight line (from, to)
    /// over `n` frames.
    pub fn sweep_both(
        mut self,
        a: (Vec3, Vec3),
        b: (Vec3, Vec3),
        fingers: Fingers,
        n: usize,
    ) -> Self {
        for i in 1..=n {
            let t_ms = self.t();
            let s = i as f32 / n as f32;
            self.frames.push(HandFrame {
                t_ms,
                hands: vec![
                    hand(a.0.lerp(a.1, s), fingers),
                    hand(b.0.lerp(b.1, s), fingers),
                ],
            });
        }
        self
    }

    /// `n` frames with no hand in view.
    pub fn gone(mut self, n: usize) -> Self {
        for _ in 0..n {
            let t_ms = self.t();
            self.frames.push(HandFrame {
                t_ms,
                hands: vec![],
            });
        }
        self
    }

    /// Everything a fresh `DesktopControl` does over the script.
    pub fn run(&self) -> Vec<Action> {
        let mut control = DesktopControl::new(CameraModel::default());
        self.frames.iter().flat_map(|f| control.update(f)).collect()
    }

    /// What a fresh `DesktopControl` says it's doing at the script's end.
    pub fn status(&self) -> Status {
        let mut control = DesktopControl::new(CameraModel::default());
        for f in &self.frames {
            control.update(f);
        }
        control.status()
    }
}

/// Presses and releases only, without the pointer moves and scrolling
/// between them.
pub fn buttons(inputs: &[Action]) -> Vec<Action> {
    inputs
        .iter()
        .copied()
        .filter(|i| matches!(i, Action::Press(_) | Action::Release(_)))
        .collect()
}

/// Where the pointer was last sent.
pub fn last_point(inputs: &[Action]) -> Option<Vec2> {
    inputs.iter().rev().find_map(|i| match i {
        Action::PointTo(at) => Some(*at),
        _ => None,
    })
}

/// Where the pointer was when each button input happened.
pub fn point_at_each_button(inputs: &[Action]) -> Vec<(Action, Option<Vec2>)> {
    let mut at = None;
    let mut out = Vec::new();
    for &i in inputs {
        match i {
            Action::PointTo(p) => at = Some(p),
            other => out.push((other, at)),
        }
    }
    out
}
