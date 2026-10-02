//! Palm detection: MediaPipe's full palm detector, run with ONNX Runtime,
//! and the glue around it (letterbox, anchor decoding, weighted NMS, and the
//! rotated hand region the landmark model will crop). Parameters follow
//! MediaPipe's `palm_detection_cpu` and `palm_detection_detection_to_roi`
//! graphs.

use std::path::Path;

use glam::Vec2;
use ort::{session::Session, value::Tensor};

/// The bundled palm detection model (see `models/SOURCES.json`).
pub const PALM_MODEL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../models/palm_detection_full.onnx"
);

/// Model input side, in pixels.
const INPUT_SIDE: usize = 192;
/// One anchor per model output row.
const ANCHOR_COUNT: usize = 2016;
/// Model outputs, by name: box and keypoint regressions, and raw scores.
const BOXES_OUTPUT: &str = "Identity";
const SCORES_OUTPUT: &str = "Identity_1";
/// Raw scores are clipped to ± this before the sigmoid, as MediaPipe does.
const SCORE_CLIP: f32 = 100.0;
/// At most this many hands are reported, best first.
const MAX_HANDS: usize = 2;
/// Detections scoring below this are dropped.
const MIN_SCORE: f32 = 0.5;
/// Detections overlapping more than this (IoU) are merged.
const MIN_SUPPRESSION_IOU: f32 = 0.3;
/// The hand region is this much bigger than the palm box...
const REGION_SCALE: f32 = 2.6;
/// ...and moved this far (in palm-box heights) from the palm toward the fingers.
const REGION_SHIFT: f32 = 0.5;

const NUM_KEYPOINTS: usize = 7;
/// Values per model output row: box center and size, then the keypoints.
const ROW: usize = 4 + 2 * NUM_KEYPOINTS;
const WRIST: usize = 0;
const MIDDLE_KNUCKLE: usize = 2;

/// A borrowed RGBA image, row-major, 8 bits per channel.
#[derive(Clone, Copy)]
pub struct RgbaImage<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: &'a [u8],
}

/// Where a hand is: a square in image pixels (x right, y down), turned so
/// the hand points up inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandRegion {
    pub center: Vec2,
    /// Side length, in pixels.
    pub size: f32,
    /// Radians; the region's up (-y) axis is the image's turned by this,
    /// clockwise on screen.
    pub rotation: f32,
    /// Detector confidence, 0..1.
    pub score: f32,
}

impl HandRegion {
    /// Whether `point` (image pixels) lies inside the region.
    pub fn contains(&self, point: Vec2) -> bool {
        let local = Vec2::from_angle(-self.rotation).rotate(point - self.center);
        local.abs().max_element() <= self.size / 2.0
    }

    /// Corners in image pixels, clockwise on screen from top-left.
    pub fn corners(&self) -> [Vec2; 4] {
        let turn = Vec2::from_angle(self.rotation);
        let h = self.size / 2.0;
        [(-h, -h), (h, -h), (h, h), (-h, h)]
            .map(|(x, y)| self.center + turn.rotate(Vec2::new(x, y)))
    }
}

/// Finds hands in images.
pub struct PalmDetector {
    session: Session,
    anchors: Vec<Vec2>,
}

impl PalmDetector {
    pub fn new(model: impl AsRef<Path>) -> ort::Result<Self> {
        Ok(Self {
            session: Session::builder()?.commit_from_file(model)?,
            anchors: anchors(),
        })
    }

    /// Up to `MAX_HANDS` hand regions in `image`, best first.
    pub fn detect(&mut self, image: RgbaImage) -> ort::Result<Vec<HandRegion>> {
        let letterbox = Letterbox::fit(image.width, image.height);
        let input = Tensor::from_array(([1, INPUT_SIDE, INPUT_SIDE, 3], letterbox.sample(image)))?;
        let outputs = self.session.run(ort::inputs![input])?;
        let (_, boxes) = outputs[BOXES_OUTPUT].try_extract_tensor::<f32>()?;
        let (_, scores) = outputs[SCORES_OUTPUT].try_extract_tensor::<f32>()?;
        let palms = self
            .anchors
            .iter()
            .zip(boxes.as_chunks::<ROW>().0)
            .zip(scores)
            .filter_map(|((&anchor, raw), &score)| {
                let score = sigmoid(score.clamp(-SCORE_CLIP, SCORE_CLIP));
                (score >= MIN_SCORE).then(|| Palm::decode(anchor, raw, score, &letterbox))
            })
            .collect();
        Ok(weighted_nms(palms)
            .iter()
            .take(MAX_HANDS)
            .map(Palm::region)
            .collect())
    }
}

