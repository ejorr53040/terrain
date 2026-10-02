mod support;

use bevy::prelude::*;
use support::*;

const START: Vec3 = Vec3::new(0.0, 0.0, 0.5);

#[test]
fn pinch_drag_release_moves_cube_by_gain_times_hand_motion() {
    let moved = START + Vec3::new(0.10, -0.04, 0.0);
    let mut h = Harness::new(
        [
            hold(&[Pose::open(START)], SETTLE),
            hold(&[Pose::pinched(START)], SETTLE),
            hold(&[Pose::pinched(moved)], SETTLE),
            hold(&[Pose::open(moved)], SETTLE),
            hold(&[Pose::open(moved + Vec3::new(0.10, 0.0, 0.0))], SETTLE),
        ]
        .concat(),
    );
    h.run_all();
    assert_near(h.cube().translation, Vec3::new(0.15, -0.06, 0.0));
}

#[test]
fn a_half_closed_pinch_neither_grabs_nor_drops() {
    let step = Vec3::new(0.10, 0.0, 0.0);
    let mut h = Harness::new(
        [
            // Hovering at 4 cm from open: no grab, so this motion is ignored.
            hold(&[Pose::gap(START, 0.04)], SETTLE),
            hold(&[Pose::gap(START + step, 0.04)], SETTLE),
            // Close the pinch, then relax to 4 cm while moving: still held.
            hold(&[Pose::pinched(START + step)], SETTLE),
            hold(&[Pose::gap(START + 2.0 * step, 0.04)], SETTLE),
            // Open past 6.5 cm: dropped, so this motion is ignored.
            hold(&[Pose::gap(START + 2.0 * step, 0.08)], SETTLE),
            hold(&[Pose::gap(START + 3.0 * step, 0.08)], SETTLE),
        ]
        .concat(),
    );
    h.run_all();
    assert_near(h.cube().translation, Vec3::new(0.15, 0.0, 0.0));
}

#[test]
fn releasing_and_regrabbing_carries_the_cube_farther_than_one_reach() {
    let reach = Vec3::new(0.10, 0.0, 0.0);
    let mut h = Harness::new(
        [
            hold(&[Pose::pinched(START)], SETTLE),
            hold(&[Pose::pinched(START + reach)], SETTLE),
            hold(&[Pose::open(START + reach)], SETTLE),
            hold(&[Pose::open(START)], SETTLE),
            hold(&[Pose::pinched(START)], SETTLE),
            hold(&[Pose::pinched(START + reach)], SETTLE),
            hold(&[Pose::open(START + reach)], SETTLE),
        ]
        .concat(),
    );
    h.run_all();
    assert_near(h.cube().translation, Vec3::new(0.30, 0.0, 0.0));
}

#[test]
fn demo_fixture_drags_the_cube_around_a_circle_and_back() {
    let mut h = Harness::with_source(
        terrain_hands::ReplaySource::from_json_file(terrain_app::DEMO_FIXTURE).unwrap(),
    );
    // 30 open frames, the pinch, the 120-frame circle, a 15-frame pause,
    // then 30 open frames.
    h.run(30 + 1 + 60);
    let halfway = h.cube().translation;
    assert!(
        halfway.x < -0.15,
        "cube should be well left of start halfway round, got {halfway}"
    );
    h.run(60 + 15 + 30);
    assert_near(h.cube().translation, Vec3::ZERO);
}
