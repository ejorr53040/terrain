//! The terrain scene: hands from a `HandSource` pick up and move `Grabbable` entities.

mod build;
mod calibration;
mod cursor;
mod materials;
mod preview;
mod settings;
mod shape;
mod templates;
mod tools;

use std::{path::PathBuf, sync::Mutex};

use avian3d::prelude::*;
use bevy::{ecs::entity_disabling::Disabled, prelude::*};
use terrain_hands::{
    CameraModel, FORGET_AFTER_MS, HandFrame, HandSource, HandState, HandStateEstimator,
    TrackedHands,
};

pub use build::{OverlayArea, Placed, Selection};
pub use materials::{Material, Materials, Preset};
pub use settings::PhysicsSettings;
pub use shape::Shape;
pub use templates::{Library, Part, Requests, Template};
pub use tools::Tool;
pub use calibration::{CALIBRATION_DISTANCE_M, Calibration, load_camera};
pub use cursor::CursorPlugin;
pub use preview::{DEFAULT_CAMERA, PreviewPlugin, start_tracking};

/// Hand motion is amplified by this much when applied to a grabbed entity
/// in a scene with no camera. With one, the gain is set at the pinch so the
/// entity keeps pace with its hand's cursor on screen.
pub const GRAB_GAIN: f32 = 1.5;

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
}

/// A held entity closes the gap to where the hand wants it with this time
/// constant, in seconds: short enough to follow a quick move, long enough
/// not to jitter.
const STEER_SECONDS: f32 = 0.03;

