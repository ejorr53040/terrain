//! Desktop control says what the hand is doing, for an on-screen indicator.

mod support;

use glam::Vec3;
use support::{Grip, Script};
use terrain_desktop::Status;

const AHEAD: Vec3 = Vec3::new(0.0, 0.0, 0.6);
const OTHER: Vec3 = Vec3::new(0.12, -0.05, 0.6);
/// Below the reach: a hand back at the keyboard.
const LOWERED: Vec3 = Vec3::new(0.0, -0.3, 0.6);

#[test]
fn a_hand_being_raised_is_engaging_then_pointing() {
    assert_eq!(Script::default().gone(5).status(), Status::Disengaged);
    let raising = Script::default().hold(AHEAD, Grip::Open, 5);
    assert_eq!(raising.status(), Status::Engaging);
    assert_eq!(
        raising.hold(AHEAD, Grip::Open, 15).status(),
        Status::Pointing
    );
}

#[test]
fn a_pinch_is_a_pinch_until_it_drags_or_long_presses() {
    let pinched = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Pinch, 6);
    assert_eq!(pinched.status(), Status::Pinch);
    let moved = AHEAD + Vec3::new(0.08, 0.0, 0.0);
    let dragged = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Pinch, 6)
        .sweep(AHEAD, moved, Grip::Pinch, 10);
    assert_eq!(dragged.status(), Status::Drag);
    assert_eq!(
        pinched.hold(AHEAD, Grip::Pinch, 20).status(),
        Status::LongPress
    );
}

#[test]
fn a_fist_is_a_window_drag() {
    let fist = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Fist, 5);
    assert_eq!(fist.status(), Status::WindowDrag);
}

#[test]
fn the_other_hand_pinching_is_scrolling() {
    let scrolling = Script::default().hold(AHEAD, Grip::Open, 20).hold_two(
        (AHEAD, Grip::Open),
        (OTHER, Grip::Pinch),
        6,
    );
    assert_eq!(scrolling.status(), Status::Scroll);
}

#[test]
fn a_lowered_hand_is_disengaged() {
    let lowered = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(LOWERED, Grip::Open, 40);
    assert_eq!(lowered.status(), Status::Disengaged);
}
