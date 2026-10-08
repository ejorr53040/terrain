//! Builds synthetic `HandFrame`s: a canonical hand, open, pinching or in a
//! fist, placed in front of the default camera in meters.

#![allow(dead_code)]

use glam::{Vec2, Vec3};
use terrain_desktop::{DesktopControl, Input, Status};
use terrain_hands::{CameraModel, Hand, HandFrame, Handedness, landmark};

/// Time between camera frames in tests.
pub const FRAME_MS: u64 = 33;

/// An open right hand, palm to the camera, fingers up. Meters, x right, y up,
/// z away from the camera.
const OPEN_HAND: [[f32; 3]; 21] = [
    [0.000, -0.070, 0.0], // 0 wrist
    [-0.030, -0.050, 0.0],
    [-0.050, -0.030, 0.0],
    [-0.065, -0.010, 0.0],
    [-0.075, 0.010, 0.0], // 4 thumb tip
    [-0.025, 0.020, 0.0], // 5 index MCP
    [-0.028, 0.050, 0.0],
    [-0.030, 0.070, 0.0],
    [-0.031, 0.090, 0.0], // 8 index tip
    [0.000, 0.025, 0.0],  // 9 middle MCP
    [0.000, 0.060, 0.0],
    [0.000, 0.080, 0.0],
    [0.000, 0.100, 0.0],
    [0.020, 0.020, 0.0], // 13 ring MCP
    [0.022, 0.050, 0.0],
    [0.024, 0.070, 0.0],
    [0.025, 0.085, 0.0],
    [0.038, 0.010, 0.0], // 17 pinky MCP
    [0.042, 0.035, 0.0],
    [0.045, 0.050, 0.0],
    [0.047, 0.065, 0.0],
];

/// What the hand is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Grip {
    Open,
    /// Thumb tip on index tip, the other fingers straight.
    Pinch,
    /// Every finger folded into the palm, the thumb across them.
    Fist,
}

/// The hand `grip` makes, in the hand's own frame.
fn shape(grip: Grip) -> [Vec3; 21] {
    let mut p = OPEN_HAND.map(Vec3::from_array);
    match grip {
        Grip::Open => {}
        Grip::Pinch => p[4] = p[8] + Vec3::new(-0.01, 0.0, 0.0),
        Grip::Fist => {
            // Each finger's three joints fold down in front of its knuckle.
            for mcp in [5, 9, 13, 17] {
                for (k, fold) in [(1, 0.015), (2, 0.025), (3, 0.03)] {
                    p[mcp + k] = p[mcp] + Vec3::new(0.0, -fold * 0.6, -fold);
                }
            }
            // The thumb lies across the folded fingers, near the index.
            p[4] = p[8] + Vec3::new(0.0, 0.0, -0.02);
        }
    }
    p
}

/// A right hand making `grip` with its palm `at` (meters, camera space:
/// x right, y up, z away), as the default camera's tracker reports it.
pub fn hand(at: Vec3, grip: Grip) -> Hand {
    let local = shape(grip);
    let palm = landmark::PALM.map(|i| local[i]).iter().sum::<Vec3>() / landmark::PALM.len() as f32;
    let camera = CameraModel::default();
    let half_w = (camera.hfov_deg.to_radians() / 2.0).tan();
    let half_h = half_w / camera.aspect;
    let image = local.map(|p| {
        let c = at + p - palm;
        Vec3::new(
            0.5 + c.x / (2.0 * half_w * c.z),
            0.5 - c.y / (2.0 * half_h * c.z),
            p.z - palm.z,
        )
    });
    Hand {
        handedness: Handedness::Right,
        score: 1.0,
        image,
        world: local.map(|p| Vec3::new(p.x - palm.x, -(p.y - palm.y), p.z - palm.z)),
    }
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

    /// The hand making `grip` at `at` for `n` frames.
    pub fn hold(mut self, at: Vec3, grip: Grip, n: usize) -> Self {
        for _ in 0..n {
            let t_ms = self.t();
            self.frames.push(HandFrame {
                t_ms,
                hands: vec![hand(at, grip)],
            });
        }
        self
    }

    /// Two hands, `a` and `b`, each at its place making its grip, for `n` frames.
    pub fn hold_two(mut self, a: (Vec3, Grip), b: (Vec3, Grip), n: usize) -> Self {
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
    pub fn sweep(mut self, from: Vec3, to: Vec3, grip: Grip, n: usize) -> Self {
        for i in 1..=n {
            let t_ms = self.t();
            self.frames.push(HandFrame {
                t_ms,
                hands: vec![hand(from.lerp(to, i as f32 / n as f32), grip)],
            });
        }
        self
    }

    /// One hand held open at `still` while the other, making `grip`, moves
    /// in a straight line from `moving.0` to `moving.1` over `n` frames.
    pub fn sweep_two(mut self, still: Vec3, moving: (Vec3, Vec3), grip: Grip, n: usize) -> Self {
        for i in 1..=n {
            let t_ms = self.t();
            let at = moving.0.lerp(moving.1, i as f32 / n as f32);
            self.frames.push(HandFrame {
                t_ms,
                hands: vec![hand(still, Grip::Open), hand(at, grip)],
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
    pub fn run(&self) -> Vec<Input> {
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
pub fn buttons(inputs: &[Input]) -> Vec<Input> {
    inputs
        .iter()
        .copied()
        .filter(|i| matches!(i, Input::Press(_) | Input::Release(_)))
        .collect()
}

/// Where the pointer was last sent.
pub fn last_point(inputs: &[Input]) -> Option<Vec2> {
    inputs.iter().rev().find_map(|i| match i {
        Input::PointTo(at) => Some(*at),
        _ => None,
    })
}

/// Where the pointer was when each button input happened.
pub fn point_at_each_button(inputs: &[Input]) -> Vec<(Input, Option<Vec2>)> {
    let mut at = None;
    let mut out = Vec::new();
    for &i in inputs {
        match i {
            Input::PointTo(p) => at = Some(p),
            other => out.push((other, at)),
        }
    }
    out
}
