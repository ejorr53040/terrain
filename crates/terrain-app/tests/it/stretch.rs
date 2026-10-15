
use bevy::prelude::*;
use crate::support::*;
use terrain_app::CUBE_SIZE;

/// One hand holds the cube; the other then pinches with its cursor on the
/// cube (in the middle of the view, where the cube is). Both then move to
/// `apart`, keeping their midpoint, so the cube should stay put.
fn two_hand_pull(first: Vec3, second: Vec3, apart: (Vec3, Vec3)) -> Harness {
    let mut h = Harness::with_cubes(
        [
            hold(&[Pose::pinched(first)], SETTLE),
            hold(&[Pose::pinched(first), Pose::open(second)], SETTLE),
            hold(&[Pose::pinched(first), Pose::pinched(second)], SETTLE),
            hold(&[Pose::pinched(apart.0), Pose::pinched(apart.1)], SETTLE),
        ]
        .concat(),
        &[Vec3::ZERO],
    );
    h.run_all();
    h
}

/// Asserts the cube's size, as a multiple of its starting size, and that it
/// got there by its size, not by scaling.
fn assert_stretched(h: &Harness, expected: Vec3) {
    let actual = h.shape().size / CUBE_SIZE;
    assert!(
        actual.abs_diff_eq(expected, 0.1),
        "expected size {expected} times a cube, got {actual}"
    );
    assert_eq!(h.cube().scale, Vec3::ONE, "a stretch mustn't scale");
}

#[test]
fn pulling_two_hands_apart_on_one_cube_stretches_it_along_the_axis_between_them() {
    // Hands 8 cm apart, then 16 cm: twice the gap, twice as long that way.
    let across = two_hand_pull(
        Vec3::new(-0.06, 0.0, 0.5),
        Vec3::new(0.02, 0.0, 0.5),
        (Vec3::new(-0.10, 0.0, 0.5), Vec3::new(0.06, 0.0, 0.5)),
    );
    assert_stretched(&across, Vec3::new(2.0, 1.0, 1.0));
    assert_near(across.cube().translation, Vec3::ZERO);

    let up = two_hand_pull(
        Vec3::new(0.0, -0.06, 0.5),
        Vec3::new(0.01, 0.02, 0.5),
        (Vec3::new(0.0, -0.10, 0.5), Vec3::new(0.01, 0.06, 0.5)),
    );
    assert_stretched(&up, Vec3::new(1.0, 2.0, 1.0));

    // One hand nearer the camera than the other.
    let deep = two_hand_pull(
        Vec3::new(-0.02, 0.0, 0.42),
        Vec3::new(0.0, 0.0, 0.50),
        (Vec3::new(-0.02, 0.0, 0.38), Vec3::new(0.0, 0.0, 0.54)),
    );
    // Gap 8.2 cm, then 16.1 cm.
    assert_stretched(&deep, Vec3::new(1.0, 1.0, 1.96));
}

#[test]
fn pushing_two_hands_together_shrinks_the_cube() {
    let h = two_hand_pull(
        Vec3::new(-0.10, 0.0, 0.5),
        Vec3::new(0.02, 0.0, 0.5),
        (Vec3::new(-0.07, 0.0, 0.5), Vec3::new(-0.01, 0.0, 0.5)),
    );
    // 12 cm to 6 cm: half as wide.
    assert_stretched(&h, Vec3::new(0.5, 1.0, 1.0));
}

#[test]
fn letting_go_with_either_hand_keeps_the_stretch_and_the_other_hand_carries_on() {
    let first = Vec3::new(-0.06, 0.0, 0.5);
    let second = Vec3::new(0.02, 0.0, 0.5);
    let apart = (Vec3::new(-0.10, 0.0, 0.5), Vec3::new(0.06, 0.0, 0.5));
    let up = Vec3::new(0.0, 0.05, 0.0);
    for first_lets_go in [true, false] {
        let (open, carry) = if first_lets_go {
            (Pose::open(apart.0), apart.1)
        } else {
            (Pose::open(apart.1), apart.0)
        };
        let mut h = Harness::with_cubes(
            [
                hold(&[Pose::pinched(first)], SETTLE),
                hold(&[Pose::pinched(first), Pose::open(second)], SETTLE),
                hold(&[Pose::pinched(first), Pose::pinched(second)], SETTLE),
                hold(&[Pose::pinched(apart.0), Pose::pinched(apart.1)], SETTLE),
                hold(&[open, Pose::pinched(carry)], SETTLE),
                hold(&[open, Pose::pinched(carry + up)], SETTLE),
            ]
            .concat(),
            &[Vec3::ZERO],
        );
        h.run_all();
        assert_stretched(&h, Vec3::new(2.0, 1.0, 1.0));
        let moved = h.cube().translation;
        assert!(
            moved.y > up.y && moved.x.abs() < 0.01,
            "the hand still pinching should carry the cube up, it's at {moved}"
        );
    }
}

#[test]
fn r_puts_a_stretched_cube_back_to_its_size() {
    let mut h = two_hand_pull(
        Vec3::new(-0.06, 0.0, 0.5),
        Vec3::new(0.02, 0.0, 0.5),
        (Vec3::new(-0.10, 0.0, 0.5), Vec3::new(0.06, 0.0, 0.5)),
    );
    h.press(KeyCode::KeyR);
    assert_stretched(&h, Vec3::ONE);
}

#[test]
fn a_second_stretch_builds_on_the_first() {
    let first = Vec3::new(-0.06, 0.0, 0.5);
    let second = Vec3::new(0.02, 0.0, 0.5);
    let apart = (Vec3::new(-0.10, 0.0, 0.5), Vec3::new(0.06, 0.0, 0.5));
    let mut h = Harness::with_cubes(
        [
            hold(&[Pose::pinched(first)], SETTLE),
            hold(&[Pose::pinched(first), Pose::open(second)], SETTLE),
            hold(&[Pose::pinched(first), Pose::pinched(second)], SETTLE),
            hold(&[Pose::pinched(apart.0), Pose::pinched(apart.1)], SETTLE),
            // Let go with the second hand, bring it back in and pull again.
            hold(&[Pose::pinched(apart.0), Pose::open(apart.1)], SETTLE),
            hold(&[Pose::pinched(apart.0), Pose::open(second)], SETTLE),
            hold(&[Pose::pinched(apart.0), Pose::pinched(second)], SETTLE),
            hold(&[Pose::pinched(apart.0), Pose::pinched(apart.1)], SETTLE),
        ]
        .concat(),
        &[Vec3::ZERO],
    );
    h.run_all();
    // 16 cm to 12 cm and back: 2 × 16/12.
    assert_stretched(&h, Vec3::new(2.67, 1.0, 1.0));
}
