//! Hand landmarks: MediaPipe's full landmark model run on a hand region,
//! with its outputs mapped back to the whole image. Parameters follow
//! MediaPipe's `hand_landmark_cpu` graph.

use std::path::Path;

use glam::Vec2;
use ort::{session::Session, value::Tensor};

use crate::{Hand, HandRegion, Handedness, RgbaImage};

/// The bundled hand landmark model (see `models/SOURCES.json`).
pub const LANDMARK_MODEL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../models/hand_landmark_full.onnx"
);

/// Model input side, in pixels.
const INPUT_SIDE: usize = 224;
/// Model outputs, by name: image landmarks (input pixels), hand presence,
/// handedness (probability of "Left"), world landmarks (meters).
const IMAGE_OUTPUT: &str = "Identity";
const PRESENCE_OUTPUT: &str = "Identity_1";
const HANDEDNESS_OUTPUT: &str = "Identity_2";
const WORLD_OUTPUT: &str = "Identity_3";
/// Below this presence score, the region holds no hand.
const MIN_PRESENCE: f32 = 0.5;
/// Above this handedness score the hand is Left (MediaPipe's cut).
const LEFT_ABOVE: f32 = 0.5;
/// MediaPipe's `normalize_z`: image landmark depth is divided by this.
const DEPTH_SCALE: f32 = 0.4;

/// Finds a hand's landmarks inside a hand region.
pub struct LandmarkModel {
    session: Session,
}

impl LandmarkModel {
    pub fn new(model: impl AsRef<Path>) -> ort::Result<Self> {
        Ok(Self {
            session: crate::model::load(model)?,
        })
    }

    /// The hand in `region` of `image`, in MediaPipe's conventions for the
    /// image as given; `None` if the model sees no hand there.
    pub fn landmarks(
        &mut self,
        image: RgbaImage,
        region: &HandRegion,
    ) -> ort::Result<Option<Hand>> {
        let input = Tensor::from_array(([1, INPUT_SIDE, INPUT_SIDE, 3], crop(image, region)))?;
        let outputs = self.session.run(ort::inputs![input])?;
        let presence = outputs[PRESENCE_OUTPUT].try_extract_tensor::<f32>()?.1[0];
        if presence < MIN_PRESENCE {
            return Ok(None);
        }
        let left = outputs[HANDEDNESS_OUTPUT].try_extract_tensor::<f32>()?.1[0];
        let (_, raw_image) = outputs[IMAGE_OUTPUT].try_extract_tensor::<f32>()?;
        let (_, raw_world) = outputs[WORLD_OUTPUT].try_extract_tensor::<f32>()?;
        let size = image.size();
        let turn = Vec2::from_angle(region.rotation);
        Ok(Some(Hand {
            handedness: if left > LEFT_ABOVE {
                Handedness::Left
            } else {
                Handedness::Right
            },
            score: presence,
            image: std::array::from_fn(|i| {
                let [x, y, z] = [raw_image[3 * i], raw_image[3 * i + 1], raw_image[3 * i + 2]];
                // Input pixels to region-local pixels to normalized image.
                let local = (Vec2::new(x, y) / INPUT_SIDE as f32 - 0.5) * region.size;
                let at = region.image_point(local) / size;
                // Relative depth, in the same units as x (image widths).
                let depth = z / INPUT_SIDE as f32 / DEPTH_SCALE * region.size / size.x;
                at.extend(depth)
            }),
            world: std::array::from_fn(|i| {
                let [x, y, z] = [raw_world[3 * i], raw_world[3 * i + 1], raw_world[3 * i + 2]];
                // The model sees the hand turned upright; turn it back.
                turn.rotate(Vec2::new(x, y)).extend(z)
            }),
        }))
    }
}

/// The model input: `region` of `image` cut out and turned upright, RGB,
/// 0..1, HWC, bilinear-sampled. Sample points follow MediaPipe's OpenCV
/// warp: input pixel `i` samples region coordinate `i / 224`, in a source
/// grid whose pixel centers sit on whole numbers.
fn crop(image: RgbaImage, region: &HandRegion) -> Vec<f32> {
    let mut input = vec![0.0; INPUT_SIDE * INPUT_SIDE * 3];
    let step = region.size / INPUT_SIDE as f32;
    for row in 0..INPUT_SIDE {
        for col in 0..INPUT_SIDE {
            let local = Vec2::new(col as f32, row as f32) * step - region.size / 2.0;
            let at = (row * INPUT_SIDE + col) * 3;
            input[at..at + 3].copy_from_slice(&image.sample(region.image_point(local) + 0.5));
        }
    }
    input
}
