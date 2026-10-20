use crate::support::*;
use bevy::prelude::*;

/// Two cubes side by side, 5 cm apart (each is 15 cm across).
const LEFT_CUBE: Vec3 = Vec3::new(-0.2, 0.0, 0.0);
const RIGHT_CUBE: Vec3 = Vec3::new(0.0, 0.0, 0.0);
/// A hand that lines up, on screen, with the left cube.
const AT_LEFT_CUBE: Vec3 = Vec3::new(-0.07, 0.0, 0.5);
const RIGHT: Vec3 = Vec3::new(0.10, 0.0, 0.0);

#[test]
fn a_held_cube_shoves_a_free_cube_out_of_its_way() {
    // Pinch the left cube and carry it 15 cm right, into the right cube's place.
    let mut h = Harness::with_cubes(
        [
            hold(&[Pose::open(AT_LEFT_CUBE)], SETTLE),
            hold(&[Pose::pinched(AT_LEFT_CUBE)], SETTLE),
            hold(&[Pose::pinched(AT_LEFT_CUBE + RIGHT)], SETTLE),
        ]
        .concat(),
        &[LEFT_CUBE, RIGHT_CUBE],
    );
    h.run_all();
    let (held, shoved) = (h.cube_at(0), h.cube_at(1));
    assert!(
        held.x > -0.07,
        "held cube should reach near its target x -0.05, is at {held}"
    );
    assert!(
        shoved.x - held.x > 0.145,
        "cubes overlap: held at {held}, other at {shoved}"
    );
}

#[test]
fn a_cube_let_go_of_mid_move_stays_where_it_was_let_go() {
    // Sweep the cube right, opening the hand partway without stopping.
    let sweep =
        |i: usize, pose: fn(Vec3) -> Pose| frame(&[pose(AT_LEFT_CUBE + 0.02 * i as f32 * Vec3::X)]);
    let mut h = Harness::with_cubes(
        [
            hold(&[Pose::open(AT_LEFT_CUBE)], SETTLE),
            hold(&[Pose::pinched(AT_LEFT_CUBE)], SETTLE),
            (0..6).map(|i| sweep(i, Pose::pinched)).collect(),
            // A pinch outlasts one open frame; three let go for sure.
            (6..9).map(|i| sweep(i, Pose::open)).collect(),
        ]
        .concat(),
        &[LEFT_CUBE],
    );
    h.run_all();
    let let_go_at = h.cube_at(0);
    h.run(SETTLE);
    assert!(
        let_go_at.distance(h.cube_at(0)) < 1e-4,
        "cube drifted from {let_go_at} to {}",
        h.cube_at(0)
    );
}

#[test]
fn a_shoved_cube_stops_when_the_shove_does() {
    let mut h = Harness::with_cubes(
        [
            hold(&[Pose::open(AT_LEFT_CUBE)], SETTLE),
            hold(&[Pose::pinched(AT_LEFT_CUBE)], SETTLE),
            hold(&[Pose::pinched(AT_LEFT_CUBE + RIGHT)], SETTLE),
            hold(&[Pose::open(AT_LEFT_CUBE + RIGHT)], 1),
        ]
        .concat(),
        &[LEFT_CUBE, RIGHT_CUBE],
    );
    h.run_all();
    let shoved_to = h.cube_at(1);
    h.run(SETTLE);
    assert!(
        shoved_to.distance(h.cube_at(1)) < 1e-4,
        "shoved cube drifted from {shoved_to} to {}",
        h.cube_at(1)
    );
}
