//! Two open hands brought together, then spread apart quickly, toggle full
//! screen (Super + F). It needs no hand to have control.

use crate::support::{Grip, Script, buttons};
use glam::Vec3;
use terrain_desktop::{Button, Input};

/// Low in front of the chest, where the recorded take held them: below the
/// Reach, so neither hand takes control.
const LOW: Vec3 = Vec3::new(0.0, -0.15, 0.6);
/// Palms 16 cm apart: together.
const TOGETHER: Vec3 = Vec3::new(0.08, 0.0, 0.0);
/// Palms 50 cm apart: spread.
const SPREAD: Vec3 = Vec3::new(0.25, 0.0, 0.0);

fn full_screen() -> Vec<Input> {
    vec![
        Input::Press(Button::Super),
        Input::Press(Button::F),
        Input::Release(Button::F),
        Input::Release(Button::Super),
    ]
}

#[test]
fn two_open_hands_brought_together_then_spread_toggle_full_screen() {
    let inputs = Script::default()
        .hold_two(
            (LOW - TOGETHER, Grip::Open),
            (LOW + TOGETHER, Grip::Open),
            10,
        )
        .sweep_both(
            (LOW - TOGETHER, LOW - SPREAD),
            (LOW + TOGETHER, LOW + SPREAD),
            Grip::Open,
            12,
        )
        .hold_two((LOW - SPREAD, Grip::Open), (LOW + SPREAD, Grip::Open), 10)
        .run();
    assert_eq!(buttons(&inputs), full_screen());
}

#[test]
fn hands_lifted_apart_and_stretched_wider_do_nothing() {
    // Resting hands sit about 27 cm apart: not together.
    let rest = Vec3::new(0.135, 0.0, 0.0);
    let inputs = Script::default()
        .hold_two((LOW - rest, Grip::Open), (LOW + rest, Grip::Open), 10)
        .sweep_both(
            (LOW - rest, LOW - SPREAD),
            (LOW + rest, LOW + SPREAD),
            Grip::Open,
            8,
        )
        .hold_two((LOW - SPREAD, Grip::Open), (LOW + SPREAD, Grip::Open), 10)
        .run();
    assert_eq!(buttons(&inputs), vec![]);
}

#[test]
fn hands_drifting_apart_slowly_after_a_clap_do_nothing() {
    let inputs = Script::default()
        .hold_two(
            (LOW - TOGETHER, Grip::Open),
            (LOW + TOGETHER, Grip::Open),
            10,
        )
        .sweep_both(
            (LOW - TOGETHER, LOW - SPREAD),
            (LOW + TOGETHER, LOW + SPREAD),
            Grip::Open,
            45,
        )
        .run();
    assert_eq!(buttons(&inputs), vec![]);
}

#[test]
fn each_spread_toggles_once_however_the_spread_hands_wobble() {
    let wider = SPREAD + Vec3::new(0.05, 0.0, 0.0);
    let spread_once = |script: Script| {
        script
            .hold_two(
                (LOW - TOGETHER, Grip::Open),
                (LOW + TOGETHER, Grip::Open),
                10,
            )
            .sweep_both(
                (LOW - TOGETHER, LOW - SPREAD),
                (LOW + TOGETHER, LOW + SPREAD),
                Grip::Open,
                12,
            )
            .sweep_both(
                (LOW - SPREAD, LOW - wider),
                (LOW + SPREAD, LOW + wider),
                Grip::Open,
                4,
            )
            .sweep_both(
                (LOW - wider, LOW - SPREAD),
                (LOW + wider, LOW + SPREAD),
                Grip::Open,
                4,
            )
            .sweep_both(
                (LOW - SPREAD, LOW - TOGETHER),
                (LOW + SPREAD, LOW + TOGETHER),
                Grip::Open,
                15,
            )
    };
    let inputs = spread_once(spread_once(Script::default())).run();
    assert_eq!(buttons(&inputs), [full_screen(), full_screen()].concat());
}

#[test]
fn a_spread_while_the_other_hand_drags_does_nothing() {
    let ahead = Vec3::new(0.0, 0.0, 0.6);
    let beside = ahead + Vec3::new(0.16, 0.0, 0.0);
    let mut script = Script::default().hold(ahead, Grip::Open, 20).hold_two(
        (ahead, Grip::Pinch),
        (beside, Grip::Open),
        15,
    );
    // The pinch drags on while the open hand flies off to the side.
    for i in 1..=12 {
        let s = i as f32 / 12.0;
        let dragged = ahead.lerp(ahead - Vec3::new(0.05, 0.0, 0.0), s);
        script = script.hold_two(
            (dragged, Grip::Pinch),
            (beside.lerp(ahead + SPREAD * 2.0, s), Grip::Open),
            1,
        );
    }
    let inputs = script.run();
    assert!(
        !buttons(&inputs).contains(&Input::Press(Button::F)),
        "{inputs:?}"
    );
}

#[test]
fn a_hand_lost_for_a_moment_at_the_edge_mid_spread_still_toggles() {
    let mut script = Script::default()
        .hold_two(
            (LOW - TOGETHER, Grip::Open),
            (LOW + TOGETHER, Grip::Open),
            10,
        )
        .sweep_both(
            (LOW - TOGETHER, LOW - TOGETHER * 2.0),
            (LOW + TOGETHER, LOW + TOGETHER * 2.0),
            Grip::Open,
            3,
        );
    // The flung hand leaves the tracker for a few frames.
    for _ in 0..6 {
        script = script.hold(LOW + TOGETHER * 2.0, Grip::Open, 1);
    }
    let inputs = script
        .hold_two((LOW - SPREAD, Grip::Open), (LOW + SPREAD, Grip::Open), 10)
        .run();
    assert_eq!(buttons(&inputs), full_screen());
}
