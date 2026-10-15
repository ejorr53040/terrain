//! The physics settings the overlay edits: gravity, pause and step, time
//! scale and damping.

use bevy::prelude::*;
use crate::support::*;
use terrain_app::PhysicsSettings;

/// One cube up high; `settings` changed before anything runs.
fn cube_with(settings: impl FnOnce(&mut PhysicsSettings)) -> Harness {
    let mut h = Harness::with_cubes(Vec::new(), &[Vec3::new(0.0, 0.3, 0.0)]);
    settings(&mut h.app.world_mut().resource_mut::<PhysicsSettings>());
    h
}

/// How far the cube drops in `updates`.
fn drop(h: &mut Harness, updates: usize) -> f32 {
    let from = h.cube_at(0).y;
    h.run(updates);
    from - h.cube_at(0).y
}

#[test]
fn gravity_is_off_by_default_and_on_makes_shapes_fall() {
    assert!(drop(&mut cube_with(|_| {}), 10) < 1e-3, "floating by default");
    assert!(drop(&mut cube_with(|s| s.gravity_on = true), 10) > 0.1, "falls with gravity");
}

#[test]
fn stronger_gravity_falls_faster() {
    let earth = drop(&mut cube_with(|s| s.gravity_on = true), 6);
    let strong = drop(&mut cube_with(|s| {
        s.gravity_on = true;
        s.gravity = 20.0;
    }), 6);
    assert!(strong > 1.5 * earth, "20 m/s² dropped {strong}, 9.81 dropped {earth}");
}

#[test]
fn pause_freezes_and_step_moves_on_one_frame() {
    let mut h = cube_with(|s| {
        s.gravity_on = true;
        s.paused = true;
    });
    assert!(drop(&mut h, 10) < 1e-4, "paused, nothing moves");
    h.app.world_mut().resource_mut::<PhysicsSettings>().step = true;
    let stepped = drop(&mut h, 1);
    assert!(stepped > 0.0, "a step moves things on");
    // The step's last tick reaches the Transform an update later.
    h.run(1);
    assert!(drop(&mut h, 10) < 1e-4, "and then it's paused again");
}

#[test]
fn half_time_scale_falls_less() {
    let full = drop(&mut cube_with(|s| s.gravity_on = true), 8);
    let half = drop(&mut cube_with(|s| {
        s.gravity_on = true;
        s.time_scale = 0.5;
    }), 8);
    assert!(half < 0.6 * full, "half speed dropped {half}, full {full}");
}

#[test]
fn damping_slows_a_fall() {
    let free = drop(&mut cube_with(|s| s.gravity_on = true), 10);
    let damped = drop(&mut cube_with(|s| {
        s.gravity_on = true;
        s.linear_damping = 5.0;
    }), 10);
    assert!(damped < 0.7 * free, "damped dropped {damped}, free {free}");
}
