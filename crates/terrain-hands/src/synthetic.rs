//! Synthetic hands for tests: a canonical hand in a pose, placed in front of
//! a pinhole camera, so tests can speak in real-world meters.

use glam::{Quat, Vec3};

use crate::{CameraModel, Hand, HandFrame, Handedness, landmark};

/// Thumb-to-index gap of a firmly closed pinch, in meters.
pub const PINCHED_GAP: f32 = 0.01;

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

/// What the fingers are doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fingers {
    Open,
    /// Thumb tip on index tip (`PINCHED_GAP` apart), the other fingers straight.
    Pinch,
    /// Thumb tip this far (meters) left of the index tip.
    Gap(f32),
    /// Every finger folded into the palm, the thumb across them.
    Fist,
    /// Index and middle fingers straight, ring and little finger folded,
    /// the thumb holding them down.
    Vee,
}

impl Fingers {
    /// The hand's landmarks, in the hand's own frame.
    fn shape(self) -> [Vec3; 21] {
        let mut p = OPEN_HAND.map(Vec3::from_array);
        match self {
            Fingers::Open => {}
            Fingers::Pinch => p[4] = p[8] + Vec3::new(-PINCHED_GAP, 0.0, 0.0),
            Fingers::Gap(gap) => p[4] = p[8] + Vec3::new(-gap, 0.0, 0.0),
            Fingers::Fist => {
                fold(&mut p, &[5, 9, 13, 17]);
                // The thumb lies across the folded fingers, near the index.
                p[4] = p[8] + Vec3::new(0.0, 0.0, -0.02);
            }
            Fingers::Vee => {
                fold(&mut p, &[13, 17]);
                p[4] = p[13] + Vec3::new(0.0, -0.01, -0.02);
            }
        }
        p
    }
}

/// Folds the fingers with these knuckles: each finger's three joints fold
/// down in front of its knuckle.
fn fold(p: &mut [Vec3; 21], knuckles: &[usize]) {
    for &mcp in knuckles {
        for (k, by) in [(1, 0.015), (2, 0.025), (3, 0.03)] {
            p[mcp + k] = p[mcp] + Vec3::new(0.0, -by * 0.6, -by);
        }
    }
}

/// A hand to place in front of the camera.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    /// Palm center in the user's level frame: meters, x right, y up, z = distance from camera.
    pub at: Vec3,
    pub fingers: Fingers,
    /// Turn of the hand about its palm center, from palm-to-camera, fingers-up.
    pub rotation: Quat,
    /// Which hand the tracker reports this as.
    pub handedness: Handedness,
}

impl Pose {
    pub fn new(at: Vec3, fingers: Fingers) -> Self {
        Self {
            at,
            fingers,
            rotation: Quat::IDENTITY,
            handedness: Handedness::Right,
        }
    }

    pub fn open(at: Vec3) -> Self {
        Self::new(at, Fingers::Open)
    }

    pub fn pinched(at: Vec3) -> Self {
        Self::new(at, Fingers::Pinch)
    }

    pub fn gap(at: Vec3, gap: f32) -> Self {
        Self::new(at, Fingers::Gap(gap))
    }

    pub fn rotated(self, rotation: Quat) -> Self {
        Self { rotation, ..self }
    }

    pub fn left(self) -> Self {
        Self {
            handedness: Handedness::Left,
            ..self
        }
    }
}

/// The webcam the synthetic hands are seen through. Poses are given in the
/// user's level frame; the camera may be pitched up or down in it.
#[derive(Clone, Copy, Debug)]
pub struct TestCamera {
    pub hfov_deg: f32,
    /// Positive looks up.
    pub pitch_deg: f32,
}

impl Default for TestCamera {
    /// The camera terrain assumes when uncalibrated: level, default FOV.
    fn default() -> Self {
        Self {
            hfov_deg: CameraModel::default().hfov_deg,
            pitch_deg: 0.0,
        }
    }
}

impl TestCamera {
    /// The MediaPipe-shaped `Hand` a tracker would report for `pose`: world
    /// landmarks in meters (y down) along the camera's axes, image landmarks
    /// normalized (y down).
    pub fn hand(&self, pose: Pose) -> Hand {
        let local = pose.fingers.shape();
        let palm =
            landmark::PALM.map(|i| local[i]).iter().sum::<Vec3>() / landmark::PALM.len() as f32;
        // From the user's level frame into the camera's axes.
        let to_camera = Quat::from_rotation_x(self.pitch_deg.to_radians());
        let at = to_camera * pose.at;
        let local = local.map(|p| to_camera * (pose.rotation * (p - palm)));
        // Our own pinhole math, so terrain's unprojection is checked rather than reused.
        let half_w = (self.hfov_deg.to_radians() / 2.0).tan();
        let half_h = half_w / CameraModel::default().aspect;
        let image = local.map(|p| {
            let c = at + p;
            Vec3::new(
                0.5 + c.x / (2.0 * half_w * c.z),
                0.5 - c.y / (2.0 * half_h * c.z),
                p.z,
            )
        });
        Hand {
            handedness: pose.handedness,
            score: 1.0,
            image,
            world: local.map(|p| Vec3::new(p.x, -p.y, p.z)),
        }
    }

    /// One frame of `poses`, as this camera sees them.
    pub fn frame(&self, poses: &[Pose]) -> HandFrame {
        HandFrame {
            t_ms: 0,
            hands: poses.iter().map(|&p| self.hand(p)).collect(),
        }
    }

    /// `n` identical frames of `poses`: the hands held still.
    pub fn hold(&self, poses: &[Pose], n: usize) -> Vec<HandFrame> {
        vec![self.frame(poses); n]
    }
}

/// `pose` seen by the default camera.
pub fn hand(pose: Pose) -> Hand {
    TestCamera::default().hand(pose)
}
