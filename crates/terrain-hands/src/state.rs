use glam::{Mat3, Quat, Vec3};

use crate::{Hand, HandFrame};

/// Pinch closes below this thumb-tip to index-tip distance (meters)...
const PINCH_CLOSE_M: f32 = 0.03;
/// ...and opens above this one. The gap between them stops flicker.
const PINCH_OPEN_M: f32 = 0.05;

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
    /// x right, y up, z = distance from the camera).
    ///
    /// World landmarks give the hand's true shape in meters, already aligned
    /// with the camera's axes; image landmarks say where each one lands on
    /// screen. For a pinhole camera every landmark then gives two equations
    /// that are linear in the unknown origin, so a least-squares solve over
    /// all 21 recovers position and depth together, exact under perspective.
    fn locate(&self, hand: &Hand) -> Vec3 {
        let a = 2.0 * (self.hfov_deg.to_radians() / 2.0).tan();
        let b = a / self.aspect;
        let mut normal = Mat3::ZERO;
        let mut rhs = Vec3::ZERO;
        for i in 0..21 {
            let w = hand.world_in_camera(i);
            let du = hand.image[i].x - 0.5;
            let dv = hand.image[i].y - 0.5;
            // u: x / (a z) = du    v: -y / (b z) = dv
            for (row, value) in [
                (Vec3::new(1.0, 0.0, -a * du), a * du * w.z - w.x),
                (Vec3::new(0.0, -1.0, -b * dv), b * dv * w.z + w.y),
            ] {
                normal += Mat3::from_cols(row * row.x, row * row.y, row * row.z);
                rhs += row * value;
            }
        }
        normal.inverse() * rhs
    }
}

/// What a hand is doing, in camera space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandState {
    /// Palm center: meters, x right, y up, z = distance from the camera.
    pub position: Vec3,
    /// Palm orientation in camera space; only changes in it are meaningful.
    pub rotation: Quat,
    pub pinching: bool,
}

/// Turns `HandFrame`s into `HandState`s, remembering what it needs between frames.
#[derive(Default)]
pub struct HandStateEstimator {
    camera: CameraModel,
    pinching: bool,
}

impl HandStateEstimator {
    pub fn new(camera: CameraModel) -> Self {
        Self {
            camera,
            pinching: false,
        }
    }

    /// The state of the first hand in `frame`, if any.
    pub fn update(&mut self, frame: &HandFrame) -> Option<HandState> {
        let Some(hand) = frame.hands.first() else {
            self.pinching = false;
            return None;
        };
        self.pinching = self.next_pinch(hand);
        Some(HandState {
            position: self.camera.locate(hand) + hand.palm_center_offset(),
            rotation: hand.palm_rotation(),
            pinching: self.pinching,
        })
    }

    fn next_pinch(&self, hand: &Hand) -> bool {
        let gap = hand.pinch_gap();
        if self.pinching {
            gap <= PINCH_OPEN_M
        } else {
            gap < PINCH_CLOSE_M
        }
    }
}
