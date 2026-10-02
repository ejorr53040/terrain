mod support;

use bevy::prelude::*;
use support::*;

const START: Vec3 = Vec3::new(0.0, 0.0, 0.5);

#[test]
fn pinch_drag_release_moves_cube_by_gain_times_hand_motion() {
    let moved = START + Vec3::new(0.10, -0.04, 0.0);
    let frames = vec![
        frame(&[Pose::open(START)]),
        frame(&[Pose::pinched(START)]),
        frame(&[Pose::pinched(moved)]),
        frame(&[Pose::open(moved)]),
        frame(&[Pose::open(moved + Vec3::new(0.10, 0.0, 0.0))]),
    ];
    let mut h = Harness::new(frames);
    h.run(5);
    assert_near(h.cube().translation, Vec3::new(0.15, -0.06, 0.0));
}

#[test]
fn a_half_closed_pinch_neither_grabs_nor_drops() {
    let step = Vec3::new(0.10, 0.0, 0.0);
    let frames = vec![
        // Hovering at 4 cm from open: no grab, so this motion is ignored.
        frame(&[Pose::gap(START, 0.04)]),
        frame(&[Pose::gap(START + step, 0.04)]),
        // Close the pinch, then relax to 4 cm while moving: still held.
        frame(&[Pose::pinched(START + step)]),
        frame(&[Pose::gap(START + 2.0 * step, 0.04)]),
        // Open past 5 cm: dropped, so this motion is ignored.
        frame(&[Pose::gap(START + 2.0 * step, 0.06)]),
        frame(&[Pose::gap(START + 3.0 * step, 0.06)]),
    ];
    let mut h = Harness::new(frames);
    h.run(6);
    assert_near(h.cube().translation, Vec3::new(0.15, 0.0, 0.0));
}

#[test]
fn releasing_and_regrabbing_carries_the_cube_farther_than_one_reach() {
    let reach = Vec3::new(0.10, 0.0, 0.0);
    let frames = vec![
        frame(&[Pose::pinched(START)]),
        frame(&[Pose::pinched(START + reach)]),
        frame(&[Pose::open(START + reach)]),
        frame(&[Pose::open(START)]),
        frame(&[Pose::pinched(START)]),
        frame(&[Pose::pinched(START + reach)]),
        frame(&[Pose::open(START + reach)]),
    ];
    let mut h = Harness::new(frames);
    h.run(7);
    assert_near(h.cube().translation, Vec3::new(0.30, 0.0, 0.0));
}

#[test]
fn demo_fixture_drags_the_cube_around_a_circle_and_back() {
    let mut h = Harness::with_source(
        terrain_hands::ReplaySource::from_json_file(terrain_app::DEMO_FIXTURE).unwrap(),
    );
    // 30 open frames, the pinch, then half of the 120-frame circle.
    h.run(30 + 1 + 60);
    assert_near(h.cube().translation, Vec3::new(-0.24, 0.0, 0.0));
    h.run(60 + 30);
    assert_near(h.cube().translation, Vec3::ZERO);
}
