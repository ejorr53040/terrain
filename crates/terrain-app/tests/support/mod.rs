//! Builds synthetic `HandFrame`s by placing a canonical hand in front of a
//! pinhole camera, so tests can speak in real-world meters.

#![allow(dead_code)]

use std::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};
use terrain_app::{GrabPlugin, Grabbable};
use terrain_hands::{CameraModel, Hand, HandFrame, HandSource, Handedness, ReplaySource, landmark};

/// Time between camera frames, and between app updates, in tests.
pub const FRAME_MS: u64 = 33;

/// Frames to hold a pose for the smoothed hand to settle on it.
pub const SETTLE: usize = 30;

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
    /// Which hand the tracker reports this as.
    pub handedness: Handedness,
}

impl Pose {
    pub fn open(at: Vec3) -> Self {
        Self {
            at,
            pinch_gap: None,
            rotation: Quat::IDENTITY,
            handedness: Handedness::Right,
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
            handedness: Handedness::Right,
        }
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

/// Builds the MediaPipe-shaped `Hand` a tracker would report for `pose`:
/// world landmarks in meters (y down), image landmarks normalized (y down).
pub fn hand(pose: Pose) -> Hand {
    let mut local = OPEN_HAND.map(Vec3::from_array);
    if let Some(gap) = pose.pinch_gap {
        local[4] = local[8] + Vec3::new(-gap, 0.0, 0.0);
    }
    let palm = landmark::PALM.map(|i| local[i]).iter().sum::<Vec3>() / landmark::PALM.len() as f32;
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
        handedness: pose.handedness,
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

/// `n` identical frames of `poses`: the hand held still.
pub fn hold(poses: &[Pose], n: usize) -> Vec<HandFrame> {
    vec![frame(poses); n]
}

/// A frame where the tracker sees no hands.
pub fn no_hands() -> HandFrame {
    frame(&[])
}

/// A headless app with one cube at the origin. Each update advances the
/// clock by `FRAME_MS`.
pub struct Harness {
    pub app: App,
    pub cube: Entity,
    /// Frames given up front, for `run_all`; 0 for `with_source`.
    frame_count: usize,
}

impl Harness {
    /// Plays `frames` one per update (re-stamped `FRAME_MS` apart).
    pub fn new(frames: Vec<HandFrame>) -> Self {
        let frames: Vec<HandFrame> = frames
            .into_iter()
            .enumerate()
            .map(|(i, f)| HandFrame {
                t_ms: i as u64 * FRAME_MS,
                ..f
            })
            .collect();
        Self::timed(frames)
    }

    /// Plays `frames` at their own `t_ms` timestamps.
    pub fn timed(frames: Vec<HandFrame>) -> Self {
        Self {
            frame_count: frames.len(),
            ..Self::with_source(ReplaySource::new(frames))
        }
    }

    pub fn with_source(source: impl HandSource + 'static) -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                FRAME_MS,
            )))
            .add_plugins(GrabPlugin::new(source));
        let cube = app
            .world_mut()
            .spawn((Transform::default(), Grabbable))
            .id();
        Self {
            app,
            cube,
            frame_count: 0,
        }
    }

    /// Plays every frame given to `new` or `timed`.
    pub fn run_all(&mut self) -> &mut Self {
        self.run(self.frame_count)
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

/// Small deterministic noise in [-1, 1], so jitter tests are repeatable.
pub struct Noise(u64);

impl Noise {
    pub fn new() -> Self {
        Self(0x9e37_79b9_7f4a_7c15)
    }

    pub fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }

    pub fn vec3(&mut self, amplitude: f32) -> Vec3 {
        Vec3::new(self.next(), self.next(), self.next()) * amplitude
    }
}

/// Root-mean-square distance of `samples` from their mean.
pub fn rms_spread(samples: &[Vec3]) -> f32 {
    let mean = samples.iter().sum::<Vec3>() / samples.len() as f32;
    (samples
        .iter()
        .map(|s| s.distance_squared(mean))
        .sum::<f32>()
        / samples.len() as f32)
        .sqrt()
}

pub fn assert_turned(actual: Quat, expected: Quat) {
    let off = actual.angle_between(expected).to_degrees();
    assert!(
        off < 0.5,
        "expected {expected}, got {actual} ({off:.2}° off)"
    );
}
