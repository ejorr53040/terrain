//! A hand cursor over the overlay works the overlay, never the scene.

use crate::support::*;
use bevy::prelude::*;
use terrain_app::{HandCursor, OverlayArea, OverlayOpen, OverlayPointer};

const AHEAD: Vec3 = Vec3::new(0.0, 0.0, 0.5);

/// The cube pinched and pulled 10 cm right, then let go, with the overlay
/// covering `overlay` of the view (fractions, from the top left).
fn pull_with_overlay(overlay: Option<Rect>) -> Vec3 {
    let camera = TestCamera::default();
    let moved = AHEAD + Vec3::new(0.10, 0.0, 0.0);
    let mut h = Harness::new(
        [
            camera.hold(&[Pose::open(AHEAD)], SETTLE),
            camera.hold(&[Pose::pinched(AHEAD)], SETTLE),
            camera.hold(&[Pose::pinched(moved)], SETTLE),
            camera.hold(&[Pose::open(moved)], SETTLE),
        ]
        .concat(),
    );
    h.app.world_mut().resource_mut::<OverlayArea>().0 = overlay;
    h.run_all();
    h.cube().translation
}

#[test]
fn a_pinch_over_the_overlay_grabs_nothing() {
    assert!(
        pull_with_overlay(None).x > 0.05,
        "without the overlay the pinch pulls the cube"
    );
    let everywhere = Rect::new(0.0, 0.0, 1.0, 1.0);
    assert_near(pull_with_overlay(Some(everywhere)), Vec3::ZERO);
}

/// Where a hand `at` depth `z` puts its cursor at the view's right edge.
fn at_right_edge(z: f32) -> Vec3 {
    let half_w = (TestCamera::default().hfov_deg.to_radians() / 2.0).tan();
    Vec3::new(0.485 * 2.0 * half_w * z, 0.0, z)
}

/// An open hand resting at the right edge for `ms`.
fn rest_at_edge(ms: u64) -> Vec<terrain_hands::HandFrame> {
    hold(&[Pose::open(at_right_edge(0.5))], (ms / FRAME_MS) as usize)
}

fn open(h: &Harness) -> bool {
    h.app.world().resource::<OverlayOpen>().open
}

#[test]
fn resting_a_hand_on_the_hot_edge_opens_the_overlay() {
    let mut brief = Harness::new([hold(&[Pose::open(AHEAD)], 5), rest_at_edge(150)].concat());
    brief.run_all();
    assert!(!open(&brief), "a brush past the edge doesn't open it");
    let mut rested = Harness::new([hold(&[Pose::open(AHEAD)], 5), rest_at_edge(400)].concat());
    rested.run_all();
    assert!(open(&rested));
}

#[test]
fn the_overlay_closes_a_second_after_the_hand_leaves_it() {
    let away = |ms: u64| hold(&[Pose::open(AHEAD)], (ms / FRAME_MS) as usize);
    let mut h = Harness::new([rest_at_edge(400), away(500)].concat());
    h.run_all();
    assert!(open(&h), "still open half a second after leaving");
    let mut h = Harness::new([rest_at_edge(400), away(1300)].concat());
    h.run_all();
    assert!(!open(&h), "closed after a second away");
}

#[test]
fn a_hand_over_the_overlay_keeps_it_open() {
    // The overlay is a panel down the right third.
    let over = Vec3::new(0.15, 0.0, 0.5);
    let mut h = Harness::new([rest_at_edge(400), hold(&[Pose::open(over)], 60)].concat());
    h.app.world_mut().resource_mut::<OverlayArea>().0 = Some(Rect::new(0.66, 0.0, 1.0, 1.0));
    h.run_all();
    assert!(open(&h));
}

/// The overlay down the right half of the view.
const RIGHT_HALF: Rect = Rect {
    min: Vec2::new(0.5, 0.0),
    max: Vec2::new(1.0, 1.0),
};
/// A hand in front of the right half, and one in front of the left.
const OVER: Vec3 = Vec3::new(0.1, 0.0, 0.5);
const OFF: Vec3 = Vec3::new(-0.1, 0.0, 0.5);

