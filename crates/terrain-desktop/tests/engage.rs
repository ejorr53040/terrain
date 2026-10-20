//! A hand takes control of the desktop only when it's raised open into the
//! reach, so hands at the keyboard don't click as they type.

mod support;

use glam::Vec3;
use support::{Grip, Script, buttons};
use terrain_desktop::{Button, Input};

const AHEAD: Vec3 = Vec3::new(0.0, 0.0, 0.6);
/// Low and to the side, past the reach: where hands at the keyboard show.
const AT_KEYBOARD: Vec3 = Vec3::new(-0.2, -0.17, 0.6);

#[test]
fn a_hand_at_the_keyboard_never_touches_the_desktop() {
    let inputs = Script::default()
        .hold(AT_KEYBOARD, Grip::Open, 30)
        .hold(AT_KEYBOARD, Grip::Pinch, 10)
        .hold(AT_KEYBOARD, Grip::Fist, 10)
        .hold(AT_KEYBOARD, Grip::Open, 10)
        .run();
    assert_eq!(inputs, vec![]);
}

#[test]
fn a_hand_pinching_as_it_rises_into_reach_doesnt_click_until_raised_open() {
    let inputs = Script::default()
        .sweep(AT_KEYBOARD, AHEAD, Grip::Pinch, 10)
        .hold(AHEAD, Grip::Pinch, 20)
        .run();
    assert_eq!(inputs, vec![]);
}

#[test]
fn a_hand_raised_open_for_a_moment_takes_control() {
    let inputs = Script::default().hold(AHEAD, Grip::Open, 5).run();
    assert_eq!(inputs, vec![], "too soon");
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Pinch, 8)
        .hold(AHEAD, Grip::Open, 5)
        .run();
    assert_eq!(
        buttons(&inputs),
        vec![Input::Press(Button::Left), Input::Release(Button::Left)]
    );
}

#[test]
fn a_hand_lowered_back_to_the_keyboard_gives_up_control() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .sweep(AHEAD, AT_KEYBOARD, Grip::Open, 5)
        .hold(AT_KEYBOARD, Grip::Open, 40)
        .hold(AT_KEYBOARD, Grip::Pinch, 10)
        .run();
    assert_eq!(buttons(&inputs), vec![]);
}

#[test]
fn the_other_hand_cant_click_while_one_has_control() {
    let other = AHEAD + Vec3::new(0.15, 0.0, 0.0);
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold_two((other, Grip::Open), (AHEAD, Grip::Open), 20)
        .hold_two((other, Grip::Pinch), (AHEAD, Grip::Open), 10)
        .run();
    assert_eq!(buttons(&inputs), vec![]);
}

#[test]
fn a_hand_curling_as_it_lowers_out_of_reach_doesnt_click() {
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .sweep(AHEAD, AT_KEYBOARD, Grip::Open, 4)
        .hold(AT_KEYBOARD, Grip::Pinch, 10)
        .hold(AT_KEYBOARD, Grip::Fist, 10)
        .run();
    assert_eq!(buttons(&inputs), vec![]);
}

#[test]
fn time_running_backwards_does_nothing_strange() {
    let mut script = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Pinch, 3);
    // A clip spliced onto another: its clock starts over.
    let restarted = Script::default()
        .hold(AHEAD, Grip::Pinch, 3)
        .hold(AHEAD, Grip::Open, 5);
    script.frames.extend(restarted.frames);
    let inputs = script.run();
    assert!(buttons(&inputs).len() <= 2, "{:?}", buttons(&inputs));
}
