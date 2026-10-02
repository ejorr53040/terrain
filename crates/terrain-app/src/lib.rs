//! The terrain scene: hands from a `HandSource` pick up and move `Grabbable` entities.

mod calibration;
mod preview;

use std::{path::PathBuf, sync::Mutex};

use bevy::prelude::*;
use terrain_hands::{
    CameraModel, FORGET_AFTER_MS, HandFrame, HandSource, HandState, HandStateEstimator,
    TrackedHands,
};

pub use calibration::{CALIBRATION_DISTANCE_M, Calibration, load_camera};
pub use preview::{DEFAULT_CAMERA, PreviewPlugin, start_tracking};

/// Hand motion is amplified by this much when applied to a grabbed entity,
/// so small, comfortable movements cover the scene.
pub const GRAB_GAIN: f32 = 1.5;

/// Where the scene camera sits; it looks at the origin.
pub const SCENE_CAMERA_AT: Vec3 = Vec3::new(0.0, 0.25, 1.2);

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
        .add_systems(
            Update,
            (
                read_hands,
                calibration::calibrate,
                remember_start,
                reset,
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

/// A grab in progress: what's held, by which hand, and where the hand and
/// the entity were when the pinch began.
struct Held {
    entity: Entity,
    hand: HandState,
    transform: Transform,
    /// Capture time of the last frame the holding hand was seen in.
    last_seen_ms: u64,
}

fn remember_start(mut commands: Commands, new: Query<(Entity, &Transform), Added<Grabbable>>) {
    for (entity, transform) in &new {
        commands.entity(entity).insert(Start(*transform));
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
        match frame.hands.iter().find(|h| h.id == held.hand.id) {
            Some(hand) if hand.pinching => {
                held.last_seen_ms = frame.t_ms;
                if let Ok((_, mut transform)) = grabbables.get_mut(held.entity) {
                    transform.translation = held.transform.translation
                        + GRAB_GAIN * (hand.position - held.hand.position);
                    // The hand's turn since the pinch, applied in the scene frame (on the left).
                    transform.rotation =
                        hand.rotation * held.hand.rotation.inverse() * held.transform.rotation;
                }
                true
            }
            Some(_) => false,
            None => true,
        }
    });
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
        if grab.held.iter().any(|held| held.hand.id == hand.id) {
            continue;
        }
        // The hand's place on screen, in the same terms as a projected point.
        let pointing = Vec2::new(2.0 * hand.in_image.x - 1.0, 1.0 - 2.0 * hand.in_image.y)
            * Vec2::new(aspect, 1.0);
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
            grab.held.push(Held {
                entity,
                hand: *hand,
                transform: *transform,
                last_seen_ms: frame.t_ms,
            });
        }
    }
}
