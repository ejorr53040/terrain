use std::fmt;

use glam::{Mat3, Quat, Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::{
    Hand, HandFrame, Handedness, landmark,
    smoothing::{OneEuro, QuatFilter, Vec3Filter},
};

/// Pinch closes below this thumb-tip to index-tip distance (meters)...
const PINCH_CLOSE_M: f32 = 0.04;
/// ...and opens above this one. The gap between them stops flicker. Measured
/// on a webcam: a firm pinch reads 1–3 cm (3–4 cm beside the other hand,
/// its thumb partly hidden), a relaxed hold up to about 5 cm, an open hand
/// about 9 cm.
const PINCH_OPEN_M: f32 = 0.065;
/// A pinch opens only after reading open on this many frames in a row, so a
/// single blurred frame doesn't drop what the hand is holding.
const OPEN_FRAMES: u32 = 2;

/// A fist closes once every finger's reach (`Hand::finger_reach`) is below
/// this...
const FIST_CLOSE_REACH: f32 = 1.1;
/// ...and opens once any finger reaches past this one. Measured on webcam
/// clips: a deliberate pinch keeps a finger above 1.1 in 99% of frames, a
/// loose fist mostly reads 0.8–1.0, a straight finger about 1.8.
const FIST_OPEN_REACH: f32 = 1.25;

/// Calibration needs the hand in at least this many frames...
const CALIBRATION_MIN_FRAMES: usize = 10;
/// ...held this still: RMS distance of the palm from its mean (meters).
const CALIBRATION_MAX_SPREAD_M: f32 = 0.03;
/// Webcam fields of view a calibration may find (degrees). Outside this,
/// the hand wasn't at the distance calibration assumed.
const CALIBRATION_FOV_RANGE: std::ops::RangeInclusive<f32> = 40.0..=110.0;
/// Refits of the field of view. Each one rescales depth; the palm's small
/// metric offset from the landmark origin makes it not quite exact, and
/// three rounds settle it well under a millimeter.
const CALIBRATION_FIT_ROUNDS: usize = 3;

/// Pinhole model of the webcam, and how it sits relative to the user.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraModel {
    pub hfov_deg: f32,
    /// Width over height.
    pub aspect: f32,
    /// Turns the camera's axes into the user's: level, y up. A webcam
    /// pitched up at the user otherwise reads raising a hand as partly
    /// moving it away.
    #[serde(default)]
    pub tilt: Quat,
}

impl Default for CameraModel {
    fn default() -> Self {
        Self {
            hfov_deg: 65.0,
            aspect: 16.0 / 9.0,
            tilt: Quat::IDENTITY,
        }
    }
}

/// Why calibration couldn't use what it saw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalibrationError {
    /// Too few frames had a usable hand.
    NoHand,
    /// The hand wasn't held still.
    Moving,
    /// The fit needs an implausible lens: the hand wasn't at the distance.
    WrongDistance,
}

impl fmt::Display for CalibrationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Self::NoHand => "no hand seen",
            Self::Moving => "the hand moved; hold it still",
            Self::WrongDistance => "the hand was too near or too far",
        })
    }
}

impl CameraModel {
    /// Width and height of the visible image plane one meter from the
    /// camera (meters).
    pub fn view_size_at_1m(&self) -> Vec2 {
        let width = 2.0 * (self.hfov_deg.to_radians() / 2.0).tan();
        Vec2::new(width, width / self.aspect)
    }

    /// Whether this could be a real webcam: a field of view webcams have and
    /// a usable tilt. Saved calibrations are checked with it when loaded.
    pub fn plausible(&self) -> bool {
        CALIBRATION_FOV_RANGE.contains(&self.hfov_deg)
            && self.aspect.is_finite()
            && self.aspect > 0.0
            && self.tilt.is_finite()
            && self.tilt.is_normalized()
    }

