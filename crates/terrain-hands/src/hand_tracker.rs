//! The hand tracker: palm detection to find hands, the landmark model to
//! follow them, each frame's landmarks giving the next frame's regions.
//! Follows MediaPipe's `hand_landmark_tracking_cpu` graph.

use std::path::Path;

use glam::Vec2;

use crate::{Hand, HandRegion, Handedness, LandmarkModel, PalmDetector, RgbaImage, landmark};

/// Hands tracked at once.
const MAX_HANDS: usize = 2;
/// While following a hand, look for another only every this many frames:
/// the palm detector costs more than following a hand, and a second hand
/// can wait a tenth of a second to be found.
const SEARCH_EVERY: u32 = 3;
/// A new palm overlapping a tracked hand's region more than this (IoU) is
/// that same hand.
const SAME_HAND_IOU: f32 = 0.5;
/// The next frame's region is this much bigger than the landmarks' box...
const REGION_SCALE: f32 = 2.0;
/// ...and moved this far (in box heights) toward the fingers.
const REGION_SHIFT: f32 = 0.1;
/// Each frame's handedness label moves a followed hand's vote this far
/// toward it. A hand keeps its label through brief misreads: it takes four
/// frames in a row reading as the other hand to change it.
const LABEL_WEIGHT: f32 = 0.2;

/// A hand and the region it was found in.
#[derive(Clone, Debug)]
pub struct TrackedHand {
    /// As MediaPipe reports it on the unmirrored image. MediaPipe labels
    /// handedness assuming a mirrored image, so on an unmirrored one the
    /// label is the opposite hand.
    pub hand: Hand,
    /// The region the landmarks were found in (for drawing it).
    pub region: HandRegion,
}

/// Finds and follows up to two hands through a stream of images.
pub struct HandTracker {
    palms: PalmDetector,
    landmarks: LandmarkModel,
    /// Where each tracked hand should be in the next image, and its
    /// handedness vote.
    regions: Vec<(HandRegion, LabelVote)>,
    /// Images since the palm detector last ran.
    since_search: u32,
}

impl HandTracker {
    pub fn new(
        palm_model: impl AsRef<Path>,
        landmark_model: impl AsRef<Path>,
    ) -> ort::Result<Self> {
        Ok(Self {
            palms: PalmDetector::new(palm_model)?,
            landmarks: LandmarkModel::new(landmark_model)?,
            regions: Vec::new(),
            since_search: 0,
        })
    }

    /// The hands in `image`, the next image in the stream.
    pub fn track(&mut self, image: RgbaImage) -> ort::Result<Vec<TrackedHand>> {
        let mut regions = std::mem::take(&mut self.regions);
        // Look for new hands while there's room for them: on every image
        // while none are followed, now and then while one is.
        self.since_search += 1;
        if regions.is_empty() || regions.len() < MAX_HANDS && self.since_search >= SEARCH_EVERY {
            self.since_search = 0;
            for palm in self.palms.detect(image)? {
                if regions.len() < MAX_HANDS
                    && regions.iter().all(|(r, _)| r.iou(&palm) <= SAME_HAND_IOU)
                {
                    regions.push((palm, LabelVote::None));
                }
            }
        }
        let mut tracked = Vec::new();
        for (region, vote) in regions {
            let Some(mut hand) = self.landmarks.landmarks(image, &region)? else {
                continue;
            };
            let next = next_region(&hand, image);
            // Two regions that converged on one hand: keep the first.
            if self
                .regions
                .iter()
                .any(|(r, _)| r.iou(&next) > SAME_HAND_IOU)
            {
                continue;
            }
            let vote = vote.add(hand.handedness);
            hand.handedness = vote.label();
            self.regions.push((next, vote));
            tracked.push(TrackedHand { hand, region });
        }
        Ok(tracked)
    }
}

/// The landmarks MediaPipe frames a tracked hand with: wrist, thumb base
/// and each finger's two lowest joints, which move least as fingers bend.
const STABLE: [usize; 12] = [0, 1, 2, 3, 5, 6, 9, 10, 13, 14, 17, 18];

/// Where `hand` should be in the next image: MediaPipe's landmarks-to-rect
/// over the stable landmarks, turned so wrist-to-knuckles points up.
fn next_region(hand: &Hand, image: RgbaImage) -> HandRegion {
    let size = image.size();
    let at = |i: usize| hand.image[i].truncate() * size;
    let points = STABLE.map(at);
    let knuckles = (at(landmark::INDEX_MCP) + at(landmark::RING_MCP)) / 2.0;
    let toward = (knuckles + at(landmark::MIDDLE_MCP)) / 2.0;
    let up = toward - at(landmark::WRIST);
    let rotation = Vec2::NEG_Y.angle_to(up);
    // Bounding box of the points in the frame turned with the hand.
    let unturn = Vec2::from_angle(-rotation);
    let (min, max) = points.iter().fold(
        (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
        |(lo, hi), &p| {
            let q = unturn.rotate(p);
            (lo.min(q), hi.max(q))
        },
    );
    let center = Vec2::from_angle(rotation).rotate((min + max) / 2.0);
    HandRegion::around(
        center,
        max - min,
        up,
        REGION_SCALE,
        REGION_SHIFT,
        hand.score,
    )
}

/// A followed hand's handedness, voted over the frames it's been seen in.
#[derive(Clone, Copy, Debug)]
enum LabelVote {
    /// A newly found hand, not yet labelled.
    None,
    /// Leaning Left above 0, Right below; ±1 is unanimous.
    Lean(f32),
}

impl LabelVote {
    /// The vote after one more frame labelled `label`. A new hand takes
    /// its first label outright.
    fn add(self, label: Handedness) -> Self {
        let toward = if label == Handedness::Left { 1.0 } else { -1.0 };
        Self::Lean(match self {
            Self::None => toward,
            Self::Lean(lean) => lean + (toward - lean) * LABEL_WEIGHT,
        })
    }

    /// The label the vote leans toward; a tie stays Left.
    fn label(self) -> Handedness {
        match self {
            Self::Lean(lean) if lean < 0.0 => Handedness::Right,
            _ => Handedness::Left,
        }
    }
}
