//! The active tool (keys 1–4) and what `[` / `]` do with it to the selection.

use crate::support::*;
use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use terrain_app::{CUBE_SIZE, Tool};

/// A hand whose cursor sits on a cube in the middle of the scene.
const ON_MIDDLE: Vec3 = Vec3::new(0.0, 0.0, 0.5);

/// One cube in the middle, selected by a hand hovering it.
fn hovered_cube() -> Harness {
    let mut h = Harness::with_cubes(hold(&[Pose::open(ON_MIDDLE)], 4 * SETTLE), &[Vec3::ZERO]);
    h.run(SETTLE);
    h
}

fn tool(h: &Harness) -> Tool {
    *h.app.world().resource::<Tool>()
}

fn solid_at(h: &mut Harness, at: Vec3) -> bool {
    h.app
        .world_mut()
        .run_system_once(move |q: SpatialQuery| {
            q.point_intersections(at, &SpatialQueryFilter::default())
                .len()
        })
        .unwrap()
        > 0
}

#[test]
fn keys_1_to_4_pick_the_tool() {
    let mut h = hovered_cube();
    assert_eq!(tool(&h), Tool::Move, "Move to begin with");
    for (key, expected) in [
        (KeyCode::Digit2, Tool::Round),
        (KeyCode::Digit3, Tool::Taper),
        (KeyCode::Digit4, Tool::Copy),
        (KeyCode::Digit1, Tool::Move),
    ] {
        h.press(key);
        assert_eq!(tool(&h), expected);
    }
}

#[test]
fn brackets_round_the_selection_with_the_round_tool_only() {
    let mut h = hovered_cube();
    h.press(KeyCode::BracketRight);
    assert_eq!(h.shape().roundness, 0.0, "Move doesn't round");
    h.press(KeyCode::Digit2);
    for _ in 0..3 {
        h.press(KeyCode::BracketRight);
    }
    assert!(
        (h.shape().roundness - 0.3).abs() < 1e-4,
        "{}",
        h.shape().roundness
    );
    h.press(KeyCode::BracketLeft);
    assert!(
        (h.shape().roundness - 0.2).abs() < 1e-4,
        "{}",
        h.shape().roundness
    );
    for _ in 0..20 {
        h.press(KeyCode::BracketRight);
    }
    assert_eq!(h.shape().roundness, 1.0, "roundness tops out at 1");
    for _ in 0..20 {
        h.press(KeyCode::BracketLeft);
    }
    assert_eq!(h.shape().roundness, 0.0, "and bottoms out at 0");
}

#[test]
fn a_fully_round_shape_is_a_cylinder_standing_up() {
    let mut h = hovered_cube();
    let near_corner = Vec3::new(0.065, 0.0, 0.065);
    let side = Vec3::new(0.07, 0.0, 0.0);
    let top_edge = Vec3::new(0.0, 0.07, 0.07);
    assert!(
        solid_at(&mut h, near_corner),
        "a box is solid into its corners"
    );
    h.press(KeyCode::Digit2);
    for _ in 0..10 {
        h.press(KeyCode::BracketRight);
    }
    h.run(2);
    assert_eq!(
        h.shape().size,
        Vec3::splat(CUBE_SIZE),
        "rounding keeps the size"
    );
    assert!(!solid_at(&mut h, near_corner), "a cylinder has no corners");
    assert!(solid_at(&mut h, side), "but is solid out to its sides");
    assert!(solid_at(&mut h, top_edge), "and has flat ends");
}

#[test]
fn half_round_is_in_between() {
    let mut h = hovered_cube();
    h.press(KeyCode::Digit2);
    for _ in 0..5 {
        h.press(KeyCode::BracketRight);
    }
    h.run(2);
    // Corner arcs of radius 3.75 cm: the very corner is gone, the flat
    // part of each side isn't.
    assert!(!solid_at(&mut h, Vec3::new(0.07, 0.0, 0.07)));
    assert!(solid_at(&mut h, Vec3::new(0.06, 0.0, 0.03)));
}

