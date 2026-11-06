//! The terrain scene: hands from a `HandSource` pick up and move `Grabbable` entities.

mod build;
mod calibration;
mod cursor;
mod grab;
mod materials;
mod overlay;
mod preview;
mod requests;
mod screen;
mod settings;
mod shape;
mod templates;
mod tools;

use std::{path::PathBuf, sync::Mutex};

use avian3d::prelude::*;
use bevy::{ecs::entity_disabling::Disabled, prelude::*};
use terrain_hands::{CameraModel, HandFrame, HandSource, HandStateEstimator, TrackedHands};

pub use build::{Placed, Selection};
pub use calibration::{CALIBRATION_DISTANCE_M, Calibration};
pub use cursor::CursorPlugin;
pub use grab::GRAB_GAIN;
use grab::Grab;
pub use materials::{Material, Materials, Preset};
pub use overlay::{OverlayArea, OverlayOpen, OverlayPlugin, OverlayPointer};
pub use preview::PreviewPlugin;
pub use requests::Requests;
pub use settings::PhysicsSettings;
pub use shape::Shape;
pub use templates::{Library, Part, Template};
pub use tools::Tool;

/// Where the scene camera sits; it looks at the origin.
pub const SCENE_CAMERA_AT: Vec3 = Vec3::new(0.0, 0.25, 1.2);

/// How wide each cube is, in meters.
pub const CUBE_SIZE: f32 = 0.15;

/// The box shapes live in. Its walls are rebuilt whenever it changes.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Arena {
    /// Opposite corners of the walls, in scene meters.
    pub min: Vec3,
    pub max: Vec3,
    /// The grid that placed shapes snap to, in meters.
    pub cell: f32,
    /// The most shapes it holds, so physics stays smooth.
    pub cap: usize,
}

impl Default for Arena {
    /// Sized so the whole of it is in view of the scene camera.
    fn default() -> Self {
        Self {
            min: Vec3::new(-0.65, -0.3, -0.4),
            max: Vec3::new(0.65, 0.4, 0.2),
            cell: CUBE_SIZE,
            cap: 64,
        }
    }
}

impl Arena {
    /// The longest a shape may be stretched: half a cell short of the
    /// Arena's shallowest side, so it can still turn a little in there.
    pub fn longest(&self) -> f32 {
        (self.max - self.min).min_element() - self.cell / 2.0
    }

    /// The grid cell `at` is in, kept inside the Arena.
    pub(crate) fn cell_of(&self, at: Vec3) -> IVec3 {
        ((at - self.min) / self.cell)
            .floor()
            .as_ivec3()
            .clamp(IVec3::ZERO, self.cells() - IVec3::ONE)
    }

    /// How many whole cells fit along each side.
    pub(crate) fn cells(&self) -> IVec3 {
        ((self.max - self.min) / self.cell)
            .floor()
            .as_ivec3()
            .max(IVec3::ONE)
    }

    pub(crate) fn middle_of(&self, cell: IVec3) -> Vec3 {
        self.min + (cell.as_vec3() + 0.5) * self.cell
    }

    /// Whether a shape placed at `at` would start inside one of `shapes`.
    // ponytail: treats every shape as one cell big; a stretched shape can still be overlapped.
    pub(crate) fn taken(&self, at: Vec3, shapes: &[Vec3]) -> bool {
        shapes
            .iter()
            .any(|s| (*s - at).abs().max_element() < self.cell - 1e-3)
    }
}

/// Where the cubes start: a row across the view, 20 cm apart.
pub const CUBE_STARTS: [Vec3; 5] = [
    Vec3::new(-0.4, 0.0, 0.0),
    Vec3::new(-0.2, 0.0, 0.0),
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(0.2, 0.0, 0.0),
    Vec3::new(0.4, 0.0, 0.0),
];

/// The bundled demo replay: a pinch that drags the cube around a circle.
pub const DEMO_FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/demo-drag.json");