    /// This camera refitted from `hands`, one upright hand (fingers straight
    /// up, palm to the camera) held still `distance_m` from the lens: the
    /// field of view that puts the palm at that distance, and the tilt that
    /// turns the hand's wrist-to-knuckles line straight up.
    pub fn calibrated(&self, hands: &[Hand], distance_m: f32) -> Result<Self, CalibrationError> {
        if hands.len() < CALIBRATION_MIN_FRAMES {
            return Err(CalibrationError::NoHand);
        }
        let palms_at = |camera: &Self| -> Vec<Vec3> {
            hands
                .iter()
                .filter_map(|h| Some(camera.locate(h)? + h.palm_center_offset()))
                .collect()
        };
        let palms = palms_at(self);
        if palms.len() < CALIBRATION_MIN_FRAMES {
            return Err(CalibrationError::NoHand);
        }
        let mean = palms.iter().sum::<Vec3>() / palms.len() as f32;
        let spread = (palms.iter().map(|p| p.distance_squared(mean)).sum::<f32>()
            / palms.len() as f32)
            .sqrt();
        if spread > CALIBRATION_MAX_SPREAD_M {
            return Err(CalibrationError::Moving);
        }
        // Depth scales inversely with the image plane's width at 1 m, so
        // rescale it until the palm reads at `distance_m`.
        let mut camera = *self;
        for _ in 0..CALIBRATION_FIT_ROUNDS {
            let palms = palms_at(&camera);
            let depth = palms.iter().map(|p| p.z).sum::<f32>() / palms.len() as f32;
            let width_at_1m = 2.0 * (camera.hfov_deg.to_radians() / 2.0).tan() * depth / distance_m;
            camera.hfov_deg = (2.0 * (width_at_1m / 2.0).atan()).to_degrees();
        }
        if !camera.plausible() {
            return Err(CalibrationError::WrongDistance);
        }
        let up = hands
            .iter()
            .map(|h| {
                (h.world_in_camera(landmark::MIDDLE_MCP) - h.world_in_camera(landmark::WRIST))
                    .normalize_or_zero()
            })
            .sum::<Vec3>()
            .normalize_or(Vec3::Y);
        camera.tilt = Quat::from_rotation_arc(up, Vec3::Y);
        Ok(camera)
    }

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
        let Vec2 {
            x: width_at_1m,
            y: height_at_1m,
        } = self.view_size_at_1m();
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

/// What a hand is doing, smoothed, in the camera's space turned level by
/// its `tilt`: meters, x right, y straight up, z level and away from the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandState {
    /// Tells this hand apart from others from frame to frame, for as long
    /// as it stays tracked. The tracker's handedness label can't: it often
    /// gives two hands the same one.
    pub id: u32,
    /// Which hand the tracker thinks this is.
    pub handedness: Handedness,
    /// Palm center, meters.
    pub position: Vec3,
    /// Palm center in the image as the frame gave it (mirrored for a live
    /// source), normalized: where the hand is on screen.
    pub in_image: Vec2,
    /// Palm orientation; only changes in it are meaningful.
    pub rotation: Quat,
    pub pinching: bool,
    /// Every finger is folded into the palm. A fist's thumb often rests on
    /// the index finger, so it may read as `pinching` too.
    pub fist: bool,
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

/// A hand in a new frame continues a track only if its palm moved less than
/// this in the image (image widths) since the track last saw it: room for a
/// fast flick (at arm's length, about 25 cm in a frame), while a hand that
/// appears across the view, just as another left, starts afresh rather
/// than taking over its grab.
const SAME_HAND_MAX_MOVE: f32 = 0.4;

/// What's remembered about one hand between frames.
struct Track {
    id: u32,
    /// Palm center in the image last time it was seen, normalized.
    palm_in_image: Vec2,
    last_seen_ms: u64,
    pinching: bool,
    fist: bool,
    /// Frames in a row a held pinch has read open.
    open_frames: u32,
    /// x and y, with z held at 0...
    across: Vec3Filter,
    /// ...and z alone.
    depth: Vec3Filter,
    rotation: QuatFilter,
}

impl Track {
    fn new(id: u32, t_ms: u64) -> Self {
        Self {
            id,
            palm_in_image: Vec2::ZERO,
            last_seen_ms: t_ms,
            pinching: false,
            fist: false,
            open_frames: 0,
            across: Vec3Filter::new(POSITION_SMOOTHING),
            depth: Vec3Filter::new(DEPTH_SMOOTHING),
            rotation: QuatFilter::new(ROTATION_SMOOTHING),
        }
    }

