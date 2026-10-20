//! Adding and removing shapes by key: N spawns, Delete removes, D duplicates.

use crate::support::*;
use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use terrain_app::{Arena, Grabbable, Selection};

const LEFT_CUBE: Vec3 = Vec3::new(-0.25, 0.0, 0.0);
const RIGHT_CUBE: Vec3 = Vec3::new(0.25, 0.0, 0.0);
/// Hands whose cursors sit on the left and right cube.
const ON_LEFT: Vec3 = Vec3::new(-0.12, 0.0, 0.5);
const ON_RIGHT: Vec3 = Vec3::new(0.12, 0.0, 0.5);
/// Below both cubes, on nothing.
const ON_NOTHING: Vec3 = Vec3::new(0.0, -0.2, 0.5);

fn shapes(h: &mut Harness) -> Vec<(Entity, Vec3)> {
    let world = h.app.world_mut();
    world
        .query_filtered::<(Entity, &Transform), With<Grabbable>>()
        .iter(world)
        .map(|(e, t)| (e, t.translation))
        .collect()
}

fn selected(h: &Harness) -> Option<Entity> {
    h.app.world().resource::<Selection>().0
}

fn assert_on_grid(at: Vec3, arena: &Arena) {
    let cells = (at - arena.min) / arena.cell - Vec3::splat(0.5);
    assert!(
        cells.abs_diff_eq(cells.round(), 1e-3),
        "{at} isn't the middle of a grid cell ({cells} cells in)"
    );
}

/// Keeps every pair of shapes at least a cell apart along some axis, so
/// none starts inside another.
fn assert_apart(shapes: &[(Entity, Vec3)], cell: f32) {
    for (i, (_, a)) in shapes.iter().enumerate() {
        for (_, b) in &shapes[i + 1..] {
            assert!(
                (*a - *b).abs().max_element() >= cell - 1e-3,
                "shapes at {a} and {b} overlap"
            );
        }
    }
}

#[test]
fn n_spawns_a_box_in_the_cursors_grid_cell() {
    let mut h = Harness::with_cubes(hold(&[Pose::open(ON_NOTHING)], SETTLE), &[]);
    h.run_all().press(KeyCode::KeyN).run(1);
    let made = shapes(&mut h);
    assert_eq!(made.len(), 1, "N should make one box");
    let at = made[0].1;
    assert_on_grid(at, &Arena::default());
    assert!(
        at.y < -0.05,
        "the cursor is low in the view, the box is at {at}"
    );
}

#[test]
fn spawned_boxes_follow_the_cursor_and_land_where_it_is() {
    let left = Vec3::new(-0.15, 0.0, 0.5);
    let right = Vec3::new(0.15, 0.0, 0.5);
    let mut h = Harness::with_cubes(hold(&[Pose::open(left)], SETTLE), &[]);
    h.run_all().press(KeyCode::KeyN).run(1);
    let first = shapes(&mut h)[0].1;
    let mut h = Harness::with_cubes(hold(&[Pose::open(right)], SETTLE), &[]);
    h.run_all().press(KeyCode::KeyN).run(1);
    let second = shapes(&mut h)[0].1;
    assert!(
        first.x < -0.1 && second.x > 0.1,
        "boxes at {first} and {second}"
    );
}

#[test]
fn spawning_into_an_occupied_cell_or_past_the_cap_is_refused() {
    let mut h = Harness::with_cubes(hold(&[Pose::open(ON_NOTHING)], SETTLE), &[]);
    h.run_all()
        .press(KeyCode::KeyN)
        .run(1)
        .press(KeyCode::KeyN)
        .run(1);
    assert_eq!(
        shapes(&mut h).len(),
        1,
        "the second N lands on the first box"
    );

    let mut h = Harness::with_cubes(
        hold(&[Pose::open(ON_NOTHING)], SETTLE),
        &[LEFT_CUBE, RIGHT_CUBE],
    );
    h.app.world_mut().resource_mut::<Arena>().cap = 2;
    h.run_all().press(KeyCode::KeyN).run(1);
    assert_eq!(shapes(&mut h).len(), 2, "the Arena is full");
}

#[test]
fn selection_is_the_hovered_shape_else_the_last_grabbed() {
    let mut h = Harness::with_cubes(
        [hold(&[Pose::open(ON_NOTHING)], SETTLE)].concat(),
        &[LEFT_CUBE, RIGHT_CUBE],
    );
    h.run_all();
    assert_eq!(selected(&h), None, "nothing hovered or grabbed yet");

    let mut h = Harness::with_cubes(
        [
            hold(&[Pose::open(ON_LEFT)], SETTLE),
            hold(&[Pose::pinched(ON_LEFT)], SETTLE),
            hold(&[Pose::open(ON_LEFT)], SETTLE),
            hold(&[Pose::open(ON_NOTHING)], SETTLE),
        ]
        .concat(),
        &[LEFT_CUBE, RIGHT_CUBE],
    );
    h.run(3 * SETTLE);
    assert_eq!(selected(&h), Some(h.cubes[0]), "hovering the left cube");
    h.run(SETTLE);
    assert_eq!(
        selected(&h),
        Some(h.cubes[0]),
        "grabbed last, hovering nothing"
    );

    let mut h = Harness::with_cubes(
        [
            hold(&[Pose::open(ON_LEFT)], SETTLE),
            hold(&[Pose::pinched(ON_LEFT)], SETTLE),
            hold(&[Pose::open(ON_RIGHT)], SETTLE),
        ]
        .concat(),
        &[LEFT_CUBE, RIGHT_CUBE],
    );
    h.run_all();
    assert_eq!(selected(&h), Some(h.cubes[1]), "hover beats the last grab");
}