/// The fastest a held entity is steered, in meters per second, so a jump in
/// tracking can't fling it or what it hits.
const STEER_MAX_SPEED: f32 = 4.0;

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
        .init_resource::<Tool>()
        .init_resource::<Materials>()
        .init_resource::<Library>()
        .init_resource::<Requests>()
        .init_resource::<PhysicsSettings>()
        // No gravity and no momentum: things only move while held or shoved.
        .add_plugins(PhysicsPlugins::default())
        .insert_resource(Gravity(Vec3::ZERO))
        .add_systems(FixedUpdate, steer)
        // Before the frame's physics ticks, so a step or a pause counts this frame.
        .add_systems(PreUpdate, settings::apply_physics)
        .add_systems(
            Update,
            (
                build_walls,
                read_hands,
                calibration::calibrate,
                remember_start,
                fit_colliders,
                reset,
                sync_cursors,
                drive_grab,
                settle_copies,
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
    /// The frame that arrived this update, as the tracker saw it.
    raw: Option<HandFrame>,
    /// Hands from a frame that arrived this update, not yet acted on.
    fresh: Option<TrackedHands>,
}

/// What each hand is holding: at most one entity per hand, one hand per entity.
#[derive(Resource, Default)]
pub(crate) struct Grab {
    held: Vec<Held>,
    /// The entity most recently picked up.
    last: Option<Entity>,
}

/// A grab in progress: what's held, by which hand, where the hand and the
/// entity were when the pinch began, and where the hand now wants it.
struct Held {
    entity: Entity,
    hand: HandState,
    transform: Transform,
    target: Transform,
    /// The entity's size when the pinch began, and the size hands now want.
    size: Vec3,
    target_size: Vec3,
    /// How far the entity moves per meter the hand moves, along each axis.
    gain: Vec3,
    /// Capture time of the last frame the holding hand was seen in.
    last_seen_ms: u64,
    /// A second hand on the same entity, stretching it.
    second: Option<Second>,
}

/// A second hand pinching an entity already held. While both pinch, the
/// entity follows their midpoint without turning, and its own axis most in
/// line with the hands is scaled by how far their gap has grown since the
/// second pinch. `Held::hand` and `Held::transform` are re-taken at that
/// pinch, so the midpoint's move is measured from there.
struct Second {
    hand: HandState,
    /// The entity's axis being stretched: 0, 1 or 2 for its x, y or z.
    axis: usize,
    last_seen_ms: u64,
}

/// The shortest a stretch makes a side, in meters. The longest is set by the Arena.
const STRETCH_MIN: f32 = 0.03;

impl Held {
    /// Carries on from where the entity is now, measuring `hand`'s motion from here.
    fn reclutch(&mut self, hand: &HandState) {
        self.hand = *hand;
        self.transform = self.target;
        self.size = self.target_size;
    }
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

/// A copy still being carried off its original.
#[derive(Component)]
struct Copying;

/// Makes a copy solid once it's let go.
fn settle_copies(mut commands: Commands, grab: Res<Grab>, copies: Query<Entity, With<Copying>>) {
    for entity in &copies {
        if grab.held.iter().all(|held| held.entity != entity) {
            commands.entity(entity).remove::<(Sensor, Copying)>();
        }
    }
}

/// Fits each shape's collider to its parameters when they change.
// ponytail: rebuilt every change, i.e. every frame of a stretch; throttle if that shows up in a profile.
fn fit_colliders(mut commands: Commands, shapes: Query<(Entity, &Shape), Changed<Shape>>) {
    for (entity, shape) in &shapes {
        commands
            .entity(entity)
            .insert(shape.collider());
    }
}

/// Pushes each held entity toward its hand's target by setting its velocity,
/// so it shoves what's in the way and is stopped by what won't move. With no
/// gravity every other entity is stilled, so it stays where it was let go or
/// shoved to; with gravity they fall, bounce and slide freely.
fn steer(
    grab: Res<Grab>,
    gravity: Res<Gravity>,
    mut bodies: Query<
        (
            Entity,
            &Transform,
            &mut LinearVelocity,
            &mut AngularVelocity,
        ),
        With<Grabbable>,
    >,
) {
    for (entity, at, mut linear, mut angular) in &mut bodies {
        let Some(held) = grab.held.iter().find(|h| h.entity == entity) else {
            if gravity.0 == Vec3::ZERO {
                linear.0 = Vec3::ZERO;
                angular.0 = Vec3::ZERO;
            }
            continue;
        };
        linear.0 = ((held.target.translation - at.translation) / STEER_SECONDS)
            .clamp_length_max(STEER_MAX_SPEED);
        // The short way round to the target's turn.
        let mut turn = held.target.rotation * at.rotation.inverse();
        if turn.w < 0.0 {
            turn = -turn;
        }
        angular.0 = turn.to_scaled_axis() / STEER_SECONDS;
    }
}

/// `R`, or a fresh calibration, puts the scene back as it began: every
/// `Grabbable` it began with where it started, deleted ones too, placed ones
/// gone, and nothing held.
fn reset(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
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
    if keys.just_pressed(KeyCode::KeyR) || calibrated {
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

/// The camera the scene is drawn from, for where things appear on screen.
pub(crate) type SceneCamera<'w, 's> =
    Query<'w, 's, (&'static Transform, &'static Projection), (With<Camera3d>, Without<Grabbable>)>;

/// The scene camera's view. Points on screen are in clip space with x
/// scaled by the aspect ratio, so distances are as the eye sees them; a hand
/// or cursor's place in the view is 0 to 1 across and down from the top left.
pub(crate) struct Screen {
    clip_from_world: Mat4,
    aspect: f32,
}

impl Screen {
    /// `None` without a scene camera. The scene has one 3D camera, unparented.
    pub(crate) fn of(scene_camera: &SceneCamera) -> Option<Self> {
        let (camera, projection) = scene_camera.single().ok()?;
        Some(Self {
            clip_from_world: projection.get_clip_from_view() * camera.to_matrix().inverse(),
            aspect: match projection {
                Projection::Perspective(p) => p.aspect_ratio,
                _ => 1.0,
            },
        })
    }

    /// Where a point lands on screen; `None` behind the camera.
    pub(crate) fn project(&self, at: Vec3) -> Option<Vec2> {
        let clip = self.clip_from_world * at.extend(1.0);
        (clip.w > 0.0).then(|| clip.truncate().truncate() / clip.w * Vec2::new(self.aspect, 1.0))
    }

    /// A place in the view, in the same terms as a projected point.
    pub(crate) fn pointing(&self, in_view: Vec2) -> Vec2 {
        Vec2::new(2.0 * in_view.x - 1.0, 1.0 - 2.0 * in_view.y) * Vec2::new(self.aspect, 1.0)
    }

    /// Whether `in_view` is on a shape's outline on screen.
    pub(crate) fn covers(&self, t: &Transform, shape: &Shape, in_view: Vec2) -> bool {
        // ponytail: the outline is the corners' bounding rectangle, a little
        // generous on a turned cube; use their convex hull if that misfires.
        let corners = (0..8).filter_map(|i| {
            let corner = Vec3::new(
                if i & 1 == 0 { -0.5 } else { 0.5 },
                if i & 2 == 0 { -0.5 } else { 0.5 },
                if i & 4 == 0 { -0.5 } else { 0.5 },
            );
            self.project(t.transform_point(corner * shape.size))
        });
        let (min, max) = corners.fold((Vec2::INFINITY, Vec2::NEG_INFINITY), |(min, max), c| {
            (min.min(c), max.max(c))
        });
        let pointing = self.pointing(in_view);
        pointing.cmpge(min).all() && pointing.cmple(max).all()
    }

    /// Where the line of sight through `in_view` meets the plane `z = 0`,
    /// which faces the camera through the middle of the scene.
    pub(crate) fn on_middle_plane(&self, in_view: Vec2) -> Option<Vec3> {
        let world_from_clip = self.clip_from_world.inverse();
        let ndc = self.pointing(in_view) / Vec2::new(self.aspect, 1.0);
        // Two depths in front of the camera, finite under Bevy's reversed-z projection.
        let near = world_from_clip.project_point3(ndc.extend(1.0));
        let far = world_from_clip.project_point3(ndc.extend(0.5));
        let along = far - near;
        let t = -near.z / along.z;
        (t.is_finite() && t > 0.0).then(|| near + t * along)
    }
}

/// Relative clutch: while pinched, a held entity moves by its hand's motion
/// since the pinch began, times `GRAB_GAIN`, and turns by the hand's turn
/// since then. The hand's space (x right, y up, z away from the camera,
/// i.e. toward the viewer) lines up with the scene's axes.
///
/// Each hand holds at most one entity and each entity is held by at most
/// one hand; a new pinch takes the free entity nearest the hand on screen.
/// If a holding hand drops out of tracking its entity freezes, and is let
/// go once the hand has been gone longer than `FORGET_AFTER_MS`.
#[allow(clippy::too_many_arguments)]
fn drive_grab(
    mut commands: Commands,
    tool: Res<Tool>,
    made_of: Query<&Material>,
    mut hands: ResMut<Hands>,
    mut grab: ResMut<Grab>,
    mut grabbables: Query<(Entity, &Transform, &mut Shape), With<Grabbable>>,
    scene_camera: SceneCamera,
    arena: Res<Arena>,
) {
    let Some(frame) = hands.fresh.take() else {
        return;
    };
    // Checked against each new frame, not only hand-less ones: the tracker
    // may go quiet and come back with the hand already there.
    grab.held
        .retain(|held| frame.t_ms.saturating_sub(held.last_seen_ms) <= FORGET_AFTER_MS);
    grab.held.retain_mut(|held| {
        let first = frame.hands.iter().find(|h| h.id == held.hand.id);
        let Some(second) = &mut held.second else {
            return match first {
                Some(hand) if hand.pinching => {
                    held.last_seen_ms = frame.t_ms;
                    held.target.translation = held.transform.translation
                        + held.gain * (hand.position - held.hand.position);
                    // The hand's turn since the pinch, applied in the scene frame (on the left).
                    held.target.rotation =
                        hand.rotation * held.hand.rotation.inverse() * held.transform.rotation;
                    true
                }
                Some(_) => false,
                None => true,
            };
        };
        let other = frame.hands.iter().find(|h| h.id == second.hand.id);
        if other.is_some() {
            second.last_seen_ms = frame.t_ms;
        }
        match (first, other) {
            (Some(a), Some(b)) if a.pinching && b.pinching => {
                held.last_seen_ms = frame.t_ms;
                let gap = |a: &HandState, b: &HandState| a.position.distance(b.position).max(0.01);
                let mid = |a: &HandState, b: &HandState| (a.position + b.position) / 2.0;
                held.target.translation = held.transform.translation
                    + held.gain * (mid(a, b) - mid(&held.hand, &second.hand));
                held.target.rotation = held.transform.rotation;
                held.target_size[second.axis] = (held.size[second.axis] * gap(a, b)
                    / gap(&held.hand, &second.hand))
                .clamp(STRETCH_MIN, arena.longest());
                true
            }
            // The first hand let go: the second carries on alone.
            (Some(a), Some(b)) if !a.pinching && b.pinching => {
                let b = *b;
                held.second = None;
                held.last_seen_ms = frame.t_ms;
                held.reclutch(&b);
                true
            }
            (Some(a), _) if !a.pinching => false,
            // The second hand let go, or has been gone too long: the first carries on alone.
            (Some(a), b)
                if b.is_some_and(|b| !b.pinching)
                    || frame.t_ms.saturating_sub(second.last_seen_ms) > FORGET_AFTER_MS =>
            {
                let a = *a;
                held.second = None;
                held.reclutch(&a);
                true
            }
            // A hand is out of view for now: hold still.
            _ => true,
        }
    });
    // Only the steer moves and turns held entities, so physics can stop them; a stretch is set here.
    for held in &grab.held {
        if let Ok((_, _, mut shape)) = grabbables.get_mut(held.entity)
            && shape.size != held.target_size
        {
            shape.size = held.target_size;
        }
    }
    let screen = Screen::of(&scene_camera);
    for hand in frame.hands.iter().filter(|h| h.pinch_started) {
        if grab.held.iter().any(|held| {
            held.hand.id == hand.id || held.second.as_ref().is_some_and(|s| s.hand.id == hand.id)
        }) {
            continue;
        }
        // On an entity held by one hand only: the pinch joins in to stretch it.
        let joined = screen.as_ref().and_then(|screen| {
            grab.held.iter_mut().find(|held| {
                held.second.is_none()
                    && grabbables
                        .get(held.entity)
                        .is_ok_and(|(_, t, shape)| screen.covers(t, shape, hand.in_image))
            })
        });
        if let Some(held) = joined {
            let Some(first) = frame.hands.iter().find(|h| h.id == held.hand.id) else {
                continue;
            };
            let along = (hand.position - first.position).normalize_or(Vec3::X);
            let axes =
                [Vec3::X, Vec3::Y, Vec3::Z].map(|a| (held.target.rotation * a).dot(along).abs());
            let axis = (0..3).max_by(|&i, &j| axes[i].total_cmp(&axes[j])).unwrap();
            held.reclutch(first);
            held.second = Some(Second {
                hand: *hand,
                axis,
                last_seen_ms: frame.t_ms,
            });
            continue;
        }
        let free = grabbables
            .iter()
            .filter(|(entity, ..)| grab.held.iter().all(|held| held.entity != *entity));
        let nearest = match &screen {
            Some(screen) => free
                .filter_map(|(entity, t, shape)| {
                    Some((entity, (t, shape), screen.project(t.translation)?))
                })
                .min_by(|(_, _, a), (_, _, b)| {
                    let pointing = screen.pointing(hand.in_image);
                    a.distance(pointing).total_cmp(&b.distance(pointing))
                })
                .map(|(entity, t, _)| (entity, t)),
            // No scene camera, so no screen to be near on: any free one.
            None => free
                .map(|(entity, t, shape)| (entity, (t, shape)))
                .min_by_key(|(entity, _)| *entity),
        };
        if let Some((mut entity, (transform, shape))) = nearest {
            // Copy grabs a new shape in the original's place; it passes through
            // the original until let go.
            if *tool == Tool::Copy {
                if grabbables.iter().count() >= arena.cap {
                    continue;
                }
                let material = made_of.get(entity).copied().unwrap_or_default();
                entity = commands
                    .spawn((Grabbable, Placed, *shape, material, *transform, Sensor, Copying))
                    .id();
            }
            // The view each camera has, at the hand's and the entity's depth,
            // so a move fills the same share of the screen as the cursor's.
            let gain = match scene_camera.single() {
                Ok((camera, Projection::Perspective(p))) => {
                    let depth = (transform.translation - camera.translation).dot(*camera.forward());
                    let scene_height = 2.0 * depth * (p.fov / 2.0).tan();
                    let webcam = hands.estimator.camera();
                    let webcam_width =
                        2.0 * hand.position.z.max(0.1) * (webcam.hfov_deg.to_radians() / 2.0).tan();
                    let across = scene_height * p.aspect_ratio / webcam_width;
                    let up = scene_height / (webcam_width / webcam.aspect);
                    // Toward and away has no cursor to keep pace with; it feels as across does.
                    Vec3::new(across, up, across)
                }
                _ => Vec3::splat(GRAB_GAIN),
            };
            grab.last = Some(entity);
            grab.held.push(Held {
                entity,
                hand: *hand,
                transform: *transform,
                target: *transform,
                size: shape.size,
                target_size: shape.size,
                gain,
                last_seen_ms: frame.t_ms,
                second: None,
            });
        }
    }
}
