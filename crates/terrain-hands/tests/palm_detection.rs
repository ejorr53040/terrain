//! Seam B, palm detection: on MediaPipe's test images, the Rust detector
//! finds a hand region around every hand Python MediaPipe finds, and none
//! where it finds no hands.

use std::{collections::BTreeMap, path::Path};

use glam::Vec2;
use serde::Deserialize;
use terrain_hands::{HandRegion, PALM_MODEL, PalmDetector, RgbaImage};
use zune_jpeg::{
    JpegDecoder,
    zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions},
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/hands");

#[derive(Deserialize)]
struct GoldenHand {
    /// Normalized to the image: x right, y down.
    image: Vec<[f32; 3]>,
}

fn goldens() -> BTreeMap<String, Vec<GoldenHand>> {
    let text = std::fs::read_to_string(Path::new(FIXTURES).join("goldens.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// (width, height, RGBA pixels)
fn load(name: &str) -> (u32, u32, Vec<u8>) {
    let bytes = std::fs::read(Path::new(FIXTURES).join(name)).unwrap();
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    let rgba = decoder.decode().unwrap();
    let info = decoder.info().unwrap();
    (info.width.into(), info.height.into(), rgba)
}

fn detect(name: &str) -> (u32, u32, Vec<HandRegion>) {
    let (width, height, rgba) = load(name);
    let mut detector = PalmDetector::new(PALM_MODEL).unwrap();
    let regions = detector
        .detect(RgbaImage {
            width,
            height,
            pixels: &rgba,
        })
        .unwrap();
    (width, height, regions)
}

#[test]
fn every_golden_hand_lies_inside_a_detected_region() {
    for (name, hands) in goldens().into_iter().filter(|(_, h)| !h.is_empty()) {
        let (width, height, regions) = detect(&name);
        let size = Vec2::new(width as f32, height as f32);
        for (i, hand) in hands.iter().enumerate() {
            let landmarks: Vec<Vec2> = hand
                .image
                .iter()
                .map(|&[x, y, _]| Vec2::new(x, y) * size)
                .collect();
            assert!(
                regions
                    .iter()
                    .any(|r| landmarks.iter().all(|&p| r.contains(p))),
                "{name}: hand {i} isn't inside any of {} detected region(s): {regions:?}",
                regions.len()
            );
        }
        assert_eq!(
            regions.len(),
            hands.len(),
            "{name}: one region per hand, got {regions:?}"
        );
    }
}

#[test]
fn images_without_hands_have_no_regions() {
    for (name, _) in goldens().into_iter().filter(|(_, h)| h.is_empty()) {
        let (_, _, regions) = detect(&name);
        assert!(regions.is_empty(), "{name}: found {regions:?}");
    }
}
