mod support;

use bevy::prelude::*;
use support::*;
use terrain_hands::HandFrame;

/// Where calibration asks for the hand: 50 cm straight in front of the camera.
const CALIBRATION_SPOT: Vec3 = Vec3::new(0.0, 0.0, 0.5);
/// Frames covering calibration's 3 s countdown and 1 s of measuring.
const CALIBRATING: usize = 130;

#[test]
fn after_calibrating_depth_is_true_to_scale_on_a_narrower_camera() {
    let camera = TestCamera {
        hfov_deg: 50.0,
        ..default()
    };
    let toward = CALIBRATION_SPOT - Vec3::new(0.0, 0.0, 0.10);
    let mut h = Harness::new(
        [
            camera.hold(&[Pose::open(CALIBRATION_SPOT)], CALIBRATING),
            camera.hold(&[Pose::pinched(CALIBRATION_SPOT)], SETTLE),
            camera.hold(&[Pose::pinched(toward)], SETTLE),
            camera.hold(&[Pose::open(toward)], SETTLE),
        ]
        .concat(),
    );
    h.press(KeyCode::KeyC).run_all();
    assert_near(h.cube().translation, Vec3::new(0.0, 0.0, -0.15));
}

/// Where the cube ends up after `calibrating` frames and then a 10 cm push
/// toward a 50° camera, with or without pressing `C` first.
fn push_after(calibrating: Vec<HandFrame>, press_c: bool) -> Vec3 {
    let camera = TestCamera {
        hfov_deg: 50.0,
        ..default()
    };
    let toward = CALIBRATION_SPOT - Vec3::new(0.0, 0.0, 0.10);
    let mut h = Harness::new(
        [
            calibrating,
            camera.hold(&[Pose::open(CALIBRATION_SPOT)], SETTLE),
            camera.hold(&[Pose::pinched(CALIBRATION_SPOT)], SETTLE),
            camera.hold(&[Pose::pinched(toward)], SETTLE),
            camera.hold(&[Pose::open(toward)], SETTLE),
        ]
        .concat(),
    );
    if press_c {
        h.press(KeyCode::KeyC);
    }
    h.run_all();
    h.cube().translation
}

#[test]
fn calibrating_with_no_hand_in_view_keeps_the_old_camera() {
    let empty = vec![no_hands(); CALIBRATING];
    assert_near(push_after(empty.clone(), true), push_after(empty, false));
}

#[test]
fn calibrating_with_a_moving_hand_keeps_the_old_camera() {
    let camera = TestCamera {
        hfov_deg: 50.0,
        ..default()
    };
    // Waving 10 cm side to side, a frame each way.
    let waving: Vec<HandFrame> = (0..CALIBRATING)
        .map(|i| {
            let side = if i % 2 == 0 { 0.05 } else { -0.05 };
            camera.frame(&[Pose::open(CALIBRATION_SPOT + Vec3::X * side)])
        })
        .collect();
    assert_near(push_after(waving.clone(), true), push_after(waving, false));
}

#[test]
fn after_calibrating_a_tilted_camera_raising_the_hand_raises_the_cube() {
    let pitch = 20.0_f32;
    let camera = TestCamera {
        pitch_deg: pitch,
        ..default()
    };
    // 50 cm out along the camera's line of sight, which points up and out.
    let (sin, cos) = pitch.to_radians().sin_cos();
    let spot = Vec3::new(0.0, sin, cos) * 0.5;
    let raised = spot + Vec3::new(0.0, 0.10, 0.0);
    let mut h = Harness::new(
        [
            camera.hold(&[Pose::open(spot)], CALIBRATING),
            camera.hold(&[Pose::pinched(spot)], SETTLE),
            camera.hold(&[Pose::pinched(raised)], SETTLE),
            camera.hold(&[Pose::open(raised)], SETTLE),
        ]
        .concat(),
    );
    h.press(KeyCode::KeyC).run_all();
    assert_near(h.cube().translation, Vec3::new(0.0, 0.15, 0.0));
}

#[test]
fn calibrating_uses_the_hand_in_the_middle_when_two_are_in_view() {
    let camera = TestCamera {
        hfov_deg: 50.0,
        ..default()
    };
    // The other hand hangs low and farther off, listed first.
    let resting = Pose::open(Vec3::new(0.15, -0.25, 0.9)).left();
    let toward = CALIBRATION_SPOT - Vec3::new(0.0, 0.0, 0.10);
    let mut h = Harness::new(
        [
            camera.hold(&[resting, Pose::open(CALIBRATION_SPOT)], CALIBRATING),
            camera.hold(&[Pose::pinched(CALIBRATION_SPOT)], SETTLE),
            camera.hold(&[Pose::pinched(toward)], SETTLE),
            camera.hold(&[Pose::open(toward)], SETTLE),
        ]
        .concat(),
    );
    h.press(KeyCode::KeyC).run_all();
    assert_near(h.cube().translation, Vec3::new(0.0, 0.0, -0.15));
}

#[test]
fn calibrating_with_the_hand_far_beyond_50_cm_keeps_the_old_camera() {
    // Standing back at 1.5 m: fitting that as 50 cm would need a ~120° lens.
    let far = vec![frame(&[Pose::open(Vec3::new(0.0, 0.0, 1.5))]); CALIBRATING];
    assert_near(push_after(far.clone(), true), push_after(far, false));
}

#[test]
fn a_successful_calibration_puts_the_cube_back() {
    let camera = TestCamera::default();
    let moved = CALIBRATION_SPOT + Vec3::new(0.10, 0.0, 0.0);
    let mut h = Harness::new(
        [
            camera.hold(&[Pose::pinched(CALIBRATION_SPOT)], SETTLE),
            camera.hold(&[Pose::pinched(moved)], SETTLE),
            camera.hold(&[Pose::open(moved)], SETTLE),
            camera.hold(&[Pose::open(CALIBRATION_SPOT)], CALIBRATING),
        ]
        .concat(),
    );
    h.run(3 * SETTLE);
    assert!(
        h.cube().translation.x > 0.1,
        "the grab should have moved the cube first"
    );
    h.press(KeyCode::KeyC).run_all();
    assert_near(h.cube().translation, Vec3::ZERO);
}

#[test]
fn a_pinch_held_through_calibration_does_not_grab_the_reset_cube() {
    let camera = TestCamera::default();
    let mut h = Harness::new(
        [
            camera.hold(&[Pose::pinched(CALIBRATION_SPOT)], CALIBRATING),
            camera.hold(&[Pose::pinched(CALIBRATION_SPOT + Vec3::X * 0.10)], SETTLE),
        ]
        .concat(),
    );
    h.run(1);
    h.press(KeyCode::KeyC).run_all();
    assert_near(h.cube().translation, Vec3::ZERO);
}
