//! The active tool (keys 1–4) and what `[` / `]` do with it to the selection.

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use crate::support::*;
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
            q.point_intersections(at, &SpatialQueryFilter::default()).len()
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
    assert!((h.shape().roundness - 0.3).abs() < 1e-4, "{}", h.shape().roundness);
    h.press(KeyCode::BracketLeft);
    assert!((h.shape().roundness - 0.2).abs() < 1e-4, "{}", h.shape().roundness);
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
    assert!(solid_at(&mut h, near_corner), "a box is solid into its corners");
    h.press(KeyCode::Digit2);
    for _ in 0..10 {
        h.press(KeyCode::BracketRight);
    }
    h.run(2);
    assert_eq!(h.shape().size, Vec3::splat(CUBE_SIZE), "rounding keeps the size");
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
