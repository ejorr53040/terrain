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

/// The entity being held, if any.
#[derive(Resource, Default)]
struct Grab {
    held: Option<Held>,
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
        grab.held = None;
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

/// Relative clutch: while pinched, the entity moves by the hand's motion
/// since the pinch began, times `GRAB_GAIN`, and turns by the hand's turn
/// since then. Camera space (x right, y up, z away from the camera, i.e.
/// toward the viewer) lines up with the scene's axes.
///
/// The first hand to pinch holds the entity; the other hand is ignored until
/// it lets go, and then only grabs by starting a new pinch. If the holding hand drops out of tracking the entity freezes,
/// and is released once the hand has been gone longer than `FORGET_AFTER_MS`.
fn drive_grab(
    mut hands: ResMut<Hands>,
    mut grab: ResMut<Grab>,
    mut grabbables: Query<(Entity, &mut Transform), With<Grabbable>>,
) {
    let Some(frame) = hands.fresh.take() else {
        return;
    };
    // Checked against each new frame, not only hand-less ones: the tracker
    // may go quiet and come back with the hand already there.
    if grab
        .held
        .as_ref()
        .is_some_and(|held| frame.t_ms.saturating_sub(held.last_seen_ms) > FORGET_AFTER_MS)
    {
        grab.held = None;
    }
    let Some(held) = &mut grab.held else {
        if let Some(hand) = frame.hands.iter().find(|h| h.pinch_started)
            && let Some((entity, transform)) = grabbables.iter().next()
        {
            grab.held = Some(Held {
                entity,
                hand: *hand,
                transform: *transform,
                last_seen_ms: frame.t_ms,
            });
        }
        return;
    };
    match frame.hands.iter().find(|h| h.id == held.hand.id) {
        Some(hand) if hand.pinching => {
            held.last_seen_ms = frame.t_ms;
            if let Ok((_, mut transform)) = grabbables.get_mut(held.entity) {
                transform.translation =
                    held.transform.translation + GRAB_GAIN * (hand.position - held.hand.position);
                // The hand's turn since the pinch, applied in the scene frame (on the left).
                transform.rotation =
                    hand.rotation * held.hand.rotation.inverse() * held.transform.rotation;
            }
        }
        Some(_) => grab.held = None,
        None => {}
    }
}
