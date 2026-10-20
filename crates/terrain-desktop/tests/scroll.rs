//! A Vee scrolls: the further the hand moves from where the Vee began, the
//! faster the page scrolls that way, with the pointer held still.

mod support;

use glam::{Vec2, Vec3};
use support::{Grip, Script, buttons, last_point};
use terrain_desktop::Input;

const AHEAD: Vec3 = Vec3::new(0.0, 0.0, 0.6);

/// All the scrolling done, in wheel steps: y down the page, x to the right.
fn scrolled(inputs: &[Input]) -> Vec2 {
    inputs
        .iter()
        .filter_map(|i| match i {
            Input::Scroll(by) => Some(*by),
            _ => None,
        })
        .sum()
}

#[test]
fn a_vee_raised_above_where_it_began_scrolls_up() {
    let up = AHEAD + Vec3::new(0.0, 0.1, 0.0);
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Vee, 10)
        .sweep(AHEAD, up, Grip::Vee, 10)
        .hold(up, Grip::Vee, 30)
        .run();
    let by = scrolled(&inputs);
    assert!(by.y < -3.0 && by.x.abs() < 0.5, "scrolled {by}");
    assert_eq!(buttons(&inputs), vec![]);
}

#[test]
fn a_vee_lowered_scrolls_down_and_one_moved_right_scrolls_right() {
    let down = AHEAD + Vec3::new(0.0, -0.1, 0.0);
    let right = AHEAD + Vec3::new(0.1, 0.0, 0.0);
    let vee_to = |to: Vec3| {
        Script::default()
            .hold(AHEAD, Grip::Open, 20)
            .hold(AHEAD, Grip::Vee, 10)
            .sweep(AHEAD, to, Grip::Vee, 10)
            .hold(to, Grip::Vee, 30)
            .run()
    };
    let by = scrolled(&vee_to(down));
    assert!(by.y > 3.0 && by.x.abs() < 0.5, "lowered: scrolled {by}");
    let by = scrolled(&vee_to(right));
    assert!(by.x > 3.0 && by.y.abs() < 0.5, "moved right: scrolled {by}");
}

#[test]
fn a_vee_that_strays_within_the_dead_zone_doesnt_scroll() {
    let near = AHEAD + Vec3::new(0.008, 0.008, 0.0);
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Vee, 10)
        .sweep(AHEAD, near, Grip::Vee, 5)
        .hold(near, Grip::Vee, 30)
        .run();
    assert_eq!(scrolled(&inputs), Vec2::ZERO);
}

#[test]
fn scrolling_stops_when_the_vee_comes_back_to_where_it_began() {
    let up = AHEAD + Vec3::new(0.0, 0.1, 0.0);
    let script = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Vee, 10)
        .sweep(AHEAD, up, Grip::Vee, 10)
        .sweep(up, AHEAD, Grip::Vee, 10);
    let before = script.run().len();
    let inputs = script.hold(AHEAD, Grip::Vee, 30).run();
    // Smoothing takes a few frames to settle back.
    assert_eq!(scrolled(&inputs[before + 20..]), Vec2::ZERO);
}

#[test]
fn the_pointer_holds_still_while_the_vee_scrolls() {
    let up = AHEAD + Vec3::new(0.05, 0.1, 0.0);
    let script = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Vee, 10);
    let at_start = last_point(&script.run());
    let inputs = script.sweep(AHEAD, up, Grip::Vee, 10).hold(up, Grip::Vee, 10).run();
    assert_eq!(last_point(&inputs), at_start);
}

#[test]
fn the_other_hand_pinching_and_dragging_doesnt_scroll() {
    // The two-hand pinch-drag scroll is gone: the Vee replaces it.
    let pointer = Vec3::new(-0.05, 0.0, 0.6);
    let other = Vec3::new(0.12, -0.05, 0.6);
    let up = other + Vec3::new(0.0, 0.1, 0.0);
    let inputs = Script::default()
        .hold(pointer, Grip::Open, 20)
        .hold_two((pointer, Grip::Open), (other, Grip::Open), 10)
        .hold_two((pointer, Grip::Open), (other, Grip::Pinch), 6)
        .sweep_two(pointer, (other, up), Grip::Pinch, 10)
        .run();
    assert_eq!(scrolled(&inputs), Vec2::ZERO);
}
