//! The terrain scene: hands from a `HandSource` pick up and move `Grabbable` entities.

mod calibration;
mod cursor;
mod preview;

use std::{path::PathBuf, sync::Mutex};

use avian3d::prelude::*;
use bevy::prelude::*;
use terrain_hands::{
    CameraModel, FORGET_AFTER_MS, HandFrame, HandSource, HandState, HandStateEstimator,
    TrackedHands,
};

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

/// The box the cubes live in: its walls, in scene meters. Sized so the whole
/// of it is in view of the scene camera.
pub const BOX_MIN: Vec3 = Vec3::new(-0.65, -0.3, -0.4);
pub const BOX_MAX: Vec3 = Vec3::new(0.65, 0.4, 0.2);

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

/// Where a `Grabbable` started, for `R` to put it back.
#[derive(Component)]
struct Start(Transform);

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
        // No gravity and no momentum: things only move while held or shoved.
        .add_plugins(PhysicsPlugins::default())
        .insert_resource(Gravity(Vec3::ZERO))
        .add_systems(Startup, spawn_walls)
        .add_systems(FixedUpdate, steer)
        .add_systems(
            Update,
            (
                read_hands,
                calibration::calibrate,
                remember_start,
                reset,
                sync_cursors,
                drive_grab,
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
struct Grab {
    held: Vec<Held>,
}

/// A grab in progress: what's held, by which hand, where the hand and the
/// entity were when the pinch began, and where the hand now wants it.
struct Held {
    entity: Entity,
    hand: HandState,
    transform: Transform,
    target: Transform,
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

/// The smallest and largest a stretched entity can be, as a multiple of its
/// unstretched size. The largest keeps a cube shorter than the box is deep.
const STRETCH_MIN: f32 = 0.2;
const STRETCH_MAX: f32 = 3.5;

impl Held {
    /// Carries on from where the entity is now, measuring `hand`'s motion from here.
    fn reclutch(&mut self, hand: &HandState) {
        self.hand = *hand;
        self.transform = self.target;
    }
}

/// Closes the box in with six solid, immovable walls, so nothing held,
/// shoved or let go of can leave it.
fn spawn_walls(mut commands: Commands) {
    // Thick enough that a fast cube can't step through in one physics tick.
    const THICKNESS: f32 = 1.0;
    let size = BOX_MAX - BOX_MIN;
    for axis in 0..3 {
        for (face, outward) in [(BOX_MIN[axis], -1.0), (BOX_MAX[axis], 1.0)] {
            let mut centre = (BOX_MIN + BOX_MAX) / 2.0;
            centre[axis] = face + outward * THICKNESS / 2.0;
            let mut extent = size + Vec3::splat(2.0 * THICKNESS);
            extent[axis] = THICKNESS;
            commands.spawn((
                RigidBody::Static,
                Collider::cuboid(extent.x, extent.y, extent.z),
                Transform::from_translation(centre),
            ));
        }
    }
}

/// Gives each new `Grabbable` a place to reset to and a solid cube body.
fn remember_start(mut commands: Commands, new: Query<(Entity, &Transform), Added<Grabbable>>) {
    for (entity, transform) in &new {
        commands.entity(entity).insert((
            Start(*transform),
            RigidBody::Dynamic,
            Collider::cuboid(CUBE_SIZE, CUBE_SIZE, CUBE_SIZE),
        ));
    }
}

/// Pushes each held entity toward its hand's target by setting its velocity,
/// so it shoves what's in the way and is stopped by what won't move. Every
/// other entity is stilled, so it stays where it was let go or shoved to.
fn steer(
    grab: Res<Grab>,
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
            linear.0 = Vec3::ZERO;
            angular.0 = Vec3::ZERO;
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

/// `R`, or a fresh calibration, puts every `Grabbable` back where it
/// started and lets go of it.
fn reset(
    keys: Res<ButtonInput<KeyCode>>,
    mut calibration: ResMut<Calibration>,
    mut grab: ResMut<Grab>,
    mut grabbables: Query<(&mut Transform, &Start)>,
) {
    let calibrated = std::mem::take(&mut calibration.reset_scene);
    if keys.just_pressed(KeyCode::KeyR) || calibrated {
        grab.held.clear();
        for (mut transform, start) in &mut grabbables {
            *transform = start.0;
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
type SceneCamera<'w, 's> =
    Query<'w, 's, (&'static Transform, &'static Projection), (With<Camera3d>, Without<Grabbable>)>;

/// Relative clutch: while pinched, a held entity moves by its hand's motion
/// since the pinch began, times `GRAB_GAIN`, and turns by the hand's turn
/// since then. The hand's space (x right, y up, z away from the camera,
/// i.e. toward the viewer) lines up with the scene's axes.
///
/// Each hand holds at most one entity and each entity is held by at most
/// one hand; a new pinch takes the free entity nearest the hand on screen.
/// If a holding hand drops out of tracking its entity freezes, and is let
/// go once the hand has been gone longer than `FORGET_AFTER_MS`.
fn drive_grab(
    mut hands: ResMut<Hands>,
    mut grab: ResMut<Grab>,
    mut grabbables: Query<(Entity, &mut Transform), With<Grabbable>>,
    scene_camera: SceneCamera,
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
                held.target.scale[second.axis] = (held.transform.scale[second.axis] * gap(a, b)
                    / gap(&held.hand, &second.hand))
                .clamp(STRETCH_MIN, STRETCH_MAX);
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
        if let Ok((_, mut transform)) = grabbables.get_mut(held.entity) {
            transform.scale = held.target.scale;
        }
    }
    // Screen width over height, so distances on screen are as the eye sees them.
    let aspect = match scene_camera.single() {
        Ok((_, Projection::Perspective(p))) => p.aspect_ratio,
        _ => 1.0,
    };
    // Where a point lands on screen, in clip space with x scaled by `aspect`;
    // `None` behind the camera. The scene has one 3D camera, unparented.
    let on_screen = scene_camera.single().ok().map(|(camera, projection)| {
        let clip_from_world = projection.get_clip_from_view() * camera.to_matrix().inverse();
        move |at: Vec3| {
            let clip = clip_from_world * at.extend(1.0);
            (clip.w > 0.0).then(|| clip.truncate().truncate() / clip.w * Vec2::new(aspect, 1.0))
        }
    });
    for hand in frame.hands.iter().filter(|h| h.pinch_started) {
        if grab.held.iter().any(|held| {
            held.hand.id == hand.id || held.second.as_ref().is_some_and(|s| s.hand.id == hand.id)
        }) {
            continue;
        }
        // The hand's place on screen, in the same terms as a projected point.
        let pointing = Vec2::new(2.0 * hand.in_image.x - 1.0, 1.0 - 2.0 * hand.in_image.y)
            * Vec2::new(aspect, 1.0);
        // On an entity held by one hand only: the pinch joins in to stretch it.
        let joined = on_screen.as_ref().and_then(|project| {
            grab.held.iter_mut().find(|held| {
                held.second.is_none()
                    && grabbables.get(held.entity).is_ok_and(|(_, t)| {
                        // ponytail: the outline is the corners' bounding rectangle, a little
                        // generous on a turned cube; use their convex hull if that misfires.
                        let corners = (0..8).filter_map(|i| {
                            let corner = Vec3::new(
                                if i & 1 == 0 { -0.5 } else { 0.5 },
                                if i & 2 == 0 { -0.5 } else { 0.5 },
                                if i & 4 == 0 { -0.5 } else { 0.5 },
                            );
                            project(t.transform_point(corner * CUBE_SIZE))
                        });
                        let (min, max) = corners
                            .fold((Vec2::INFINITY, Vec2::NEG_INFINITY), |(min, max), c| {
                                (min.min(c), max.max(c))
                            });
                        pointing.cmpge(min).all() && pointing.cmple(max).all()
                    })
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
            .filter(|(entity, _)| grab.held.iter().all(|held| held.entity != *entity));
        let nearest = match &on_screen {
            Some(project) => free
                .filter_map(|(entity, t)| Some((entity, t, project(t.translation)?)))
                .min_by(|(_, _, a), (_, _, b)| {
                    a.distance(pointing).total_cmp(&b.distance(pointing))
                })
                .map(|(entity, t, _)| (entity, t)),
            // No scene camera, so no screen to be near on: any free one.
            None => free.min_by_key(|(entity, _)| *entity),
        };
        if let Some((entity, transform)) = nearest {
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
            grab.held.push(Held {
                entity,
                hand: *hand,
                transform: *transform,
                target: *transform,
                gain,
                last_seen_ms: frame.t_ms,
                second: None,
            });
        }
    }
}
