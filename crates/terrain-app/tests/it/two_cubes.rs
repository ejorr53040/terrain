use crate::support::*;
use bevy::prelude::*;
use terrain_app::CUBE_STARTS;

/// A scene of two cubes, left and right of the middle.
const LEFT_CUBE: Vec3 = Vec3::new(-0.25, 0.0, 0.0);
const RIGHT_CUBE: Vec3 = Vec3::new(0.25, 0.0, 0.0);
/// Hands held out in front of the camera, one on each side of the view.
const LEFT_HAND: Vec3 = Vec3::new(-0.12, 0.0, 0.5);
const RIGHT_HAND: Vec3 = Vec3::new(0.12, 0.0, 0.5);
/// A hand's move up or down, small enough that the cube it carries stays clear of the box.
const UP: Vec3 = Vec3::new(0.0, 0.05, 0.0);
const DOWN: Vec3 = Vec3::new(0.0, -0.05, 0.0);

/// Asserts a cube that started at `start` was carried straight along `hand_moved`,
/// at least as far as the hand went (the scene amplifies hand motion). Roughly
/// straight: it follows its cursor on screen, and the camera looks down at it.
fn assert_carried(actual: Vec3, start: Vec3, hand_moved: Vec3) {
    let moved = actual - start;
    assert!(
        moved.normalize().abs_diff_eq(hand_moved.normalize(), 0.1)
            && moved.length() >= hand_moved.length(),
        "cube from {start} should be carried along {hand_moved}, is at {actual}"
    );
}

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
    assert_carried(h.cube_at(0), LEFT_CUBE, UP);
    assert_carried(h.cube_at(1), RIGHT_CUBE, DOWN);
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
        assert_carried(h.cube_at(taken), start[taken], UP);
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
    assert_carried(h.cube_at(1), RIGHT_CUBE, UP);
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

#[test]
fn across_the_apps_row_of_five_each_cube_is_grabbed_by_the_hand_in_front_of_it() {
    assert_eq!(CUBE_STARTS.len(), 5, "the app has five cubes");
    // Hand places across the view that line up, on screen, with the five
    // cubes in their row.
    let hands_x = [-0.14, -0.07, 0.0, 0.07, 0.14];
    for (i, x) in hands_x.into_iter().enumerate() {
        let hand = Vec3::new(x, 0.0, 0.5);
        let mut h = Harness::with_cubes(
            [
                hold(&[Pose::open(hand)], SETTLE),
                hold(&[Pose::pinched(hand)], SETTLE),
                hold(&[Pose::pinched(hand + UP)], SETTLE),
            ]
            .concat(),
            &CUBE_STARTS,
        );
        h.run_all();
        for (j, start) in CUBE_STARTS.into_iter().enumerate() {
            if j == i {
                assert_carried(h.cube_at(j), start, UP);
            } else {
                assert_near(h.cube_at(j), start);
            }
        }
    }
}
