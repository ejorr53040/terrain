use crate::support::*;
use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use terrain_app::{Arena, CUBE_SIZE};

/// Pinches at the middle of the view and drags right by 30 cm, `times` over.
fn drag_right(times: usize) -> Vec<HandFrame> {
    let start = Vec3::new(0.0, 0.0, 1.0);
    let one = [
        hold(&[Pose::open(start)], SETTLE),
        hold(&[Pose::pinched(start)], SETTLE),
        hold(&[Pose::pinched(start + Vec3::new(0.3, 0.0, 0.0))], SETTLE),
    ]
    .concat();
    (0..times).flat_map(|_| one.clone()).collect()
}

use terrain_hands::HandFrame;

#[test]
fn shrinking_the_arena_moves_its_walls_in() {
    let mut h = Harness::with_cubes(drag_right(4), &[Vec3::ZERO]);
    // The walls are up from the default Arena first, then it shrinks.
    h.run(1);
    let wall = 0.3;
    h.app.world_mut().resource_mut::<Arena>().max.x = wall;
    h.run_all().run(SETTLE);
    let x = h.cube_at(0).x;
    assert!(
        (x - (wall - CUBE_SIZE / 2.0)).abs() < 0.02,
        "cube at x {x}, should rest at the new wall, {wall}"
    );
}

#[test]
fn the_default_arena_is_the_old_box_with_15_cm_cells_and_room_for_64_shapes() {
    let arena = Arena::default();
    assert_eq!(arena.min, Vec3::new(-0.65, -0.3, -0.4));
    assert_eq!(arena.max, Vec3::new(0.65, 0.4, 0.2));
    assert_eq!(arena.cell, 0.15);
    assert_eq!(arena.cap, 64);
}

#[test]
fn a_shallow_arena_stops_a_stretch_short_of_its_depth() {
    // Hands 8 cm apart along z (toward the camera), then 40 cm: five times
    // as deep, more than an Arena cut to 0.4 m deep holds.
    let first = Vec3::new(-0.02, 0.0, 0.42);
    let second = Vec3::new(0.0, 0.0, 0.50);
    let mut h = Harness::with_cubes(
        [
            hold(&[Pose::pinched(first)], SETTLE),
            hold(&[Pose::pinched(first), Pose::open(second)], SETTLE),
            hold(&[Pose::pinched(first), Pose::pinched(second)], SETTLE),
            hold(
                &[
                    Pose::pinched(first - Vec3::new(0.0, 0.0, 0.08)),
                    Pose::pinched(second + Vec3::new(0.0, 0.0, 0.24)),
                ],
                SETTLE,
            ),
        ]
        .concat(),
        &[Vec3::ZERO],
    );
    h.run(1);
    h.app.world_mut().resource_mut::<Arena>().min.z = -0.2;
    h.run_all();
    let depth = h.shape().size.z;
    let arena = h.app.world().resource::<Arena>();
    assert!(
        depth < arena.max.z - arena.min.z,
        "stretched {depth} m deep, the Arena is only {} m",
        arena.max.z - arena.min.z
    );
    assert!(depth > 0.3, "stretch stopped early, at {depth} m");
}

#[test]
fn a_stretched_shape_is_solid_all_along_its_new_length() {
    let first = Vec3::new(-0.06, 0.0, 0.5);
    let second = Vec3::new(0.02, 0.0, 0.5);
    let mut h = Harness::with_cubes(
        [
            hold(&[Pose::pinched(first)], SETTLE),
            hold(&[Pose::pinched(first), Pose::open(second)], SETTLE),
            hold(&[Pose::pinched(first), Pose::pinched(second)], SETTLE),
            hold(
                &[
                    Pose::pinched(first - Vec3::X * 0.04),
                    Pose::pinched(second + Vec3::X * 0.04),
                ],
                SETTLE,
            ),
        ]
        .concat(),
        &[Vec3::ZERO],
    );
    h.run_all();
    // Twice as wide: 30 cm, so solid 12 cm out from the middle.
    let solid = h
        .app
        .world_mut()
        .run_system_once(|q: SpatialQuery| {
            q.point_intersections(Vec3::new(0.12, 0.0, 0.0), &SpatialQueryFilter::default())
                .len()
        })
        .unwrap();
    assert!(
        solid > 0,
        "the stretched end isn't solid; size {}",
        h.shape().size
    );
}
