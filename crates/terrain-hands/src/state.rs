use glam::Vec3;

use crate::{Hand, HandFrame, landmark};

/// Pinch closes below this thumb-tip to index-tip distance (meters)...
const PINCH_CLOSE_M: f32 = 0.03;
/// ...and opens above this one. The gap between them stops flicker.
const PINCH_OPEN_M: f32 = 0.05;

/// Hand distance from the camera until depth is estimated (ticket 3).
const ASSUMED_DEPTH_M: f32 = 0.5;

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
    /// Camera-space point (meters; x right, y up, z = distance) for a
    /// normalized image point seen at `depth`.
    fn unproject(&self, image: Vec3, depth: f32) -> Vec3 {
        let half_w = (self.hfov_deg.to_radians() / 2.0).tan();
        let half_h = half_w / self.aspect;
        Vec3::new(
            (image.x - 0.5) * 2.0 * half_w * depth,
            -(image.y - 0.5) * 2.0 * half_h * depth,
            depth,
        )
    }
}

/// What a hand is doing, in camera space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandState {
    /// Palm center: meters, x right, y up, z = distance from the camera.
    pub position: Vec3,
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
            position: self.palm_center(hand),
            pinching: self.pinching,
        })
    }

    fn next_pinch(&self, hand: &Hand) -> bool {
        let gap = hand.world[landmark::THUMB_TIP].distance(hand.world[landmark::INDEX_TIP]);
        if self.pinching {
            gap <= PINCH_OPEN_M
        } else {
            gap < PINCH_CLOSE_M
        }
    }

    fn palm_center(&self, hand: &Hand) -> Vec3 {
        let image = landmark::PALM.iter().map(|&i| hand.image[i]).sum::<Vec3>()
            / landmark::PALM.len() as f32;
        self.camera.unproject(image, ASSUMED_DEPTH_M)
    }
}