/// An entity hands can pick up and move.
#[derive(Component)]
#[require(Shape, Material)]
pub struct Grabbable;

/// Marks where a tracked hand is on screen, one entity per hand.
#[derive(Component, Clone, Copy, Debug)]
pub struct HandCursor {
    /// The hand it follows.
    pub id: u32,
    /// Where the hand is in the view: 0 to 1 across and down from the top left.
    pub at: Vec2,
    pub pinching: bool,
}

/// Where a `Grabbable` started and its shape then, for `R` to put it back.
#[derive(Component)]
struct Start(Transform, Shape);

/// Reads hands from a `HandSource` and lets a pinch grab, move and release
/// `Grabbable` entities.
pub struct GrabPlugin {
    source: Mutex<Option<Box<dyn HandSource>>>,
    camera: CameraModel,
    save_calibration_to: Option<PathBuf>,
}

impl GrabPlugin {
    pub fn new(source: impl HandSource + 'static) -> Self {
        Self {
            source: Mutex::new(Some(Box::new(source))),
            camera: CameraModel::default(),
            save_calibration_to: None,
        }
    }

    /// Starts with `camera` (say, a saved calibration) instead of the default.
    pub fn with_camera(self, camera: CameraModel) -> Self {
        Self { camera, ..self }
    }

    /// Saves each successful calibration to `path`.
    pub fn saving_calibration_to(self, path: PathBuf) -> Self {
        Self {
            save_calibration_to: Some(path),
            ..self
        }
    }
}

impl Plugin for GrabPlugin {
    fn build(&self, app: &mut App) {
        let source = self
            .source
            .lock()
            .unwrap()
            .take()
            .expect("GrabPlugin added twice");
        app.insert_resource(Hands {
            source: Mutex::new(source),
            estimator: HandStateEstimator::new(self.camera),
            raw: None,
            fresh: None,
        })
        .insert_resource(Calibration::new(
            self.camera,
            self.save_calibration_to.clone(),
        ))
        // Present under DefaultPlugins; headless apps press keys by hand.
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<Grab>()
        .init_resource::<Arena>()
        .init_resource::<Selection>()
        .init_resource::<OverlayArea>()
        .init_resource::<OverlayOpen>()
        .init_resource::<OverlayPointer>()
        .init_resource::<Tool>()
        .init_resource::<Materials>()
        .init_resource::<Library>()
        .init_resource::<Requests>()
        .init_resource::<PhysicsSettings>()
        // No gravity and no momentum: things only move while held or shoved.
        .add_plugins(PhysicsPlugins::default())
        .insert_resource(Gravity(Vec3::ZERO))
        .add_systems(FixedUpdate, grab::steer)
        // Before the frame's physics ticks, so a step or a pause counts this frame.
        .add_systems(PreUpdate, settings::apply_physics)
        .add_systems(
            Update,
            (
                requests::press_keys,
                build_walls,
                read_hands,
                calibration::calibrate,
                remember_start,
                fit_colliders,
                reset,
                sync_cursors,
                grab::drive_grab,
                overlay::hot_edge,
                overlay::point_at_overlay,
                grab::settle_copies,
                build::select,
                build::edit,
                tools::use_tools,
                materials::apply_materials,
                templates::use_templates,
            )
                .chain(),
        );
    }
}

#[derive(Resource)]
pub(crate) struct Hands {
    source: Mutex<Box<dyn HandSource>>,
    estimator: HandStateEstimator,
    /// The frame that arrived this update, unsmoothed.
    raw: Option<HandFrame>,
    /// Hands from a frame that arrived this update, not yet acted on.
    fresh: Option<TrackedHands>,
}

/// One of the Arena's six walls.
#[derive(Component)]
struct Wall;

