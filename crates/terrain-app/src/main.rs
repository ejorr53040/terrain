//! terrain: a cube you move with your hands.
//!
//! Until live tracking lands, hands come from a replay fixture:
//! `terrain-app [--replay <fixture.json>]` (defaults to the bundled demo, looped).

use bevy::prelude::*;
use terrain_app::{GrabPlugin, Grabbable};
use terrain_hands::ReplaySource;

const DEMO_FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/demo-drag.json");

fn main() -> AppExit {
    let args: Vec<String> = std::env::args().collect();
    let fixture = match args.iter().position(|a| a == "--replay") {
        Some(i) => args
            .get(i + 1)
            .expect("--replay needs a fixture path")
            .clone(),
        None => DEMO_FIXTURE.to_string(),
    };
    let source = ReplaySource::from_json_file(&fixture)
        .unwrap_or_else(|e| panic!("can't load replay fixture {fixture}: {e}"))
        .looping();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "terrain".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(GrabPlugin::new(source))
        .add_systems(Startup, spawn_scene)
        .run()
}

fn spawn_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.25, 1.2).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            ..default()
        },
        Transform::from_xyz(1.0, 2.0, 1.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::from_length(0.15))),
        MeshMaterial3d(materials.add(Color::srgb(0.35, 0.65, 0.95))),
        Transform::default(),
        Grabbable,
    ));
}
