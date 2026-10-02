use glam::Vec3;
use serde::{Deserialize, Serialize};

/// MediaPipe hand landmark indices used by terrain.
pub mod landmark {
    pub const WRIST: usize = 0;
    pub const THUMB_TIP: usize = 4;
    pub const INDEX_MCP: usize = 5;
    pub const INDEX_TIP: usize = 8;
    pub const MIDDLE_MCP: usize = 9;
    pub const RING_MCP: usize = 13;
    pub const PINKY_MCP: usize = 17;

    /// Landmarks that stay put while the fingers move: their mean is the palm center.
    pub const PALM: [usize; 5] = [WRIST, INDEX_MCP, MIDDLE_MCP, RING_MCP, PINKY_MCP];
}

/// Everything the tracker saw in one camera frame.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HandFrame {
    /// Capture time in milliseconds since the source started.
    pub t_ms: u64,
    /// Zero, one or two hands.
    pub hands: Vec<Hand>,
}

/// One tracked hand, in MediaPipe's conventions, in the mirrored (selfie) view.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hand {
    pub handedness: Handedness,
    pub score: f32,
    /// Normalized image coordinates: x right, y down, in [0, 1]; z relative depth.
    pub image: [Vec3; 21],
    /// Meters, origin at the hand's geometric center: x right, y down, z away from the camera.
    pub world: [Vec3; 21],
}

impl Hand {
    /// Thumb-tip to index-tip distance in meters.
    pub fn pinch_gap(&self) -> f32 {
        self.world[landmark::THUMB_TIP].distance(self.world[landmark::INDEX_TIP])
    }

    /// Mean image position of the palm landmarks.
    pub fn palm_image_center(&self) -> Vec3 {
        landmark::PALM.iter().map(|&i| self.image[i]).sum::<Vec3>() / landmark::PALM.len() as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Handedness {
    Left,
    Right,
}
