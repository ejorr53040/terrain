//! A pinch works like a finger on a touchscreen: a tap clicks, a pinch
//! that moves drags, a pinch held still opens the context menu.

mod support;

use glam::Vec3;
use support::{Grip, Script, buttons, point_at_each_button};
use terrain_desktop::{Button, Input};

const AHEAD: Vec3 = Vec3::new(0.0, 0.0, 0.6);
const RIGHT: Vec3 = Vec3::new(0.08, 0.0, 0.6);

#[test]
fn a_quick_pinch_clicks_where_it_started() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Pinch, 8)
        .hold(AHEAD, Grip::Open, 10)
        .run();
    let at = point_at_each_button(&inputs);
    assert_eq!(
        buttons(&inputs),
        vec![Input::Press(Button::Left), Input::Release(Button::Left)]
    );
    assert_eq!(at[0].1, at[1].1, "the pointer held still through the click");
}

#[test]
fn a_pinch_held_still_right_clicks_once() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Pinch, 30)
        .hold(AHEAD, Grip::Open, 10)
        .run();
    assert_eq!(
        buttons(&inputs),
        vec![Input::Press(Button::Right), Input::Release(Button::Right)]
    );
}

#[test]
fn a_pinch_that_moves_drags_from_where_it_started() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Pinch, 6)
        .sweep(AHEAD, RIGHT, Grip::Pinch, 10)
        .hold(RIGHT, Grip::Pinch, 10)
        .hold(RIGHT, Grip::Open, 10)
        .run();
    assert_eq!(
        buttons(&inputs),
        vec![Input::Press(Button::Left), Input::Release(Button::Left)]
    );
    let at = point_at_each_button(&inputs);
    let (pressed, released) = (at[0].1.unwrap(), at[1].1.unwrap());
    assert!((pressed.x - 0.5).abs() < 0.02, "pressed at {pressed}");
    assert!(
        released.x - pressed.x > 0.15,
        "dragged from {pressed} to {released}"
    );
}
