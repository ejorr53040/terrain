mod support;

use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use support::*;

const START: Vec3 = Vec3::new(0.0, 0.0, 0.5);

#[test]
fn moving_the_hand_toward_the_camera_pushes_the_cube_into_the_screen() {
    let toward = START - Vec3::new(0.0, 0.0, 0.10);
    let mut h = Harness::new(vec![
        frame(&[Pose::pinched(START)]),
        frame(&[Pose::pinched(toward)]),
        frame(&[Pose::open(toward)]),
    ]);
    h.run(3);
    assert_near(h.cube().translation, Vec3::new(0.0, 0.0, -0.15));
}

#[test]
fn sideways_moves_are_true_to_scale_near_and_far() {
    for depth in [0.35, 0.8] {
        let at = Vec3::new(0.0, 0.0, depth);
        let moved = at + Vec3::new(0.10, 0.05, 0.0);
        let mut h = Harness::new(vec![
            frame(&[Pose::pinched(at)]),
            frame(&[Pose::pinched(moved)]),
            frame(&[Pose::open(moved)]),
        ]);
        h.run(3);
        assert_near(h.cube().translation, Vec3::new(0.15, 0.075, 0.0));
    }
}

#[test]
fn turning_the_hand_turns_the_cube_and_leaves_it_in_place() {
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        let turn = Quat::from_axis_angle(axis, FRAC_PI_2);
        let mut h = Harness::new(vec![
            frame(&[Pose::pinched(START)]),
            frame(&[Pose::pinched(START).rotated(turn)]),
        ]);
        h.run(2);
        assert_turned(h.cube().rotation, turn);
        assert_near(h.cube().translation, Vec3::ZERO);
    }
}

#[test]
fn turns_accumulate_in_the_scene_frame_across_regrabs() {
    let yaw = Quat::from_rotation_y(FRAC_PI_2);
    let pitch = Quat::from_rotation_x(FRAC_PI_2);
    let tilted = Quat::from_rotation_z(0.8);
    let mut h = Harness::new(vec![
        frame(&[Pose::pinched(START)]),
        frame(&[Pose::pinched(START).rotated(yaw)]),
        frame(&[Pose::open(START).rotated(yaw)]),
        // Re-grab with the hand already tilted; only the turn after that counts.
        frame(&[Pose::open(START).rotated(tilted)]),
        frame(&[Pose::pinched(START).rotated(tilted)]),
        frame(&[Pose::pinched(START).rotated(pitch * tilted)]),
    ]);
    h.run(6);
    assert_turned(h.cube().rotation, pitch * yaw);
}
