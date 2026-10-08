//! A fist drags the window under the pointer (Super + left button).

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
    assert_eq!(buttons(&inputs), one_window_drag());
}

#[test]
fn a_pinch_tightening_into_a_fist_drags_the_window_without_clicking() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Pinch, 10)
        .hold(AHEAD, Grip::Fist, 10)
        .hold(AHEAD, Grip::Open, 10)
        .run();
    assert_eq!(buttons(&inputs), one_window_drag());
}

#[test]
fn losing_the_hand_mid_window_drag_lets_go_of_both() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Fist, 10)
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
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Fist, 10)
        .hold(AHEAD, Grip::Pinch, 4)
        .hold(AHEAD, Grip::Open, 10)
        .run();
    assert_eq!(buttons(&inputs), one_window_drag());
}

#[test]
fn closing_into_a_fist_through_a_brief_pinch_doesnt_click() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Pinch, 2)
        .hold(AHEAD, Grip::Fist, 10)
        .hold(AHEAD, Grip::Open, 10)
        .run();
    assert_eq!(buttons(&inputs), one_window_drag());
}
