//! terrain: a cube you move with your hands.
//!
//! Hands come from the webcam (`--camera <device>`, default `/dev/video0`),
//! or from a recorded fixture with `--replay <fixture.json>` (looped; the
//! webcam then only feeds the preview).

use bevy::prelude::*;
use terrain_app::{DEFAULT_CAMERA, GrabPlugin, Grabbable, PreviewPlugin, start_tracking};
use terrain_hands::{HandSource, ReplaySource};

fn main() -> AppExit {
    let args: Vec<String> = std::env::args().collect();
    let option = |name: &str| {
        args.iter().position(|a| a == name).map(|i| {
            args.get(i + 1)
                .unwrap_or_else(|| panic!("{name} needs a value"))
                .clone()
        })
    };
    let device = option("--camera").unwrap_or_else(|| DEFAULT_CAMERA.to_string());
    let tracking = start_tracking(&device);
    let source: Box<dyn HandSource> = match (option("--replay"), &tracking) {
        (Some(fixture), _) => Box::new(
            ReplaySource::from_json_file(&fixture)
                .unwrap_or_else(|e| panic!("can't load replay fixture {fixture}: {e}"))
                .looping(),
        ),
        (None, Ok(tracker)) => Box::new(tracker.hand_source()),
        // No webcam: the overlay says why, and no hands ever arrive.
        (None, Err(_)) => Box::new(ReplaySource::new(Vec::new())),
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "terrain".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(GrabPlugin::new(source))
        .add_plugins(PreviewPlugin::new(tracking))
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
