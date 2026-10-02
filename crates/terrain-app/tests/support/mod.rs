//! Builds synthetic `HandFrame`s by placing a canonical hand in front of a
//! pinhole camera, so tests can speak in real-world meters.

#![allow(dead_code)]

use bevy::prelude::*;
use terrain_app::{GrabPlugin, Grabbable};
use terrain_hands::{CameraModel, Hand, HandFrame, HandSource, Handedness, ReplaySource};

/// Thumb-to-index gap of a firmly closed pinch, in meters.
pub const PINCHED_GAP: f32 = 0.01;

/// An open right hand, palm to the camera, fingers up. Meters, x right, y up,
/// z away from the camera. `hand` re-centers it on the palm.
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

/// A hand to place in the scene.
#[derive(Clone, Copy)]
pub struct Pose {
    /// Palm center in camera space: meters, x right, y up, z = distance from camera.
    pub at: Vec3,
    /// Thumb-tip to index-tip distance in meters; `None` leaves the hand open.
    pub pinch_gap: Option<f32>,
    /// Turn of the hand about its palm center, from palm-to-camera, fingers-up.
    pub rotation: Quat,
}

impl Pose {
    pub fn open(at: Vec3) -> Self {
        Self {
            at,
            pinch_gap: None,
            rotation: Quat::IDENTITY,
        }
    }

    pub fn pinched(at: Vec3) -> Self {
        Self::gap(at, PINCHED_GAP)
    }

    pub fn gap(at: Vec3, gap: f32) -> Self {
        Self {
            at,
            pinch_gap: Some(gap),
            rotation: Quat::IDENTITY,
        }
    }

    pub fn rotated(self, rotation: Quat) -> Self {
        Self { rotation, ..self }
    }
}

/// Builds the MediaPipe-shaped `Hand` a tracker would report for `pose`:
/// world landmarks in meters (y down), image landmarks normalized (y down).
pub fn hand(pose: Pose) -> Hand {
    let mut local = OPEN_HAND.map(Vec3::from_array);
    if let Some(gap) = pose.pinch_gap {
        local[4] = local[8] + Vec3::new(-gap, 0.0, 0.0);
    }
    let palm = [0, 5, 9, 13, 17].map(|i| local[i]).iter().sum::<Vec3>() / 5.0;
    let local = local.map(|p| pose.rotation * (p - palm));
    // Project with our own pinhole math, using the app's default camera values,
    // so the app's unprojection is checked rather than reused.
    let camera = CameraModel::default();
    let half_w = (camera.hfov_deg.to_radians() / 2.0).tan();
    let half_h = half_w / camera.aspect;
    let image = local.map(|p| {
        let c = pose.at + p;
        Vec3::new(
            0.5 + c.x / (2.0 * half_w * c.z),
            0.5 - c.y / (2.0 * half_h * c.z),
            p.z,
        )
    });
    let world = local.map(|p| Vec3::new(p.x, -p.y, p.z));
    Hand {
        handedness: Handedness::Right,
        score: 1.0,
        image,
        world,
    }
}

pub fn frame(poses: &[Pose]) -> HandFrame {
    HandFrame {
        t_ms: 0,
        hands: poses.iter().copied().map(hand).collect(),
    }
}

/// A headless app with one cube at the origin, fed `frames` one per update.
pub struct Harness {
    pub app: App,
    pub cube: Entity,
}

impl Harness {
    pub fn new(frames: Vec<HandFrame>) -> Self {
        Self::with_source(ReplaySource::new(frames))
    }

    pub fn with_source(source: impl HandSource + 'static) -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(GrabPlugin::new(source));
        let cube = app
            .world_mut()
            .spawn((Transform::default(), Grabbable))
            .id();
        Self { app, cube }
    }

    pub fn run(&mut self, updates: usize) -> &mut Self {
        for _ in 0..updates {
            self.app.update();
        }
        self
    }

    pub fn cube(&self) -> Transform {
        *self.app.world().get::<Transform>(self.cube).unwrap()
    }
}

pub fn assert_near(actual: Vec3, expected: Vec3) {
    assert!(
        actual.abs_diff_eq(expected, 1e-3),
        "expected {expected}, got {actual}"
    );
}

pub fn assert_turned(actual: Quat, expected: Quat) {
    let off = actual.angle_between(expected).to_degrees();
    assert!(
        off < 0.5,
        "expected {expected}, got {actual} ({off:.2}° off)"
    );
}