#[test]
fn brackets_taper_the_selection_with_the_taper_tool() {
    let mut h = hovered_cube();
    h.press(KeyCode::Digit3);
    for _ in 0..4 {
        h.press(KeyCode::BracketRight);
    }
    assert!((h.shape().taper - 0.4).abs() < 1e-4, "{}", h.shape().taper);
    assert_eq!(h.shape().roundness, 0.0, "Taper leaves roundness alone");
    for _ in 0..20 {
        h.press(KeyCode::BracketRight);
    }
    assert_eq!(h.shape().taper, 1.0, "taper tops out at 1");
}

#[test]
fn a_fully_tapered_box_is_a_pyramid() {
    let mut h = hovered_cube();
    h.press(KeyCode::Digit3);
    for _ in 0..10 {
        h.press(KeyCode::BracketRight);
    }
    h.run(2);
    assert!(
        solid_at(&mut h, Vec3::new(0.07, -0.07, 0.07)),
        "the base keeps its corners"
    );
    assert!(
        !solid_at(&mut h, Vec3::new(0.06, 0.06, 0.06)),
        "the top is a point"
    );
    assert!(
        solid_at(&mut h, Vec3::new(0.0, 0.07, 0.0)),
        "the point is in the middle"
    );
}

#[test]
fn a_round_shape_tapers_into_a_cone() {
    let mut h = hovered_cube();
    h.press(KeyCode::Digit2);
    for _ in 0..10 {
        h.press(KeyCode::BracketRight);
    }
    h.press(KeyCode::Digit3);
    for _ in 0..10 {
        h.press(KeyCode::BracketRight);
    }
    h.run(2);
    assert!(
        !solid_at(&mut h, Vec3::new(0.065, -0.07, 0.065)),
        "the base is round"
    );
    assert!(
        solid_at(&mut h, Vec3::new(0.07, -0.07, 0.0)),
        "out to its rim"
    );
    assert!(
        !solid_at(&mut h, Vec3::new(0.04, 0.05, 0.0)),
        "and narrows to a point"
    );
}

/// A fresh scene that will pinch the middle cube and carry it right.
fn copy_drag(h: &mut Harness) {
    let right = Vec3::new(0.1, 0.0, 0.0);
    let frames = [
        // Long enough to press the keys before the pinch.
        hold(&[Pose::open(ON_MIDDLE)], 2 * SETTLE),
        hold(&[Pose::pinched(ON_MIDDLE)], SETTLE),
        hold(&[Pose::pinched(ON_MIDDLE + right)], SETTLE),
        hold(&[Pose::open(ON_MIDDLE + right)], SETTLE),
    ]
    .concat();
    let mut fresh = Harness::with_cubes(frames, &[Vec3::ZERO]);
    std::mem::swap(h, &mut fresh);
}

fn all_shapes(h: &mut Harness) -> Vec<(Vec3, terrain_app::Shape)> {
    let world = h.app.world_mut();
    world
        .query_filtered::<(&Transform, &terrain_app::Shape), With<terrain_app::Grabbable>>()
        .iter(world)
        .map(|(t, s)| (t.translation, *s))
        .collect()
}

#[test]
fn a_pinch_with_the_copy_tool_carries_off_a_copy_and_leaves_the_original() {
    let mut h = hovered_cube();
    copy_drag(&mut h);
    h.run(SETTLE);
    // Round the original first, so the copy has something to copy.
    h.press(KeyCode::Digit2)
        .press(KeyCode::BracketRight)
        .press(KeyCode::Digit4);
    h.run_all();
    h.run(SETTLE);
    let shapes = all_shapes(&mut h);
    assert_eq!(shapes.len(), 2, "one copy");
    assert_near(h.cube_at(0), Vec3::ZERO);
    let (copy_at, copy) = shapes
        .into_iter()
        .find(|(at, _)| at.x > 0.1)
        .expect("a copy carried right");
    assert_eq!(copy, h.shape(), "the copy has the original's parameters");
    assert!(
        copy_at.distance(Vec3::ZERO) >= 0.149,
        "the copy was let go overlapping, at {copy_at}"
    );
}

#[test]
fn copy_is_refused_when_the_arena_is_full() {
    let mut h = hovered_cube();
    copy_drag(&mut h);
    h.app.world_mut().resource_mut::<terrain_app::Arena>().cap = 1;
    h.run(SETTLE).press(KeyCode::Digit4);
    h.run_all();
    assert_eq!(all_shapes(&mut h).len(), 1, "no copy past the cap");
    assert_near(h.cube_at(0), Vec3::ZERO);
}
