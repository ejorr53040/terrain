//! Seam B, palm detection: on MediaPipe's test images, the Rust detector
//! finds a hand region around every hand Python MediaPipe finds, and none
//! where it finds no hands.

mod support;

use glam::Vec2;
use support::{Fixture, goldens};
use terrain_hands::{HandRegion, PALM_MODEL, PalmDetector};

fn detect(name: &str) -> (u32, u32, Vec<HandRegion>) {
    let fixture = Fixture::load(name);
    let mut detector = PalmDetector::new(PALM_MODEL).unwrap();
    let regions = detector.detect(fixture.image()).unwrap();
    (fixture.width, fixture.height, regions)
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