#[test]
fn delete_removes_the_selection_and_r_brings_the_scene_back() {
    let mut h = Harness::with_cubes(
        hold(&[Pose::open(ON_LEFT)], SETTLE),
        &[LEFT_CUBE, RIGHT_CUBE],
    );
    h.run_all().press(KeyCode::Delete).run(1);
    let left = shapes(&mut h);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].0, h.cubes[1], "the hovered left cube goes");

    h.press(KeyCode::KeyN).run(1);
    assert_eq!(
        shapes(&mut h).len(),
        2,
        "a box spawned where the left cube was"
    );
    h.press(KeyCode::KeyR).run(1);
    let back = shapes(&mut h);
    assert_eq!(
        back.len(),
        2,
        "R removes the new box and restores the deleted cube"
    );
    assert_near(h.cube_at(0), LEFT_CUBE);
    assert_near(h.cube_at(1), RIGHT_CUBE);
}

#[test]
fn a_deleted_shape_is_no_longer_solid() {
    let mut h = Harness::with_cubes(
        hold(&[Pose::open(ON_LEFT)], 2 * SETTLE),
        &[LEFT_CUBE, RIGHT_CUBE],
    );
    let solid_at = |h: &mut Harness, at: Vec3| {
        h.app
            .world_mut()
            .run_system_once(move |q: SpatialQuery| {
                q.point_intersections(at, &SpatialQueryFilter::default())
                    .len()
            })
            .unwrap()
            > 0
    };
    h.run(SETTLE);
    assert!(solid_at(&mut h, LEFT_CUBE), "the left cube is solid before");
    h.press(KeyCode::Delete).run(2);
    assert!(
        !solid_at(&mut h, LEFT_CUBE),
        "the deleted cube is still solid"
    );
    assert!(
        solid_at(&mut h, RIGHT_CUBE),
        "the right cube should still be solid"
    );
}

#[test]
fn d_duplicates_the_selection_into_a_free_cell_next_to_it() {
    let mut h = Harness::with_cubes(
        hold(&[Pose::open(ON_LEFT)], SETTLE),
        &[LEFT_CUBE, RIGHT_CUBE],
    );
    h.run_all()
        .press(KeyCode::KeyD)
        .run(1)
        .press(KeyCode::KeyD)
        .run(1);
    let all = shapes(&mut h);
    assert_eq!(all.len(), 4, "two copies");
    let arena = Arena::default();
    for (_, at) in all.iter().filter(|(e, _)| !h.cubes.contains(e)) {
        assert_on_grid(*at, &arena);
        assert!(
            at.distance(LEFT_CUBE) < 2.0 * arena.cell,
            "copy at {at} is far from the left cube"
        );
    }
    assert_apart(&all, arena.cell);
}

#[test]
fn the_selection_stays_on_the_last_hovered_shape_when_the_cursor_leaves() {
    let mut h = Harness::with_cubes(
        [
            hold(&[Pose::open(ON_RIGHT)], SETTLE),
            hold(&[Pose::open(ON_NOTHING)], SETTLE),
        ]
        .concat(),
        &[LEFT_CUBE, RIGHT_CUBE],
    );
    h.run_all();
    assert_eq!(
        selected(&h),
        Some(h.cubes[1]),
        "still the right cube, pointed at last"
    );
}

#[test]
fn a_mouse_over_the_overlay_points_at_nothing_behind_it() {
    let mut h = Harness::with_cubes(Vec::new(), &[LEFT_CUBE, RIGHT_CUBE]);
    // The overlay covers the middle of the window (fractions), where the mouse is.
    h.app
        .world_mut()
        .resource_mut::<terrain_app::OverlayArea>()
        .0 = Some(Rect::new(0.375, 0.33, 0.625, 0.67));
    // A window with the mouse right on the left cube.
    let mut window = Window::default();
    window.resolution.set(1600.0, 900.0);
    window.set_cursor_position(Some(Vec2::new(0.5, 0.5) * Vec2::new(1600.0, 900.0)));
    h.app
        .world_mut()
        .spawn((window, bevy::window::PrimaryWindow));
    let world = h.app.world_mut();
    world
        .entity_mut(h.cubes[0])
        .insert(Transform::from_translation(Vec3::ZERO));
    h.run(2);
    assert_eq!(
        selected(&h),
        None,
        "the mouse is on the overlay, not the cube"
    );
    h.app
        .world_mut()
        .resource_mut::<terrain_app::OverlayArea>()
        .0 = None;
    h.run(2);
    assert_eq!(
        selected(&h),
        Some(h.cubes[0]),
        "off the overlay, it points at the cube"
    );
}