/// Anchor centers in model input units (0..1), one per model output row:
/// MediaPipe's SSD anchors for this model (strides 8, 16, 16, 16; two
/// fixed-size anchors per layer per cell, layers of equal stride merged).
fn anchors() -> Vec<Vec2> {
    let mut anchors = Vec::with_capacity(ANCHOR_COUNT);
    for (stride, per_cell) in [(8, 2), (16, 6)] {
        let cells = INPUT_SIDE / stride;
        for y in 0..cells {
            for x in 0..cells {
                let center = (Vec2::new(x as f32, y as f32) + 0.5) / cells as f32;
                anchors.extend(std::iter::repeat_n(center, per_cell));
            }
        }
    }
    debug_assert_eq!(anchors.len(), ANCHOR_COUNT);
    anchors
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// How an image is fitted, aspect kept, into the square model input, with
/// black bars on the short side.
struct Letterbox {
    /// Input pixels per image pixel.
    scale: f32,
    /// Where the image's top-left lands in the input, in input pixels.
    offset: Vec2,
}

impl Letterbox {
    fn fit(width: u32, height: u32) -> Self {
        let scale = INPUT_SIDE as f32 / width.max(height) as f32;
        let fitted = Vec2::new(width as f32, height as f32) * scale;
        Self {
            scale,
            offset: (Vec2::splat(INPUT_SIDE as f32) - fitted) / 2.0,
        }
    }

    /// The model input: RGB, 0..1, HWC, bilinear-sampled.
    fn sample(&self, image: RgbaImage) -> Vec<f32> {
        let mut input = vec![0.0; INPUT_SIDE * INPUT_SIDE * 3];
        let (w, h) = (image.width as usize, image.height as usize);
        let at = |x: usize, y: usize, c: usize| image.pixels[(y * w + x) * 4 + c] as f32 / 255.0;
        for row in 0..INPUT_SIDE {
            for col in 0..INPUT_SIDE {
                let p = self.to_image(Vec2::new(col as f32 + 0.5, row as f32 + 0.5)) - 0.5;
                if p.x < -0.5 || p.y < -0.5 || p.x > w as f32 - 0.5 || p.y > h as f32 - 0.5 {
                    continue;
                }
                let p = p.clamp(Vec2::ZERO, Vec2::new(w as f32 - 1.0, h as f32 - 1.0));
                let (x0, y0) = (p.x as usize, p.y as usize);
                let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
                let (fx, fy) = (p.x - x0 as f32, p.y - y0 as f32);
                for c in 0..3 {
                    let top = at(x0, y0, c) * (1.0 - fx) + at(x1, y0, c) * fx;
                    let bottom = at(x0, y1, c) * (1.0 - fx) + at(x1, y1, c) * fx;
                    input[(row * INPUT_SIDE + col) * 3 + c] = top * (1.0 - fy) + bottom * fy;
                }
            }
        }
        input
    }

    /// Input pixels to image pixels.
    fn to_image(&self, input: Vec2) -> Vec2 {
        (input - self.offset) / self.scale
    }
}

/// One palm detection, in image pixels.
#[derive(Clone, Copy, Debug)]
struct Palm {
    center: Vec2,
    size: Vec2,
    keypoints: [Vec2; NUM_KEYPOINTS],
    score: f32,
}

impl Palm {
    /// `raw` is one model output row: box center and size, then keypoints,
    /// all in input pixels relative to `anchor`.
    fn decode(anchor: Vec2, raw: &[f32; ROW], score: f32, letterbox: &Letterbox) -> Self {
        let point = |i: usize| {
            letterbox.to_image(anchor * INPUT_SIDE as f32 + Vec2::new(raw[i], raw[i + 1]))
        };
        Self {
            center: point(0),
            size: Vec2::new(raw[2], raw[3]) / letterbox.scale,
            keypoints: std::array::from_fn(|k| point(4 + 2 * k)),
            score,
        }
    }

    fn iou(&self, other: &Palm) -> f32 {
        let (a0, a1) = (self.center - self.size / 2.0, self.center + self.size / 2.0);
        let (b0, b1) = (
            other.center - other.size / 2.0,
            other.center + other.size / 2.0,
        );
        let overlap = (a1.min(b1) - a0.max(b0)).max(Vec2::ZERO);
        let intersection = overlap.x * overlap.y;
        let union = self.size.x * self.size.y + other.size.x * other.size.y - intersection;
        if union > 0.0 {
            intersection / union
        } else {
            0.0
        }
    }

    /// The hand region: turned so wrist-to-middle-knuckle points up,
    /// squared on the palm box's long side, scaled up and moved toward the
    /// fingers.
    fn region(&self) -> HandRegion {
        let up = self.keypoints[MIDDLE_KNUCKLE] - self.keypoints[WRIST];
        // Angle from the region's up (0, -1) to `up`, clockwise on screen.
        let rotation = Vec2::NEG_Y.angle_to(up);
        let fingers = Vec2::from_angle(rotation).rotate(Vec2::NEG_Y);
        HandRegion {
            center: self.center + fingers * self.size.y * REGION_SHIFT,
            size: self.size.max_element() * REGION_SCALE,
            rotation,
            score: self.score,
        }
    }
}

/// MediaPipe's weighted non-max suppression: each cluster of overlapping
/// detections becomes their score-weighted average, scored as its best.
fn weighted_nms(mut palms: Vec<Palm>) -> Vec<Palm> {
    palms.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut merged = Vec::new();
    while let Some(best) = palms.first().copied() {
        let (cluster, rest): (Vec<Palm>, Vec<Palm>) = palms
            .into_iter()
            .partition(|p| best.iou(p) > MIN_SUPPRESSION_IOU);
        palms = rest;
        let total: f32 = cluster.iter().map(|p| p.score).sum();
        let mean = |f: &dyn Fn(&Palm) -> Vec2| {
            cluster.iter().map(|p| f(p) * p.score).sum::<Vec2>() / total
        };
        merged.push(Palm {
            center: mean(&|p| p.center),
            size: mean(&|p| p.size),
            keypoints: std::array::from_fn(|k| mean(&|p| p.keypoints[k])),
            score: best.score,
        });
    }
    merged
}