    fn smoothing_starts_over(&mut self) {
        self.across = Vec3Filter::new(POSITION_SMOOTHING);
        self.depth = Vec3Filter::new(DEPTH_SMOOTHING);
        self.rotation = QuatFilter::new(ROTATION_SMOOTHING);
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
    tracks: Vec<Track>,
    next_id: u32,
}

impl HandStateEstimator {
    pub fn new(camera: CameraModel) -> Self {
        Self {
            camera,
            tracks: Vec::new(),
            next_id: 0,
        }
    }

    /// The camera hands are placed with.
    pub fn camera(&self) -> CameraModel {
        self.camera
    }

    /// Uses `camera` from the next frame on. Hands' smoothing starts over,
    /// rather than gliding from where the old camera placed them; which hand
    /// is which, and whether it's pinching, carry on.
    pub fn set_camera(&mut self, camera: CameraModel) {
        self.camera = camera;
        for track in &mut self.tracks {
            track.smoothing_starts_over();
        }
    }

    /// The smoothed state of every usable hand in `frame`. Hands unseen for
    /// longer than `FORGET_AFTER_MS` start over.
    pub fn update(&mut self, frame: &HandFrame) -> TrackedHands {
        let t_ms = frame.t_ms;
        self.tracks
            .retain(|track| t_ms.saturating_sub(track.last_seen_ms) <= FORGET_AFTER_MS);
        // Pair hands with tracks by how far their palms moved in the image,
        // closest pairs first. A hand left over starts a new track.
        let palms: Vec<Vec2> = frame.hands.iter().map(Hand::palm_in_image).collect();
        let mut pairs: Vec<(f32, usize, usize)> = palms
            .iter()
            .enumerate()
            .flat_map(|(h, palm)| {
                self.tracks
                    .iter()
                    .enumerate()
                    .map(move |(t, track)| (palm.distance(track.palm_in_image), h, t))
            })
            .filter(|&(moved, ..)| moved <= SAME_HAND_MAX_MOVE)
            .collect();
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut track_of: Vec<Option<usize>> = vec![None; frame.hands.len()];
        let mut taken = vec![false; self.tracks.len()];
        for (_, h, t) in pairs {
            if track_of[h].is_none() && !taken[t] {
                track_of[h] = Some(t);
                taken[t] = true;
            }
        }
        let hands = frame
            .hands
            .iter()
            .zip(track_of)
            .zip(palms)
            .filter_map(|((hand, t), palm)| {
                let t = t.unwrap_or_else(|| {
                    self.tracks.push(Track::new(self.next_id, t_ms));
                    self.next_id += 1;
                    self.tracks.len() - 1
                });
                self.tracks[t].palm_in_image = palm;
                self.track(hand, t, t_ms)
            })
            .collect();
        TrackedHands { t_ms, hands }
    }

    fn track(&mut self, hand: &Hand, t: usize, t_ms: u64) -> Option<HandState> {
        // Degenerate landmarks: skip the hand rather than feed NaNs on.
        let origin = self.camera.locate(hand)?;
        let rotation = hand.palm_rotation()?;
        let track = &mut self.tracks[t];
        let dt = t_ms.saturating_sub(track.last_seen_ms) as f32 / 1000.0;
        track.last_seen_ms = t_ms;
        let was_pinching = track.pinching;
        track.pinching = track.next_pinch(hand);
        let reach = hand.finger_reach();
        track.fist = if track.fist {
            reach < FIST_OPEN_REACH
        } else {
            reach < FIST_CLOSE_REACH
        };
        let palm = self.camera.tilt * (origin + hand.palm_center_offset());
        let rotation = self.camera.tilt * rotation;
        let across = track.across.filter(palm.with_z(0.0), dt);
        let depth = track.depth.filter(Vec3::Z * palm.z, dt);
        Some(HandState {
            id: track.id,
            in_image: hand.palm_in_image(),
            handedness: hand.handedness,
            position: across.with_z(depth.z),
            rotation: track.rotation.filter(rotation, dt),
            pinching: track.pinching,
            fist: track.fist,
            pinch_started: track.pinching && !was_pinching,
        })
    }
}
