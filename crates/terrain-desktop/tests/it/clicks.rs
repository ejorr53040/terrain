//! A pinch works like a finger on a touchscreen: a tap clicks, a pinch
//! that moves drags, a pinch held still opens the context menu.

use crate::support::{Fingers, Script, buttons, point_at_each_button};
use glam::Vec3;
use terrain_desktop::{Action, Button};

const AHEAD: Vec3 = Vec3::new(0.0, 0.0, 0.6);
const RIGHT: Vec3 = Vec3::new(0.08, 0.0, 0.6);

#[test]
fn a_quick_pinch_clicks_where_it_started() {
    let inputs = Script::default()
        .hold(AHEAD, Fingers::Open, 20)
        .hold(AHEAD, Fingers::Pinch, 8)
        .hold(AHEAD, Fingers::Open, 10)
        .run();
    let at = point_at_each_button(&inputs);
    assert_eq!(
        buttons(&inputs),
        vec![Action::Press(Button::Left), Action::Release(Button::Left)]
    );
    assert_eq!(at[0].1, at[1].1, "the pointer held still through the click");
}

#[test]
fn a_pinch_held_still_right_clicks_once() {
    let inputs = Script::default()
        .hold(AHEAD, Fingers::Open, 20)
        .hold(AHEAD, Fingers::Pinch, 30)
        .hold(AHEAD, Fingers::Open, 10)
        .run();
    assert_eq!(
        buttons(&inputs),
        vec![Action::Press(Button::Right), Action::Release(Button::Right)]
    );
}

#[test]
fn a_pinch_that_moves_drags_from_where_it_started() {
    let inputs = Script::default()
        .hold(AHEAD, Fingers::Open, 20)
        .hold(AHEAD, Fingers::Pinch, 6)
        .sweep(AHEAD, RIGHT, Fingers::Pinch, 10)
        .hold(RIGHT, Fingers::Pinch, 10)
        .hold(RIGHT, Fingers::Open, 10)
        .run();
    assert_eq!(
        buttons(&inputs),
        vec![Action::Press(Button::Left), Action::Release(Button::Left)]
    );
    let at = point_at_each_button(&inputs);
    let (pressed, released) = (at[0].1.unwrap(), at[1].1.unwrap());
    assert!((pressed.x - 0.5).abs() < 0.02, "pressed at {pressed}");
    assert!(
        released.x - pressed.x > 0.15,
        "dragged from {pressed} to {released}"
    );
}

#[test]
fn a_pinch_held_still_then_moved_drags_instead_of_right_clicking() {
    let inputs = Script::default()
        .hold(AHEAD, Fingers::Open, 20)
        .hold(AHEAD, Fingers::Pinch, 40)
        .sweep(AHEAD, RIGHT, Fingers::Pinch, 10)
        .hold(RIGHT, Fingers::Open, 10)
        .run();
    assert_eq!(
        buttons(&inputs),
        vec![Action::Press(Button::Left), Action::Release(Button::Left)]
    );
    let pressed = point_at_each_button(&inputs)[0].1.unwrap();
    assert!((pressed.x - 0.5).abs() < 0.02, "pressed at {pressed}");
}

#[test]
fn a_long_press_lost_with_its_hand_never_clicks() {
    let lost = Script::default()
        .hold(AHEAD, Fingers::Open, 20)
        .hold(AHEAD, Fingers::Pinch, 40)
        .gone(20)
        .run();
    assert_eq!(
        buttons(&lost),
        vec![],
        "a long press lost with its hand never happened"
    );
}

#[test]
fn a_pinch_right_clicks_from_six_tenths_of_a_second_still() {
    // The pinch counts from 120 ms after it closes; frames are 33 ms apart.
    let opened_after = |frames| {
        buttons(
            &Script::default()
                .hold(AHEAD, Fingers::Open, 20)
                .hold(AHEAD, Fingers::Pinch, frames)
                .hold(AHEAD, Fingers::Open, 10)
                .run(),
        )
    };
    let left = vec![Action::Press(Button::Left), Action::Release(Button::Left)];
    let right = vec![Action::Press(Button::Right), Action::Release(Button::Right)];
    assert_eq!(opened_after(20), left, "open after 0.66 s: a tap");
    assert_eq!(opened_after(24), right, "open after 0.79 s: a long press");
}
