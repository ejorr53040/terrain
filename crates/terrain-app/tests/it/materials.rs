//! Materials: presets that set how shapes bounce, slide and weigh.

use avian3d::prelude::*;
use bevy::prelude::*;
use crate::support::*;
use terrain_app::{Material, Materials};

/// One cube at `at` made of `material`, in a world with gravity.
fn falling(material: &str, at: Vec3) -> Harness {
    let mut h = Harness::with_cubes(Vec::new(), &[at]);
    let world = h.app.world_mut();
    world.insert_resource(Gravity(Vec3::NEG_Y * 9.81));
    let index = world.resource::<Materials>().index(material).unwrap();
    world.entity_mut(h.cube).insert(Material(index));
    h
}

/// How high the cube gets back up after it first hits the floor.
fn rebound(h: &mut Harness) -> f32 {
    let mut heights = Vec::new();
    for _ in 0..90 {
        h.run(1);
        heights.push(h.cube_at(0).y);
    }
    // The first time it stops going down and comes back up; 0 if it never does.
    let Some(lowest) = (0..heights.len() - 1).find(|&i| heights[i + 1] > heights[i] + 1e-3) else {
        return 0.0;
    };
    heights[lowest..].iter().cloned().fold(f32::MIN, f32::max) - heights[lowest]
}

#[test]
fn rubber_bounces_higher_than_wood() {
    let rubber = rebound(&mut falling("rubber", Vec3::new(0.0, 0.3, 0.0)));
    let wood = rebound(&mut falling("wood", Vec3::new(0.0, 0.3, 0.0)));
    assert!(rubber > 0.05, "rubber barely bounced: {rubber}");
    assert!(rubber > 2.0 * wood, "rubber {rubber}, wood {wood}");
}

/// How far a cube resting on the floor slides from a 1 m/s shove.
fn slide(material: &str) -> f32 {
    let mut h = falling(material, Vec3::new(-0.3, -0.2, 0.0));
    h.run(SETTLE);
    let from = h.cube_at(0).x;
    h.app.world_mut().entity_mut(h.cube).insert(LinearVelocity(Vec3::X * 1.0));
    h.run(2 * SETTLE);
    h.cube_at(0).x - from
}

#[test]
fn glass_slides_farther_than_rubber() {
    let glass = slide("glass");
    let rubber = slide("rubber");
    assert!(glass > 1.5 * rubber, "glass {glass}, rubber {rubber}");
}

#[test]
fn editing_a_preset_changes_every_shape_made_of_it() {
    let mut h = falling("wood", Vec3::new(0.0, 0.3, 0.0));
    h.run(1);
    let mut materials = h.app.world_mut().resource_mut::<Materials>();
    let wood = materials.index("wood").unwrap();
    materials.presets[wood].restitution = 0.9;
    assert!(rebound(&mut h) > 0.05, "wood made bouncy should bounce");
}

#[test]
fn m_cycles_the_selected_shapes_material() {
    let mut h = Harness::with_cubes(hold(&[Pose::open(Vec3::new(0.0, 0.0, 0.5))], 3 * SETTLE), &[Vec3::ZERO]);
    h.run(SETTLE);
    let before = h.app.world().get::<Material>(h.cube).unwrap().0;
    h.press(KeyCode::KeyM);
    let count = h.app.world().resource::<Materials>().presets.len();
    let after = h.app.world().get::<Material>(h.cube).unwrap().0;
    assert_eq!(after, (before + 1) % count);
}

#[test]
fn without_gravity_a_shape_let_go_still_stays_put() {
    // The floating world as it was: a shove dies at once.
    let mut h = Harness::with_cubes(Vec::new(), &[Vec3::ZERO]);
    h.run(2);
    h.app.world_mut().entity_mut(h.cube).insert(LinearVelocity(Vec3::X));
    h.run(SETTLE);
    assert!(h.cube_at(0).x < 0.05, "drifted to {}", h.cube_at(0));
}
