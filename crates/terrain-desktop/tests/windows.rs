//! A fist drags the window under the pointer (Super + left button); a
//! fist pushed toward the camera toggles full screen (Super + F).

mod support;

use glam::Vec3;
use support::{Grip, Script, buttons, point_at_each_button};
use terrain_desktop::{Button, Input};

const AHEAD: Vec3 = Vec3::new(0.0, 0.0, 0.6);

#[test]
fn a_fist_drags_the_window_under_the_pointer() {
    let moved = AHEAD + Vec3::new(0.08, 0.0, 0.0);
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Fist, 10)
        .sweep(AHEAD, moved, Grip::Fist, 10)
        .hold(moved, Grip::Fist, 20)
        .hold(moved, Grip::Open, 10)
        .run();
    // The window manager drags a window on Super + left button.
    assert_eq!(buttons(&inputs), one_window_drag());
    let at = point_at_each_button(&inputs);
    let (grabbed, dropped) = (at[1].1.unwrap(), at[2].1.unwrap());
    assert!(
        dropped.x - grabbed.x > 0.15,
        "dragged from {grabbed} to {dropped}"
    );
}

#[test]
fn a_fist_that_reads_as_a_pinch_never_clicks() {
    // A fist's thumb lies on the index finger: thumb and index tips are as
    // close as in a pinch.
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Fist, 20)
        .hold(AHEAD, Grip::Open, 10)
        .run();
    assert_eq!(buttons(&inputs), vec![]);
}

#[test]
fn a_pinch_tightening_into_a_fist_doesnt_click() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Pinch, 10)
        .hold(AHEAD, Grip::Fist, 20)
        .hold(AHEAD, Grip::Open, 10)
        .run();
    assert_eq!(buttons(&inputs), vec![]);
}

#[test]
fn losing_the_hand_mid_window_drag_lets_go_of_both() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Fist, 3)
        .sweep(AHEAD, AHEAD + Vec3::X * 0.08, Grip::Fist, 10)
        .gone(20)
        .run();
    assert_eq!(buttons(&inputs), one_window_drag());
}

/// Super pressed, left pressed, left released, Super released: one window drag.
fn one_window_drag() -> Vec<Input> {
    vec![
        Input::Press(Button::Super),
        Input::Press(Button::Left),
        Input::Release(Button::Left),
        Input::Release(Button::Super),
    ]
}

#[test]
fn a_fist_opening_through_a_pinch_drops_the_window_without_clicking() {
    let moved = AHEAD + Vec3::X * 0.08;
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Fist, 3)
        .sweep(AHEAD, moved, Grip::Fist, 10)
        .hold(moved, Grip::Pinch, 4)
        .hold(moved, Grip::Open, 10)
        .run();
    assert_eq!(buttons(&inputs), one_window_drag());
}

#[test]
fn closing_into_a_fist_through_a_brief_pinch_doesnt_click() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Pinch, 2)
        .hold(AHEAD, Grip::Fist, 20)
        .hold(AHEAD, Grip::Open, 10)
        .run();
    assert_eq!(buttons(&inputs), vec![]);
}

#[test]
fn a_fist_pushed_toward_the_camera_toggles_full_screen() {
    let near = AHEAD - Vec3::Z * 0.15;
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Fist, 3)
        .sweep(AHEAD, near, Grip::Fist, 8)
        .hold(near, Grip::Fist, 5)
        .hold(near, Grip::Open, 10)
        .run();
    assert_eq!(buttons(&inputs), full_screen());
}

#[test]
fn a_fist_pushed_from_off_centre_still_toggles_full_screen() {
    // Off the camera's axis, a palm coming nearer drifts outward in the
    // view: that's not a window drag.
    let from = AHEAD + Vec3::new(0.12, -0.08, 0.0);
    let near = from - Vec3::Z * 0.15;
    let inputs = Script::default()
        .hold(from, Grip::Open, 20)
        .hold(from, Grip::Fist, 3)
        .sweep(from, near, Grip::Fist, 8)
        .hold(near, Grip::Fist, 5)
        .hold(near, Grip::Open, 10)
        .run();
    assert_eq!(buttons(&inputs), full_screen());
}

#[test]
fn a_fist_held_still_before_the_push_still_toggles_full_screen() {
    // People close the fist, settle, then push: a still fist mustn't grab.
    let near = AHEAD - Vec3::Z * 0.15;
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Fist, 40)
        .sweep(AHEAD, near, Grip::Fist, 8)
        .hold(near, Grip::Fist, 5)
        .hold(near, Grip::Open, 10)
        .run();
    assert_eq!(buttons(&inputs), full_screen());
}

#[test]
fn a_fist_still_moving_after_full_screen_doesnt_drag_the_window() {
    // The push's follow-through drifts sideways before the hand opens.
    let near = AHEAD - Vec3::Z * 0.15;
    let drifted = near + Vec3::new(0.06, 0.0, 0.0);
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Fist, 3)
        .sweep(AHEAD, near, Grip::Fist, 8)
        .sweep(near, drifted, Grip::Fist, 10)
        .hold(drifted, Grip::Open, 10)
        .run();
    assert_eq!(buttons(&inputs), full_screen());
}

#[test]
fn a_quick_still_fist_does_nothing() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Fist, 8)
        .hold(AHEAD, Grip::Open, 10)
        .run();
    assert_eq!(buttons(&inputs), vec![]);
}

/// Omarchy's full screen: Super + F.
fn full_screen() -> Vec<Input> {
    vec![
        Input::Press(Button::Super),
        Input::Press(Button::F),
        Input::Release(Button::F),
        Input::Release(Button::Super),
    ]
}
