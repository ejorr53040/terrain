//! The palm points: where the hand is in the reach is where the pointer
//! is on the screen.

mod support;

use glam::{Vec2, Vec3};
use support::{Grip, Script, buttons, last_point};
use terrain_desktop::{Button, Input};

const AHEAD: Vec3 = Vec3::new(0.0, 0.0, 0.6);

#[test]
fn a_hand_straight_ahead_points_at_the_middle_of_the_screen() {
    let inputs = Script::default().hold(AHEAD, Grip::Open, 20).run();
    let at = last_point(&inputs).expect("the pointer moved");
    assert!(at.distance(Vec2::splat(0.5)) < 0.02, "pointer at {at}");
    assert_eq!(buttons(&inputs), vec![]);
}

#[test]
fn a_pinch_clicks_where_the_hand_points() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Pinch, 10)
        .hold(AHEAD, Grip::Open, 10)
        .run();
    assert_eq!(
        buttons(&inputs),
        vec![Input::Press(Button::Left), Input::Release(Button::Left)]
    );
}

#[test]
fn the_pointer_follows_the_hand_right_and_up() {
    let right_up = AHEAD + Vec3::new(0.1, 0.06, 0.0);
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .sweep(AHEAD, right_up, Grip::Open, 10)
        .hold(right_up, Grip::Open, 20)
        .run();
    let at = last_point(&inputs).unwrap();
    assert!(at.x > 0.65 && at.y < 0.35, "pointer at {at}");
}

#[test]
fn reaching_past_the_edge_of_the_reach_pins_the_pointer_to_the_screen_edge() {
    let far_left = AHEAD + Vec3::new(-0.3, 0.0, 0.0);
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .sweep(AHEAD, far_left, Grip::Open, 5)
        .hold(far_left, Grip::Open, 10)
        .run();
    let at = last_point(&inputs).unwrap();
    assert_eq!(at.x, 0.0, "pointer at {at}");
}

#[test]
fn a_hand_that_leaves_view_mid_drag_lets_go() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .sweep(AHEAD, AHEAD + Vec3::new(0.05, 0.0, 0.0), Grip::Pinch, 10)
        .gone(20)
        .run();
    assert_eq!(
        buttons(&inputs),
        vec![Input::Press(Button::Left), Input::Release(Button::Left)]
    );
}

#[test]
fn a_brief_dropout_mid_drag_keeps_holding() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .sweep(AHEAD, AHEAD + Vec3::new(0.05, 0.0, 0.0), Grip::Pinch, 10)
        .gone(3)
        .hold(AHEAD + Vec3::new(0.05, 0.0, 0.0), Grip::Pinch, 10)
        .run();
    assert_eq!(buttons(&inputs), vec![Input::Press(Button::Left)]);
}
