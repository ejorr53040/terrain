use std::collections::HashMap;

use glam::{Mat3, Quat, Vec3};

use crate::{
    Hand, HandFrame, Handedness,
    smoothing::{OneEuro, QuatFilter, Vec3Filter},
};

/// Pinch closes below this thumb-tip to index-tip distance (meters)...
const PINCH_CLOSE_M: f32 = 0.03;
/// ...and opens above this one. The gap between them stops flicker. Measured
/// on a webcam: a firm pinch reads 1–3 cm, a relaxed hold up to about 5 cm,
/// an open hand about 9 cm.
const PINCH_OPEN_M: f32 = 0.065;
/// A pinch opens only after reading open on this many frames in a row, so a
/// single blurred frame doesn't drop what the hand is holding.
const OPEN_FRAMES: u32 = 2;

/// Pinhole model of the webcam.
#[derive(Clone, Copy, Debug)]
pub struct CameraModel {
    pub hfov_deg: f32,
    /// Width over height.
    pub aspect: f32,
}

impl Default for CameraModel {
    fn default() -> Self {
        Self {
            hfov_deg: 65.0,
            aspect: 16.0 / 9.0,
        }
    }
}

impl CameraModel {
    /// Where the hand's world-landmark origin sits in camera space (meters;
    /// x right, y up, z = distance from the camera), or `None` if the
    /// landmarks are degenerate.
    ///
    /// World landmarks give the hand's true shape in meters, already aligned
    /// with the camera's axes; image landmarks say where each one lands on
    /// screen. For a pinhole camera every landmark then gives two equations
    /// that are linear in the unknown origin, so a least-squares solve over
    /// all 21 recovers position and depth together, exact under perspective.
    fn locate(&self, hand: &Hand) -> Option<Vec3> {
        // Size of the visible image plane one meter from the camera.
        let width_at_1m = 2.0 * (self.hfov_deg.to_radians() / 2.0).tan();
        let height_at_1m = width_at_1m / self.aspect;
        // Accumulate the normal equations AᵀA·o = Aᵀb for the origin o.
        let mut normal = Mat3::ZERO;
        let mut rhs = Vec3::ZERO;
        for i in 0..hand.image.len() {
            let w = hand.world_in_camera(i);
            let du = hand.image[i].x - 0.5;
            let dv = hand.image[i].y - 0.5;
            // Landmark i sits at o + w, so the pinhole gives
            //   o.x + w.x = width_at_1m·du·(o.z + w.z)
            //   o.y + w.y = -height_at_1m·dv·(o.z + w.z)
            // rearranged into rows of A (acting on o) and entries of b:
            for (row, value) in [
                (
                    Vec3::new(1.0, 0.0, -width_at_1m * du),
                    width_at_1m * du * w.z - w.x,
                ),
                (
                    Vec3::new(0.0, -1.0, -height_at_1m * dv),
                    height_at_1m * dv * w.z + w.y,
                ),
            ] {
                normal += Mat3::from_cols(row * row.x, row * row.y, row * row.z);
                rhs += row * value;
            }
        }
        (normal.determinant().abs() > 1e-9).then(|| normal.inverse() * rhs)
    }
}

/// What a hand is doing, in camera space, smoothed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandState {
    /// Which hand; also how a hand is told apart from frame to frame.
    pub handedness: Handedness,
    /// Palm center: meters, x right, y up, z = distance from the camera.
    pub position: Vec3,
    /// Palm orientation in camera space; only changes in it are meaningful.
    pub rotation: Quat,
    pub pinching: bool,
    /// The pinch closed in this frame: it was open in the hand's last one.
    pub pinch_started: bool,
}

/// The hands seen in one `HandFrame`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackedHands {
    /// The frame's capture time.
    pub t_ms: u64,
    pub hands: Vec<HandState>,
}

/// A hand unseen for longer than this starts over: its smoothing and pinch
/// state are dropped rather than gliding from where it was last seen.
pub const FORGET_AFTER_MS: u64 = 300;

