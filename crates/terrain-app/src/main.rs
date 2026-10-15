//! terrain: a row of cubes you move with your hands, one in each.
//!
//! Hands come from the webcam (`--camera <device>`, default `/dev/video0`),
//! or from a recorded fixture with `--replay <fixture.json>` (looped; the
//! webcam then only feeds the preview).
//!
//! `C` calibrates the camera; the result is kept in
//! `~/.config/terrain/calibration.json` and loaded at start. `N` adds a box
//! at the cursor, `Delete` removes the outlined shape, `D` copies it and `R`
//! puts the scene back.

use std::path::PathBuf;

use bevy::prelude::*;
use terrain_app::{
    Arena, CUBE_SIZE, CUBE_STARTS, CursorPlugin, DEFAULT_CAMERA, GrabPlugin, Grabbable,
    PreviewPlugin, SCENE_CAMERA_AT, Selection, load_camera, start_tracking,
};
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
    let replay = option("--replay");
    let source: Box<dyn HandSource> = match (replay.clone(), &tracking) {
        (Some(fixture), _) => Box::new(
            ReplaySource::from_json_file(&fixture)
                .unwrap_or_else(|e| panic!("can't load replay fixture {fixture}: {e}"))
                .looping(),
        ),
        (None, Ok(tracker)) => Box::new(tracker.hand_source()),
        // No webcam: the overlay says why, and no hands ever arrive.
        (None, Err(_)) => Box::new(ReplaySource::new(Vec::new())),
    };

    let mut grab = GrabPlugin::new(source);
    // A replay was recorded through its own camera: the saved calibration
    // doesn't apply to it, and calibrating on it mustn't overwrite that.
    if replay.is_none()
        && let Some(path) = calibration_file()
    {
        if let Some(camera) = load_camera(&path) {
            grab = grab.with_camera(camera);
        }
        grab = grab.saving_calibration_to(path);
    }

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "terrain".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(grab)
        .add_plugins((CursorPlugin, PreviewPlugin::new(tracking)))
        .add_systems(Startup, spawn_scene)
        .add_systems(Update, (draw_box, dress_new_shapes, outline_selection))
        .run()
}

/// The Arena's walls, as an outline.
fn draw_box(mut gizmos: Gizmos, arena: Res<Arena>) {
    gizmos.cube(
        Transform::from_translation((arena.min + arena.max) / 2.0).with_scale(arena.max - arena.min),
        Color::srgba(0.7, 0.85, 1.0, 0.6),
    );
}

/// Colours for shapes, the starting row's first, then new boxes' in turn.
const COLORS: [Color; 5] = [
    Color::srgb(0.35, 0.65, 0.95),
    Color::srgb(0.95, 0.6, 0.3),
    Color::srgb(0.45, 0.85, 0.5),
    Color::srgb(0.75, 0.5, 0.95),
    Color::srgb(0.95, 0.85, 0.35),
];

/// Gives each new box (from `N`) a cube to draw and the next colour.
fn dress_new_shapes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    new: Query<Entity, (Added<Grabbable>, Without<Mesh3d>)>,
    mut made: Local<usize>,
) {
    for entity in &new {
        commands.entity(entity).insert((
            Mesh3d(meshes.add(Cuboid::from_length(CUBE_SIZE))),
            MeshMaterial3d(materials.add(COLORS[*made % COLORS.len()])),
        ));
        *made += 1;
    }
}

/// A white box just outside the selected shape.
fn outline_selection(
    mut gizmos: Gizmos,
    selection: Res<Selection>,
    shapes: Query<&Transform, With<Grabbable>>,
) {
    if let Some(t) = selection.0.and_then(|e| shapes.get(e).ok()) {
        gizmos.cube(
            t.with_scale(t.scale * CUBE_SIZE * 1.12),
            Color::WHITE,
        );
    }
}

fn spawn_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(SCENE_CAMERA_AT).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            ..default()
        },
        Transform::from_xyz(1.0, 2.0, 1.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let cube = meshes.add(Cuboid::from_length(0.15));
    // A row of cubes, each its own colour.
    for (at, color) in CUBE_STARTS.into_iter().zip(COLORS) {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(materials.add(color)),
            Transform::from_translation(at),
            Grabbable,
        ));
    }
}

/// Where calibration is kept: `$XDG_CONFIG_HOME/terrain`, or `~/.config/terrain`.
fn calibration_file() -> Option<PathBuf> {
    // An empty or relative XDG_CONFIG_HOME is ignored, as the spec says.
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".config")))?;
    Some(config.join("terrain").join("calibration.json"))
}
