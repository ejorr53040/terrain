mod support;

use bevy::prelude::*;
use support::*;
use terrain_app::{CUBE_STARTS, GRAB_GAIN};

/// The app's two cubes, left and right of the middle.
const LEFT_CUBE: Vec3 = CUBE_STARTS[0];
const RIGHT_CUBE: Vec3 = CUBE_STARTS[1];
/// Hands held out in front of the camera, one on each side of the view.
const LEFT_HAND: Vec3 = Vec3::new(-0.12, 0.0, 0.5);
const RIGHT_HAND: Vec3 = Vec3::new(0.12, 0.0, 0.5);
/// A hand's move up or down...
const UP: Vec3 = Vec3::new(0.0, 0.10, 0.0);
const DOWN: Vec3 = Vec3::new(0.0, -0.10, 0.0);
/// ...and how far that moves the cube it holds.
const GAIN_UP: Vec3 = Vec3::new(0.0, GRAB_GAIN * 0.10, 0.0);

#[test]
fn each_hand_moves_its_own_cube_at_the_same_time() {
    // Both hands read as the same hand, as the tracker often has it.
    let mut h = Harness::with_cubes(
        [
            hold(&[Pose::open(LEFT_HAND), Pose::open(RIGHT_HAND)], SETTLE),
            hold(
                &[Pose::pinched(LEFT_HAND), Pose::pinched(RIGHT_HAND)],
                SETTLE,
            ),
            hold(
                &[
                    Pose::pinched(LEFT_HAND + UP),
                    Pose::pinched(RIGHT_HAND + DOWN),
                ],
                SETTLE,
            ),
            hold(
                &[Pose::open(LEFT_HAND + UP), Pose::open(RIGHT_HAND + DOWN)],
                SETTLE,
            ),
        ]
        .concat(),
        &[LEFT_CUBE, RIGHT_CUBE],
    );
    h.run_all();
    assert_near(h.cube_at(0), LEFT_CUBE + GAIN_UP);
    assert_near(h.cube_at(1), RIGHT_CUBE - GAIN_UP);
}

#[test]
fn a_pinch_takes_the_free_cube_nearest_the_hand_on_screen() {
    for (hand, taken, left_alone) in [(RIGHT_HAND, 1, 0), (LEFT_HAND, 0, 1)] {
        let mut h = Harness::with_cubes(
            [
                hold(&[Pose::open(hand)], SETTLE),
                hold(&[Pose::pinched(hand)], SETTLE),
                hold(&[Pose::pinched(hand + UP)], SETTLE),
            ]
            .concat(),
            &[LEFT_CUBE, RIGHT_CUBE],
        );
        h.run_all();
        let start = [LEFT_CUBE, RIGHT_CUBE];
        assert_near(h.cube_at(taken), start[taken] + GAIN_UP);
        assert_near(h.cube_at(left_alone), start[left_alone]);
    }
}

#[test]
fn a_cube_already_held_is_passed_over_for_the_free_one() {
    // The left hand holds the left cube; the right hand then pinches right
    // beside it, and gets the other cube.
    let beside = LEFT_HAND + Vec3::new(0.04, 0.0, 0.0);
    let mut h = Harness::with_cubes(
        [
            hold(&[Pose::pinched(LEFT_HAND)], SETTLE),
            hold(
                &[Pose::pinched(LEFT_HAND), Pose::open(beside + DOWN)],
                SETTLE,
            ),
            hold(
                &[Pose::pinched(LEFT_HAND), Pose::pinched(beside + DOWN)],
                SETTLE,
            ),
            hold(&[Pose::pinched(LEFT_HAND), Pose::pinched(beside)], SETTLE),
        ]
        .concat(),
        &[LEFT_CUBE, RIGHT_CUBE],
    );
    h.run_all();
    assert_near(h.cube_at(0), LEFT_CUBE);
    assert_near(h.cube_at(1), RIGHT_CUBE + GAIN_UP);
}

#[test]
fn r_puts_both_cubes_back_and_lets_go_of_both() {
    let mut h = Harness::with_cubes(
        [
            hold(
                &[Pose::pinched(LEFT_HAND), Pose::pinched(RIGHT_HAND)],
                SETTLE,
            ),
            hold(
                &[
                    Pose::pinched(LEFT_HAND + UP),
                    Pose::pinched(RIGHT_HAND + UP),
                ],
                SETTLE,
            ),
            hold(
                &[
                    Pose::pinched(LEFT_HAND + DOWN),
                    Pose::pinched(RIGHT_HAND + DOWN),
                ],
                SETTLE,
            ),
        ]
        .concat(),
        &[LEFT_CUBE, RIGHT_CUBE],
    );
    h.run(2 * SETTLE);
    h.press(KeyCode::KeyR).run_all();
    assert_near(h.cube_at(0), LEFT_CUBE);
    assert_near(h.cube_at(1), RIGHT_CUBE);
}