/// Smoothing for palm position across the image (x and y, meters).
const POSITION_SMOOTHING: OneEuro = OneEuro {
    min_cutoff: 1.0,
    beta: 20.0,
    d_cutoff: 1.0,
};

/// Smoothing for palm depth (z, meters). A webcam judges depth several
/// times less steadily than x and y, so it's smoothed harder, and its noise
/// is kept from loosening the x and y smoothing.
const DEPTH_SMOOTHING: OneEuro = OneEuro {
    min_cutoff: 0.5,
    beta: 3.0,
    d_cutoff: 1.0,
};

/// Smoothing for palm rotation (radians).
const ROTATION_SMOOTHING: OneEuro = OneEuro {
    min_cutoff: 1.0,
    beta: 0.5,
    d_cutoff: 1.0,
};

/// What's remembered about one hand between frames.
struct Track {
    last_seen_ms: u64,
    pinching: bool,
    /// Frames in a row a held pinch has read open.
    open_frames: u32,
    /// x and y, with z held at 0...
    across: Vec3Filter,
    /// ...and z alone.
    depth: Vec3Filter,
    rotation: QuatFilter,
}

impl Track {
    fn new(t_ms: u64) -> Self {
        Self {
            last_seen_ms: t_ms,
            pinching: false,
            open_frames: 0,
            across: Vec3Filter::new(POSITION_SMOOTHING),
            depth: Vec3Filter::new(DEPTH_SMOOTHING),
            rotation: QuatFilter::new(ROTATION_SMOOTHING),
        }
    }

    fn next_pinch(&mut self, hand: &Hand) -> bool {
        let gap = hand.pinch_gap();
        if !self.pinching {
            return gap < PINCH_CLOSE_M;
        }
        self.open_frames = if gap > PINCH_OPEN_M {
            self.open_frames + 1
        } else {
            0
        };
        if self.open_frames < OPEN_FRAMES {
            return true;
        }
        self.open_frames = 0;
        false
    }
}

/// Turns `HandFrame`s into smoothed `HandState`s, one track per hand.
#[derive(Default)]
pub struct HandStateEstimator {
    camera: CameraModel,
    tracks: HashMap<Handedness, Track>,
}

impl HandStateEstimator {
    pub fn new(camera: CameraModel) -> Self {
        Self {
            camera,
            tracks: HashMap::new(),
        }
    }

    /// The smoothed state of every usable hand in `frame`. Hands unseen for
    /// longer than `FORGET_AFTER_MS` start over.
    pub fn update(&mut self, frame: &HandFrame) -> TrackedHands {
        let t_ms = frame.t_ms;
        self.tracks
            .retain(|_, track| t_ms.saturating_sub(track.last_seen_ms) <= FORGET_AFTER_MS);
        let hands = frame
            .hands
            .iter()
            .filter_map(|hand| self.track(hand, t_ms))
            .collect();
        TrackedHands { t_ms, hands }
    }

    fn track(&mut self, hand: &Hand, t_ms: u64) -> Option<HandState> {
        // Degenerate landmarks: skip the hand rather than feed NaNs on.
        let origin = self.camera.locate(hand)?;
        let rotation = hand.palm_rotation()?;
        let track = self
            .tracks
            .entry(hand.handedness)
            .or_insert_with(|| Track::new(t_ms));
        let dt = t_ms.saturating_sub(track.last_seen_ms) as f32 / 1000.0;
        track.last_seen_ms = t_ms;
        let was_pinching = track.pinching;
        track.pinching = track.next_pinch(hand);
        let palm = origin + hand.palm_center_offset();
        let across = track.across.filter(palm.with_z(0.0), dt);
        let depth = track.depth.filter(Vec3::Z * palm.z, dt);
        Some(HandState {
            handedness: hand.handedness,
            position: across.with_z(depth.z),
            rotation: track.rotation.filter(rotation, dt),
            pinching: track.pinching,
            pinch_started: track.pinching && !was_pinching,
        })
    }
}
