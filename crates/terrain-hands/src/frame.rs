use glam::{Mat3, Quat, Vec3};
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

    /// World landmark `i` in camera axes: meters, x right, y up, z away from the camera.
    pub fn world_in_camera(&self, i: usize) -> Vec3 {
        let w = self.world[i];
        Vec3::new(w.x, -w.y, w.z)
    }

    /// Palm center relative to the world-landmark origin, in camera axes.
    pub fn palm_center_offset(&self) -> Vec3 {
        landmark::PALM
            .iter()
            .map(|&i| self.world_in_camera(i))
            .sum::<Vec3>()
            / landmark::PALM.len() as f32
    }

    /// Orientation of the palm in camera axes: y toward the fingers, z out of
    /// the palm's plane, built from the wrist and the index and pinky knuckles.
    pub fn palm_rotation(&self) -> Quat {
        let wrist = self.world_in_camera(landmark::WRIST);
        let index = self.world_in_camera(landmark::INDEX_MCP) - wrist;
        let pinky = self.world_in_camera(landmark::PINKY_MCP) - wrist;
        let fingers = (index + pinky).normalize();
        let normal = index.cross(pinky).normalize();
        let side = fingers.cross(normal);
        Quat::from_mat3(&Mat3::from_cols(side, fingers, normal))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Handedness {
    Left,
    Right,
}
