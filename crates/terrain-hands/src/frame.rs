use glam::{Mat3, Quat, Vec2, Vec3};
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
    pub const MIDDLE_TIP: usize = 12;
    pub const RING_TIP: usize = 16;
    pub const PINKY_TIP: usize = 20;

    /// Each finger's knuckle and tip, index to pinky.
    pub const FINGERS: [(usize, usize); 4] = [
        (INDEX_MCP, INDEX_TIP),
        (MIDDLE_MCP, MIDDLE_TIP),
        (RING_MCP, RING_TIP),
        (PINKY_MCP, PINKY_TIP),
    ];

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

/// One tracked hand, in MediaPipe's conventions. The tracker reports it
/// for the camera image as captured; `HandFrame`s, and everything the app
/// sees, carry it `mirrored` to the selfie view.
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
    /// The same hand seen in a mirror: image and world x flipped, and so
    /// the other handedness.
    pub fn mirrored(&self) -> Hand {
        Hand {
            handedness: match self.handedness {
                Handedness::Left => Handedness::Right,
                Handedness::Right => Handedness::Left,
            },
            score: self.score,
            image: self.image.map(|p| Vec3::new(1.0 - p.x, p.y, p.z)),
            world: self.world.map(|p| Vec3::new(-p.x, p.y, p.z)),
        }
    }

    /// Thumb-tip to index-tip distance in meters.
    pub fn pinch_gap(&self) -> f32 {
        self.world[landmark::THUMB_TIP].distance(self.world[landmark::INDEX_TIP])
    }

    /// How far the straightest finger reaches: its tip's distance from the
    /// wrist over its knuckle's. About 1.8 for a straight finger, under 1
    /// for one folded into the palm, so under about 1.1 means a fist.
    pub fn finger_reach(&self) -> f32 {
        let wrist = self.world[landmark::WRIST];
        landmark::FINGERS
            .iter()
            .map(|&(knuckle, tip)| {
                self.world[tip].distance(wrist) / self.world[knuckle].distance(wrist).max(1e-6)
            })
            .fold(0.0, f32::max)
    }

    /// World landmark `i` in camera axes: meters, x right, y up, z away from the camera.
    pub fn world_in_camera(&self, i: usize) -> Vec3 {
        let w = self.world[i];
        Vec3::new(w.x, -w.y, w.z)
    }

    /// Palm center in the image: the mean of the palm landmarks, normalized.
    pub fn palm_in_image(&self) -> Vec2 {
        landmark::PALM
            .iter()
            .map(|&i| self.image[i].truncate())
            .sum::<Vec2>()
            / landmark::PALM.len() as f32
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
    /// z points out of the back of one hand and the palm of the other, which
    /// doesn't matter: only changes in a hand's rotation are used. `None` if
    /// the knuckles are collinear with the wrist.
    pub fn palm_rotation(&self) -> Option<Quat> {
        let wrist = self.world_in_camera(landmark::WRIST);
        let index = self.world_in_camera(landmark::INDEX_MCP) - wrist;
        let pinky = self.world_in_camera(landmark::PINKY_MCP) - wrist;
        let fingers = (index + pinky).try_normalize()?;
        let normal = index.cross(pinky).try_normalize()?;
        let side = fingers.cross(normal);
        Some(Quat::from_mat3(&Mat3::from_cols(side, fingers, normal)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Handedness {
    Left,
    Right,
}