/// Closes the Arena in with six solid, immovable walls, so nothing held,
/// shoved or let go of can leave it; rebuilt when the Arena changes.
fn build_walls(mut commands: Commands, arena: Res<Arena>, walls: Query<Entity, With<Wall>>) {
    if !arena.is_changed() {
        return;
    }
    for wall in &walls {
        commands.entity(wall).despawn();
    }
    // Thick enough that a fast cube can't step through in one physics tick.
    const THICKNESS: f32 = 1.0;
    let Arena { min, max, .. } = *arena;
    let size = max - min;
    for axis in 0..3 {
        for (face, outward) in [(min[axis], -1.0), (max[axis], 1.0)] {
            let mut centre = (min + max) / 2.0;
            centre[axis] = face + outward * THICKNESS / 2.0;
            let mut extent = size + Vec3::splat(2.0 * THICKNESS);
            extent[axis] = THICKNESS;
            commands.spawn((
                Wall,
                RigidBody::Static,
                Collider::cuboid(extent.x, extent.y, extent.z),
                Transform::from_translation(centre),
            ));
        }
    }
}

/// Gives each new `Grabbable` a place to reset to and a solid body.
fn remember_start(
    mut commands: Commands,
    new: Query<(Entity, &Transform, &Shape), Added<Grabbable>>,
) {
    for (entity, transform, shape) in &new {
        commands
            .entity(entity)
            .insert((Start(*transform, *shape), RigidBody::Dynamic));
    }
}

/// Fits each shape's collider to its parameters when they change.
// ponytail: rebuilt every change, i.e. every frame of a stretch; throttle if that shows up in a profile.
fn fit_colliders(mut commands: Commands, shapes: Query<(Entity, &Shape), Changed<Shape>>) {
    for (entity, shape) in &shapes {
        commands.entity(entity).insert(shape.collider());
    }
}

/// `R` (or its button), or a fresh calibration, puts the scene back as it began: every
/// `Grabbable` it began with where it started, deleted ones too, placed ones
/// gone, and nothing held.
fn reset(
    mut commands: Commands,
    mut requests: ResMut<Requests>,
    mut calibration: ResMut<Calibration>,
    mut grab: ResMut<Grab>,
    mut grabbables: Query<(
        Entity,
        &mut Transform,
        &mut Shape,
        &Start,
        Has<Placed>,
        Has<Disabled>,
    )>,
) {
    let calibrated = std::mem::take(&mut calibration.reset_scene);
    let asked = std::mem::take(&mut requests.reset);
    if asked || calibrated {
        grab.held.clear();
        grab.last = None;
        for (entity, mut transform, mut shape, start, placed, deleted) in &mut grabbables {
            if placed {
                commands.entity(entity).despawn();
                continue;
            }
            *transform = start.0;
            *shape = start.1;
            if deleted {
                commands.entity(entity).remove::<Disabled>();
            }
        }
    }
}

fn read_hands(time: Res<Time>, mut hands: ResMut<Hands>) {
    let hands = &mut *hands;
    hands.raw = hands.source.get_mut().unwrap().next_frame(time.elapsed());
    if let Some(frame) = &hands.raw {
        hands.fresh = Some(hands.estimator.update(frame));
    }
}

/// Keeps one `HandCursor` per hand in the latest frame: made when a hand
/// appears, moved as it does, removed when it's gone.
fn sync_cursors(
    mut commands: Commands,
    hands: Res<Hands>,
    mut cursors: Query<(Entity, &mut HandCursor)>,
) {
    let Some(frame) = &hands.fresh else {
        return;
    };
    for (entity, mut cursor) in &mut cursors {
        match frame.hands.iter().find(|h| h.id == cursor.id) {
            Some(hand) => {
                cursor.at = hand.in_image;
                cursor.pinching = hand.pinching;
            }
            None => commands.entity(entity).despawn(),
        }
    }
    for hand in &frame.hands {
        if cursors.iter().all(|(_, cursor)| cursor.id != hand.id) {
            commands.spawn(HandCursor {
                id: hand.id,
                at: hand.in_image,
                pinching: hand.pinching,
            });
        }
    }
}
