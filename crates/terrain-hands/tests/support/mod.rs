//! Seam B fixtures: MediaPipe's test images and Python MediaPipe's goldens
//! for them (see `fixtures/hands/README.md`).

#![allow(dead_code)]

use std::{collections::BTreeMap, path::Path};

use serde::Deserialize;
use terrain_hands::{Handedness, RgbaImage};
use zune_jpeg::{
    JpegDecoder,
    zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions},
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/hands");

/// One hand as Python MediaPipe reports it.
#[derive(Deserialize)]
pub struct GoldenHand {
    /// As MediaPipe labels it on the unmirrored image.
    pub handedness: Handedness,
    /// Normalized to the unmirrored image: x right, y down; z relative depth.
    pub image: Vec<[f32; 3]>,
    /// Meters, hand-centered.
    pub world: Vec<[f32; 3]>,
}

/// Image name to the hands MediaPipe finds in the image on its own.
pub fn goldens() -> BTreeMap<String, Vec<GoldenHand>> {
    read("goldens.json")
}

/// Image name to the hands MediaPipe tracks in the image as the second
/// frame of a video whose first frame was the same image.
pub fn tracked_goldens() -> BTreeMap<String, Vec<GoldenHand>> {
    read("tracked.json")
}

fn read(file: &str) -> BTreeMap<String, Vec<GoldenHand>> {
    let text = std::fs::read_to_string(Path::new(FIXTURES).join(file)).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// A decoded fixture image.
pub struct Fixture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Fixture {
    pub fn load(name: &str) -> Self {
        let bytes = std::fs::read(Path::new(FIXTURES).join(name)).unwrap();
        let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
        let rgba = decoder.decode().unwrap();
        let info = decoder.info().unwrap();
        Self {
            width: info.width.into(),
            height: info.height.into(),
            rgba,
        }
    }

    /// This image flipped left to right about column `x` (pixels), so
    /// what's at `x` stays put; columns from beyond the edge repeat it.
    pub fn mirrored_about(&self, x: f32) -> Self {
        let width = self.width as usize;
        let mut rgba = Vec::with_capacity(self.rgba.len());
        for row in self.rgba.chunks(width * 4) {
            for col in 0..width {
                let from = (2.0 * x - col as f32 - 1.0)
                    .round()
                    .clamp(0.0, width as f32 - 1.0);
                let at = from as usize * 4;
                rgba.extend_from_slice(&row[at..at + 4]);
            }
        }
        Self {
            width: self.width,
            height: self.height,
            rgba,
        }
    }

    pub fn image(&self) -> RgbaImage<'_> {
        RgbaImage {
            width: self.width,
            height: self.height,
            pixels: &self.rgba,
        }
    }
}
