//! Seam B, landmarks: on MediaPipe's test images, the Rust tracker finds
//! the hands Python MediaPipe finds, with matching handedness and landmarks.

mod support;

use glam::{Vec2, Vec3};
use support::{Fixture, GoldenHand, goldens, tracked_goldens};
use terrain_hands::{Hand, HandTracker, LANDMARK_MODEL, PALM_MODEL, TrackedHand, landmark};

/// Largest allowed image landmark error, as a fraction of the image size.
const IMAGE_TOLERANCE: f32 = 0.01;
/// Largest allowed world landmark error, in meters.
const WORLD_TOLERANCE: f32 = 0.01;

fn wrist(points: &[[f32; 3]]) -> Vec2 {
    let [x, y, _] = points[landmark::WRIST];
    Vec2::new(x, y)
}

/// The tracked hand whose wrist is nearest the golden one's.
fn matching<'a>(hands: &'a [Hand], golden: &GoldenHand) -> &'a Hand {
    let at = wrist(&golden.image);
    hands
        .iter()
        .min_by(|a, b| {
            let d = |h: &Hand| h.image[landmark::WRIST].truncate().distance(at);
            d(a).total_cmp(&d(b))
        })
        .unwrap()
}

fn max_error(ours: &[Vec3; 21], theirs: &[[f32; 3]], keep: impl Fn(Vec3) -> Vec3) -> f32 {
    ours.iter()
        .zip(theirs)
        .map(|(&o, &t)| keep(o).distance(keep(Vec3::from_array(t))))
        .fold(0.0, f32::max)
}

/// Asserts `hands` match `golden_hands`, MediaPipe's for image `name`.
fn assert_match(name: &str, hands: &[Hand], golden_hands: &[GoldenHand]) {
    assert_eq!(hands.len(), golden_hands.len(), "{name}: hand count");
    for (i, golden) in golden_hands.iter().enumerate() {
        let hand = matching(hands, golden);
        assert_eq!(hand.handedness, golden.handedness, "{name} hand {i}");
        let image_error = max_error(&hand.image, &golden.image, |p| p.with_z(0.0));
        assert!(
            image_error < IMAGE_TOLERANCE,
            "{name} hand {i}: image landmarks off by up to {image_error:.4}"
        );
        let world_error = max_error(&hand.world, &golden.world, |p| p);
        assert!(
            world_error < WORLD_TOLERANCE,
            "{name} hand {i}: world landmarks off by up to {:.1} mm",
            world_error * 1e3
        );
    }
}

fn hands(tracked: Vec<TrackedHand>) -> Vec<Hand> {
    tracked.into_iter().map(|t| t.hand).collect()
}

#[test]
fn hands_found_in_a_still_image_match_mediapipe() {
    for (name, golden_hands) in goldens() {
        let fixture = Fixture::load(&name);
        let mut tracker = HandTracker::new(PALM_MODEL, LANDMARK_MODEL).unwrap();
        let found = hands(tracker.track(fixture.image()).unwrap());
        assert_match(&name, &found, &golden_hands);
    }
}

#[test]
fn hands_followed_from_the_previous_frame_match_mediapipe() {
    for (name, golden_hands) in tracked_goldens() {
        let fixture = Fixture::load(&name);
        let mut tracker = HandTracker::new(PALM_MODEL, LANDMARK_MODEL).unwrap();
        tracker.track(fixture.image()).unwrap();
        let followed = hands(tracker.track(fixture.image()).unwrap());
        assert_match(&name, &followed, &golden_hands);
    }
}

#[test]
fn a_followed_hand_keeps_its_handedness_when_one_frame_reads_as_the_other_hand() {
    let fixture = Fixture::load("pointing_up.jpg");
    let mut tracker = HandTracker::new(PALM_MODEL, LANDMARK_MODEL).unwrap();
    let first = tracker.track(fixture.image()).unwrap();
    let [found] = first.as_slice() else {
        panic!("expected one hand, found {}", first.len());
    };
    // The same hand in the same place, flipped: on its own, it reads as
    // the other hand.
    let flipped = fixture.mirrored_about(found.region.center.x);
    let alone = HandTracker::new(PALM_MODEL, LANDMARK_MODEL)
        .unwrap()
        .track(flipped.image())
        .unwrap();
    assert_eq!(alone.len(), 1, "flipped image alone: hand count");
    assert_ne!(alone[0].hand.handedness, found.hand.handedness);

    let followed = tracker.track(flipped.image()).unwrap();
    assert_eq!(followed.len(), 1, "followed: hand count");
    assert_eq!(followed[0].hand.handedness, found.hand.handedness);
}

#[test]
fn a_followed_hand_that_keeps_reading_as_the_other_hand_takes_that_label() {
    let fixture = Fixture::load("pointing_up.jpg");
    let mut tracker = HandTracker::new(PALM_MODEL, LANDMARK_MODEL).unwrap();
    let first = tracker.track(fixture.image()).unwrap();
    let [found] = first.as_slice() else {
        panic!("expected one hand, found {}", first.len());
    };
    let flipped = fixture.mirrored_about(found.region.center.x);
    let labels: Vec<_> = (0..6)
        .map(|_| {
            let hands = tracker.track(flipped.image()).unwrap();
            assert_eq!(hands.len(), 1, "followed: hand count");
            hands[0].hand.handedness
        })
        .collect();
    assert_eq!(labels[0], found.hand.handedness, "one misread: {labels:?}");
    assert_ne!(labels[5], found.hand.handedness, "six in a row: {labels:?}");
}
