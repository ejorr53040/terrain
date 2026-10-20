//! The overlay's Scene buttons ask for what R and C do, through `Requests`.

use crate::support::*;
use bevy::prelude::*;
use terrain_app::Requests;

/// Where calibration asks for the hand: 50 cm straight in front of the camera.
const CALIBRATION_SPOT: Vec3 = Vec3::new(0.0, 0.0, 0.5);
/// Frames covering calibration's 3 s countdown and 1 s of measuring.
const CALIBRATING: usize = 130;

/// The cube grabbed and moved 10 cm right, then let go; then `rest` frames.
fn moved_cube(rest: usize) -> Harness {
    let camera = TestCamera::default();
    let moved = CALIBRATION_SPOT + Vec3::new(0.10, 0.0, 0.0);
    let mut h = Harness::new(
        [
            camera.hold(&[Pose::pinched(CALIBRATION_SPOT)], SETTLE),
            camera.hold(&[Pose::pinched(moved)], SETTLE),
            camera.hold(&[Pose::open(moved)], SETTLE),
            camera.hold(&[Pose::open(CALIBRATION_SPOT)], rest),
        ]
        .concat(),
    );
    h.run(3 * SETTLE);
    assert!(
        h.cube().translation.x > 0.1,
        "the grab should have moved the cube first"
    );
    h
}

#[test]
fn the_reset_button_puts_the_cube_back() {
    let mut h = moved_cube(SETTLE);
    h.app.world_mut().resource_mut::<Requests>().reset = true;
    h.run(2);
    assert_near(h.cube().translation, Vec3::ZERO);
}

#[test]
fn the_calibrate_button_calibrates() {
    let mut h = moved_cube(CALIBRATING);
    h.app.world_mut().resource_mut::<Requests>().calibrate = true;
    h.run_all();
    // A successful calibration puts the cube back, as C does.
    assert_near(h.cube().translation, Vec3::ZERO);
}
