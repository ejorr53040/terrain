//! A fist or pinch flicked quickly in a direction sends one of herdr's
//! key chords: a fist moves the focus between panes, a pinch steps
//! through tabs (left, right) and workspaces (up, down).

mod support;

use glam::Vec3;
use support::{Grip, Script, buttons};
use terrain_desktop::{Button, Input, Status};

const AHEAD: Vec3 = Vec3::new(0.0, 0.0, 0.6);

#[test]
fn a_fist_flicked_left_focuses_the_pane_to_the_left() {
    let inputs = flick(Grip::Fist, Vec3::new(-0.12, 0.0, 0.0));
    assert_eq!(buttons(&inputs), chord(&[Button::Ctrl, Button::Alt, Button::H]));
}

#[test]
fn a_fist_flicked_each_way_focuses_the_pane_that_way() {
    let ways = [
        (Vec3::new(0.12, 0.0, 0.0), Button::L),
        (Vec3::new(0.0, 0.12, 0.0), Button::K),
        (Vec3::new(0.0, -0.12, 0.0), Button::J),
    ];
    for (by, key) in ways {
        let inputs = flick(Grip::Fist, by);
        assert_eq!(buttons(&inputs), chord(&[Button::Ctrl, Button::Alt, key]), "flicked {by}");
    }
}

#[test]
fn a_pinch_flicked_sideways_steps_through_tabs_and_up_or_down_through_workspaces() {
    let ways = [
        (Vec3::new(-0.12, 0.0, 0.0), Button::ArrowLeft),
        (Vec3::new(0.12, 0.0, 0.0), Button::ArrowRight),
        (Vec3::new(0.0, 0.12, 0.0), Button::ArrowUp),
        (Vec3::new(0.0, -0.12, 0.0), Button::ArrowDown),
    ];
    for (by, key) in ways {
        let inputs = flick(Grip::Pinch, by);
        assert_eq!(buttons(&inputs), chord(&[Button::Alt, key]), "flicked {by}");
    }
}

#[test]
fn a_slow_pinch_drag_still_drags() {
    let to = AHEAD + Vec3::new(-0.12, 0.0, 0.0);
    let inputs = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Pinch, 5)
        .sweep(AHEAD, to, Grip::Pinch, 15)
        .hold(to, Grip::Open, 10)
        .run();
    assert_eq!(
        buttons(&inputs),
        vec![Input::Press(Button::Left), Input::Release(Button::Left)]
    );
}

#[test]
fn a_flick_shows_the_command_until_the_hand_opens() {
    let by = Vec3::new(0.0, 0.12, 0.0);
    let status = Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, Grip::Fist, 5)
        .sweep(AHEAD, AHEAD + by, Grip::Fist, 3)
        .hold(AHEAD + by, Grip::Fist, 5)
        .status();
    assert_eq!(status, Status::Gesture("✊ ↑ Pane".into()));
}

/// The hand raised open to take control, closed into `grip`, flicked by
/// `by` (meters) in about a tenth of a second, then opened.
fn flick(grip: Grip, by: Vec3) -> Vec<Input> {
    Script::default()
        .hold(AHEAD, Grip::Open, 20)
        .hold(AHEAD, grip, 5)
        .sweep(AHEAD, AHEAD + by, grip, 3)
        .hold(AHEAD + by, grip, 5)
        .hold(AHEAD + by, Grip::Open, 10)
        .run()
}

/// The keys pressed in order, then released in reverse.
fn chord(keys: &[Button]) -> Vec<Input> {
    let presses = keys.iter().map(|&k| Input::Press(k));
    let releases = keys.iter().rev().map(|&k| Input::Release(k));
    presses.chain(releases).collect()
}
