//! The palm points: where the hand is in the reach is where the pointer
//! is on the screen.

use crate::support::{Fingers, Script, biggest_step, buttons, last_point, points};
use glam::{Vec2, Vec3};
use terrain_desktop::{Action, Button};

const AHEAD: Vec3 = Vec3::new(0.0, 0.0, 0.6);

#[test]
fn a_hand_straight_ahead_points_at_the_middle_of_the_screen() {
    let inputs = Script::default().hold(AHEAD, Fingers::Open, 20).run();
    let at = last_point(&inputs).expect("the pointer moved");
    assert!(at.distance(Vec2::splat(0.5)) < 0.02, "pointer at {at}");
    assert_eq!(buttons(&inputs), vec![]);
}

#[test]
fn a_pinch_clicks_where_the_hand_points() {
    let inputs = Script::default()
        .hold(AHEAD, Fingers::Open, 20)
        .hold(AHEAD, Fingers::Pinch, 10)
        .hold(AHEAD, Fingers::Open, 10)
        .run();
    assert_eq!(
        buttons(&inputs),
        vec![Action::Press(Button::Left), Action::Release(Button::Left)]
    );
}

#[test]
fn the_pointer_follows_the_hand_right_and_up() {
    let right_up = AHEAD + Vec3::new(0.1, 0.06, 0.0);
    let inputs = Script::default()
        .hold(AHEAD, Fingers::Open, 20)
        .sweep(AHEAD, right_up, Fingers::Open, 10)
        .hold(right_up, Fingers::Open, 20)
        .run();
    let at = last_point(&inputs).unwrap();
    assert!(at.x > 0.65 && at.y < 0.35, "pointer at {at}");
}

#[test]
fn reaching_past_the_edge_of_the_reach_pins_the_pointer_to_the_screen_edge() {
    let far_left = AHEAD + Vec3::new(-0.3, 0.0, 0.0);
    let inputs = Script::default()
        .hold(AHEAD, Fingers::Open, 20)
        .sweep(AHEAD, far_left, Fingers::Open, 5)
        .hold(far_left, Fingers::Open, 10)
        .run();
    let at = last_point(&inputs).unwrap();
    assert_eq!(at.x, 0.0, "pointer at {at}");
}

#[test]
fn a_hand_that_leaves_view_mid_drag_lets_go() {
    let inputs = Script::default()
        .hold(AHEAD, Fingers::Open, 20)
        .sweep(AHEAD, AHEAD + Vec3::new(0.05, 0.0, 0.0), Fingers::Pinch, 15)
        .gone(20)
        .run();
    assert_eq!(
        buttons(&inputs),
        vec![Action::Press(Button::Left), Action::Release(Button::Left)]
    );
}

#[test]
fn a_brief_dropout_mid_drag_keeps_holding() {
    let inputs = Script::default()
        .hold(AHEAD, Fingers::Open, 20)
        .sweep(AHEAD, AHEAD + Vec3::new(0.05, 0.0, 0.0), Fingers::Pinch, 10)
        .gone(3)
        .hold(AHEAD + Vec3::new(0.05, 0.0, 0.0), Fingers::Pinch, 10)
        .run();
    assert_eq!(buttons(&inputs), vec![Action::Press(Button::Left)]);
}

#[test]
fn a_hand_held_almost_still_keeps_the_pointer_still() {
    // A still hand drifts a few millimetres to and fro, about twice a second.
    let drift = Vec3::new(0.002, 0.001, 0.0);
    let mut script = Script::default().hold(AHEAD, Fingers::Open, 20).sweep(
        AHEAD,
        AHEAD + drift,
        Fingers::Open,
        4,
    );
    for _ in 0..3 {
        script = script
            .sweep(AHEAD + drift, AHEAD - drift, Fingers::Open, 8)
            .sweep(AHEAD - drift, AHEAD + drift, Fingers::Open, 8);
    }
    let shown = points(&script.run());
    let settled = &shown[shown.len() - 30..];
    let wandered = settled
        .iter()
        .map(|p| p.distance(settled[0]))
        .fold(0.0, f32::max);
    assert!(wandered < 0.002, "pointer wandered {wandered}");
}

#[test]
fn a_pointer_released_by_a_gesture_glides_to_the_hand_instead_of_jumping() {
    // A Vee scroll holds the pointer still while the hand moves; opening
    // the hand ends the scroll and lets the pointer follow again.
    let moved = AHEAD + Vec3::new(0.1, 0.0, 0.0);
    let shown = points(
        &Script::default()
            .hold(AHEAD, Fingers::Open, 20)
            .hold(AHEAD, Fingers::Vee, 10)
            .sweep(AHEAD, moved, Fingers::Vee, 10)
            .hold(moved, Fingers::Open, 20)
            .run(),
    );
    let biggest_step = biggest_step(&shown);
    assert!(biggest_step < 0.06, "pointer jumped {biggest_step}");
    let at = *shown.last().unwrap();
    assert!(at.x > 0.65, "pointer never reached the hand: {at}");
}

#[test]
fn a_pointer_released_past_the_screens_edge_glides_there_too() {
    let below = AHEAD + Vec3::new(0.0, -0.3, 0.0);
    let shown = points(
        &Script::default()
            .hold(AHEAD, Fingers::Open, 20)
            .hold(AHEAD, Fingers::Vee, 10)
            .sweep(AHEAD, below, Fingers::Vee, 10)
            .hold(below, Fingers::Open, 20)
            .run(),
    );
    let biggest_step = biggest_step(&shown);
    assert!(biggest_step < 0.15, "pointer jumped {biggest_step}");
    assert_eq!(shown.last().unwrap().y, 1.0);
}

#[test]
fn a_pinch_freezes_the_pointer_where_it_was_shown() {
    // After a quick move the pointer trails the hand a little; the click
    // lands where the pointer was, not where the hand is.
    let moved = AHEAD + Vec3::new(0.05, 0.0, 0.0);
    let shown = points(
        &Script::default()
            .hold(AHEAD, Fingers::Open, 20)
            .sweep(AHEAD, moved, Fingers::Open, 5)
            .hold(moved, Fingers::Open, 15)
            .hold(moved, Fingers::Pinch, 10)
            .run(),
    );
    let rested = &shown[shown.len() - 11..];
    let step = biggest_step(rested);
    assert!(step < 1e-4, "pointer moved {step}");
}

#[test]
fn one_frozen_gesture_turning_into_another_doesnt_jump_the_pointer() {
    // A Vee scroll holds the pointer while the hand moves; closing into a
    // fist starts the fist's gesture, which holds it too, then drags.
    let moved = AHEAD + Vec3::new(0.1, 0.0, 0.0);
    let shown = points(
        &Script::default()
            .hold(AHEAD, Fingers::Open, 20)
            .hold(AHEAD, Fingers::Vee, 10)
            .sweep(AHEAD, moved, Fingers::Vee, 10)
            .hold(moved, Fingers::Fist, 30)
            .run(),
    );
    let step = biggest_step(&shown);
    assert!(step < 0.06, "pointer jumped {step}");
}
