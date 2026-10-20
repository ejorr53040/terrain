//! While one hand points, the other pinches and drags to scroll, like
//! grabbing the page and pulling it.

mod support;

use glam::{Vec2, Vec3};
use support::{Grip, Script, buttons};
use terrain_desktop::Input;

const POINTER: Vec3 = Vec3::new(-0.05, 0.0, 0.6);
const OTHER: Vec3 = Vec3::new(0.12, -0.05, 0.6);

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
fn pulling_the_page_up_with_the_other_hand_scrolls_down() {
    let up = OTHER + Vec3::new(0.0, 0.1, 0.0);
    let inputs = Script::default()
        .hold(POINTER, Grip::Open, 20)
        .hold_two((POINTER, Grip::Open), (OTHER, Grip::Open), 10)
        .hold_two((POINTER, Grip::Open), (OTHER, Grip::Pinch), 6)
        .sweep_two(POINTER, (OTHER, up), Grip::Pinch, 10)
        .hold_two((POINTER, Grip::Open), (up, Grip::Open), 10)
        .run();
    let by = scrolled(&inputs);
    assert!(by.y > 3.0 && by.x.abs() < 0.5, "scrolled {by}");
    assert_eq!(buttons(&inputs), vec![]);
}

#[test]
fn the_other_hand_moving_open_doesnt_scroll() {
    let up = OTHER + Vec3::new(0.0, 0.1, 0.0);
    let inputs = Script::default()
        .hold(POINTER, Grip::Open, 20)
        .hold_two((POINTER, Grip::Open), (OTHER, Grip::Open), 10)
        .sweep_two(POINTER, (OTHER, up), Grip::Open, 10)
        .run();
    assert_eq!(scrolled(&inputs), Vec2::ZERO);
}

#[test]
fn the_other_hand_cant_scroll_until_one_has_control() {
    let up = OTHER + Vec3::new(0.0, 0.1, 0.0);
    let inputs = Script::default().sweep(OTHER, up, Grip::Pinch, 20).run();
    assert_eq!(scrolled(&inputs), Vec2::ZERO);
}
