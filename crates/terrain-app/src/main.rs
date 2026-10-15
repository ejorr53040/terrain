//! terrain: a row of cubes you move with your hands, one in each.
//!
//! Hands come from the webcam (`--camera <device>`, default `/dev/video0`),
//! or from a recorded fixture with `--replay <fixture.json>` (looped; the
//! webcam then only feeds the preview).
//!
//! `C` calibrates the camera; the result is kept in
//! `~/.config/terrain/calibration.json` and loaded at start. `N` adds a box
//! at the cursor, `Delete` removes the outlined shape, `D` copies it and `R`
//! puts the scene back. `Tab` picks a template, `P` stamps it at the cursor
//! and `T` saves the selection's build as one, in
//! `~/.config/terrain/templates`. `O` shows the overlay: tools and the
//! template library with search.

mod overlay;

use std::path::PathBuf;

use bevy::prelude::*;
use terrain_app::{
    Arena, CUBE_STARTS, CursorPlugin, DEFAULT_CAMERA, GrabPlugin, Grabbable,
    Library, Material, Materials, PreviewPlugin, SCENE_CAMERA_AT, Selection, Shape, Tool, load_camera, start_tracking,
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
        // Saved templates sit beside the calibration.
        .insert_resource(Library::load(
            calibration_file().and_then(|f| Some(f.parent()?.join("templates"))),
        ))
        .add_plugins((CursorPlugin, PreviewPlugin::new(tracking), overlay::OverlayPlugin))
        .add_systems(Startup, spawn_scene)
        .add_systems(Update, (draw_box, paint_shapes, mesh_shapes, outline_selection, show_tool))
        .run()
}

/// The Arena's walls, as an outline.
fn draw_box(mut gizmos: Gizmos, arena: Res<Arena>) {
    gizmos.cube(
        Transform::from_translation((arena.min + arena.max) / 2.0).with_scale(arena.max - arena.min),
        Color::srgba(0.7, 0.85, 1.0, 0.6),
    );
}

/// Colours each shape by its material, again when it or the presets change.
fn paint_shapes(
    mut commands: Commands,
    presets: Res<Materials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shapes: Query<(Entity, Ref<Material>), With<Grabbable>>,
) {
    for (entity, material) in &shapes {
        if let Some(p) = presets.presets.get(material.0)
            && (material.is_changed() || presets.is_changed())
        {
            commands
                .entity(entity)
                .insert(MeshMaterial3d(materials.add(p.color)));
        }
    }
}

/// Draws each shape from its parameters, again whenever they change.
fn mesh_shapes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    shapes: Query<(Entity, &Shape), Changed<Shape>>,
) {
    for (entity, shape) in &shapes {
        commands
            .entity(entity)
            .insert(Mesh3d(meshes.add(shape.mesh())));
    }
}

/// A white box just outside the selected shape.
fn outline_selection(
    mut gizmos: Gizmos,
    selection: Res<Selection>,
    shapes: Query<(&Transform, &Shape)>,
) {
    if let Some((t, shape)) = selection.0.and_then(|e| shapes.get(e).ok()) {
        gizmos.cube(t.with_scale(shape.size + 0.02), Color::WHITE);
    }
}

/// Names the active tool, bottom left.
#[derive(Component)]
struct ToolReadout;

/// The tool, and what the selection is made of.
fn show_tool(
    tool: Res<Tool>,
    selection: Res<Selection>,
    presets: Res<Materials>,
    library: Res<Library>,
    made_of: Query<&Material>,
    mut readout: Query<&mut Text, With<ToolReadout>>,
) {
    let material = selection
        .0
        .and_then(|e| made_of.get(e).ok())
        .and_then(|m| presets.presets.get(m.0))
        .map_or(String::new(), |p| format!("   {} (M next)", p.name));
    let template = library
        .templates
        .get(library.current)
        .map_or(String::new(), |t| format!("\ntemplate {} (Tab next, P stamp, T save)", t.name));
    let skipped = match library.errors.len() {
        0 => String::new(),
        n => format!("\n{n} template file(s) skipped, see log"),
    };
    let line = format!("{:?} tool  (1-4 pick, [ ] adjust){material}{template}{skipped}", *tool);
    for mut text in &mut readout {
        if text.0 != line {
            text.0 = line.clone();
        }
    }
}

fn spawn_scene(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(SCENE_CAMERA_AT).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        ToolReadout,
        Text::new(""),
        TextFont::from_font_size(16.0),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12),
            left: px(12),
            ..default()
        },
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            ..default()
        },
        Transform::from_xyz(1.0, 2.0, 1.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // A row of cubes, one of each material.
    for (i, at) in CUBE_STARTS.into_iter().enumerate() {
        commands.spawn((
            Material(i),
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