fn with_overlay(frames: Vec<terrain_hands::HandFrame>) -> Harness {
    let mut h = Harness::new(frames);
    h.app.world_mut().resource_mut::<OverlayArea>().0 = Some(RIGHT_HALF);
    h
}

fn pointer(h: &Harness) -> OverlayPointer {
    h.app.world().resource::<OverlayPointer>().clone()
}

fn cursor_of(h: &mut Harness) -> Vec2 {
    let mut q = h.app.world_mut().query::<&HandCursor>();
    q.iter(h.app.world()).next().expect("a hand cursor").at
}

#[test]
fn a_hand_over_the_overlay_points_there_and_off_it_points_nowhere() {
    let mut h = with_overlay(hold(&[Pose::open(OVER)], SETTLE));
    h.run_all();
    assert_eq!(pointer(&h).at, Some(cursor_of(&mut h)));
    let mut h = with_overlay(hold(&[Pose::open(OFF)], SETTLE));
    h.run_all();
    assert_eq!(pointer(&h).at, None);
}

#[test]
fn a_pinch_presses_and_opening_releases() {
    let mut h = with_overlay(
        [
            hold(&[Pose::open(OVER)], SETTLE),
            hold(&[Pose::pinched(OVER)], SETTLE),
        ]
        .concat(),
    );
    h.run_all();
    assert!(pointer(&h).pressed);
    let mut h = with_overlay(
        [
            hold(&[Pose::open(OVER)], SETTLE),
            hold(&[Pose::pinched(OVER)], SETTLE),
            hold(&[Pose::open(OVER)], SETTLE),
        ]
        .concat(),
    );
    h.run_all();
    assert!(!pointer(&h).pressed);
}

#[test]
fn a_pinch_drag_moves_the_pointer_at_half_the_hand_s_speed() {
    let to = OVER + Vec3::new(0.0, 0.05, 0.0);
    let mut h = with_overlay(
        [
            hold(&[Pose::open(OVER)], SETTLE),
            hold(&[Pose::pinched(OVER)], SETTLE),
        ]
        .concat(),
    );
    h.run_all();
    let (pointer_from, hand_from) = (pointer(&h).at.unwrap(), cursor_of(&mut h));
    let mut h = with_overlay(
        [
            hold(&[Pose::open(OVER)], SETTLE),
            hold(&[Pose::pinched(OVER)], SETTLE),
            hold(&[Pose::pinched(to)], SETTLE),
        ]
        .concat(),
    );
    h.run_all();
    let (pointer_to, hand_to) = (pointer(&h).at.unwrap(), cursor_of(&mut h));
    let hand_moved = hand_to - hand_from;
    assert!(hand_moved.length() > 0.05, "the hand should have moved");
    let pointer_moved = pointer_to - pointer_from;
    assert!(
        (pointer_moved - 0.5 * hand_moved).length() < 0.005,
        "pointer moved {pointer_moved}, hand {hand_moved}"
    );
}

#[test]
fn a_hand_already_pinching_does_not_take_the_pointer() {
    // Carrying a shape onto the overlay isn't pressing on it.
    let mut h = with_overlay(
        [
            hold(&[Pose::pinched(OFF)], SETTLE),
            hold(&[Pose::pinched(OVER)], SETTLE),
        ]
        .concat(),
    );
    h.run_all();
    assert_eq!(pointer(&h).at, None);
}

#[test]
fn the_first_hand_over_the_overlay_keeps_the_pointer() {
    // The second hand comes in higher, so its cursor is nearer the top.
    let second = OVER + Vec3::new(0.05, 0.1, 0.0);
    let mut h = with_overlay(
        [
            hold(&[Pose::open(OVER), Pose::open(OFF).left()], SETTLE),
            hold(&[Pose::open(OVER), Pose::open(second).left()], SETTLE),
        ]
        .concat(),
    );
    h.run_all();
    let mut q = h.app.world_mut().query::<&HandCursor>();
    let first = q
        .iter(h.app.world())
        .map(|c| c.at)
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap();
    assert_eq!(pointer(&h).at, Some(first));
}
