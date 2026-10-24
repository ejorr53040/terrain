//! Two Vees brought together, then spread apart quickly, toggle Omarchy's
//! dictation (Super + Ctrl + X). Like full screen's Spread, it needs no hand
//! to have control.

use crate::support::{Grip, Script, buttons};
use glam::Vec3;
use terrain_desktop::{Button, Input};

/// Low in front of the chest: below the Reach, so neither hand takes control.
const LOW: Vec3 = Vec3::new(0.0, -0.15, 0.6);
/// Palms 16 cm apart.
const TOGETHER: Vec3 = Vec3::new(0.08, 0.0, 0.0);
/// Palms 50 cm apart.
const APART: Vec3 = Vec3::new(0.25, 0.0, 0.0);

fn dictation() -> Vec<Input> {
    vec![
        Input::Press(Button::Super),
        Input::Press(Button::Ctrl),
        Input::Press(Button::X),
        Input::Release(Button::X),
        Input::Release(Button::Ctrl),
        Input::Release(Button::Super),
    ]
}

fn spread(at: Vec3, grip: Grip) -> Script {
    Script::default()
        .hold_two((at - TOGETHER, grip), (at + TOGETHER, grip), 10)
        .sweep_both(
            (at - TOGETHER, at - APART),
            (at + TOGETHER, at + APART),
            grip,
            12,
        )
        .hold_two((at - APART, grip), (at + APART, grip), 10)
}

#[test]
fn two_vees_brought_together_then_spread_toggle_dictation() {
    assert_eq!(buttons(&spread(LOW, Grip::Vee).run()), dictation());
}

#[test]
fn two_vees_held_apart_do_nothing() {
    // Resting hands sit about 27 cm apart: not together.
    let rest = Vec3::new(0.135, 0.0, 0.0);
    let inputs = Script::default()
        .hold_two((LOW - rest, Grip::Vee), (LOW + rest, Grip::Vee), 10)
        .sweep_both(
            (LOW - rest, LOW - APART),
            (LOW + rest, LOW + APART),
            Grip::Vee,
            8,
        )
        .hold_two((LOW - APART, Grip::Vee), (LOW + APART, Grip::Vee), 10)
        .run();
    assert_eq!(buttons(&inputs), vec![]);
}

#[test]
fn a_vee_spread_with_a_hand_in_control_toggles_dictation_without_scrolling() {
    // Raised into the Reach, the first hand takes control, then both make
    // Vees as they come together.
    let chest = Vec3::new(0.0, 0.0, 0.6);
    let rest = Vec3::new(0.135, 0.0, 0.0);
    let inputs = Script::default()
        .hold_two((chest - rest, Grip::Open), (chest + rest, Grip::Open), 20)
        .hold_two((chest - rest, Grip::Vee), (chest + rest, Grip::Vee), 3)
        .sweep_both(
            (chest - rest, chest - TOGETHER),
            (chest + rest, chest + TOGETHER),
            Grip::Vee,
            8,
        )
        .hold_two(
            (chest - TOGETHER, Grip::Vee),
            (chest + TOGETHER, Grip::Vee),
            10,
        )
        .sweep_both(
            (chest - TOGETHER, chest - APART),
            (chest + TOGETHER, chest + APART),
            Grip::Vee,
            12,
        )
        .hold_two((chest - APART, Grip::Vee), (chest + APART, Grip::Vee), 10)
        .run();
    assert!(
        inputs.iter().any(|i| matches!(i, Input::PointTo(_))),
        "no hand took control: {inputs:?}"
    );
    assert!(
        !inputs.iter().any(|i| matches!(i, Input::Scroll(_))),
        "{inputs:?}"
    );
    assert_eq!(buttons(&inputs), dictation());
}

#[test]
fn palms_meeting_open_then_spreading_as_vees_toggle_dictation_only() {
    let inputs = Script::default()
        .hold_two(
            (LOW - TOGETHER, Grip::Open),
            (LOW + TOGETHER, Grip::Open),
            10,
        )
        .hold_two((LOW - TOGETHER, Grip::Vee), (LOW + TOGETHER, Grip::Vee), 6)
        .sweep_both(
            (LOW - TOGETHER, LOW - APART),
            (LOW + TOGETHER, LOW + APART),
            Grip::Vee,
            4,
        )
        .hold_two((LOW - APART, Grip::Open), (LOW + APART, Grip::Open), 10)
        .run();
    assert_eq!(buttons(&inputs), dictation());
}

#[test]
fn palms_meeting_as_vees_then_spreading_open_toggle_full_screen_only() {
    let inputs = Script::default()
        .hold_two((LOW - TOGETHER, Grip::Vee), (LOW + TOGETHER, Grip::Vee), 10)
        .hold_two(
            (LOW - TOGETHER, Grip::Open),
            (LOW + TOGETHER, Grip::Open),
            6,
        )
        .sweep_both(
            (LOW - TOGETHER, LOW - APART),
            (LOW + TOGETHER, LOW + APART),
            Grip::Open,
            4,
        )
        .hold_two((LOW - APART, Grip::Vee), (LOW + APART, Grip::Vee), 10)
        .run();
    assert_eq!(
        buttons(&inputs),
        vec![
            Input::Press(Button::Super),
            Input::Press(Button::F),
            Input::Release(Button::F),
            Input::Release(Button::Super),
        ]
    );
}
