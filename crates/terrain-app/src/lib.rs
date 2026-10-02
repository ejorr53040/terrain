//! The terrain scene: hands from a `HandSource` pick up and move `Grabbable` entities.

use std::sync::Mutex;

use bevy::prelude::*;
use terrain_hands::{HandSource, HandState, HandStateEstimator};

/// Hand motion is amplified by this much when applied to a grabbed entity,
/// so small, comfortable movements cover the scene.
pub const GAIN: f32 = 1.5;

/// An entity hands can pick up and move.
#[derive(Component)]
pub struct Grabbable;

/// Reads hands from a `HandSource` and lets a pinch grab, move and release
/// `Grabbable` entities.
pub struct GrabPlugin {
    source: Mutex<Option<Box<dyn HandSource>>>,
}

impl GrabPlugin {
    pub fn new(source: impl HandSource + 'static) -> Self {
        Self {
            source: Mutex::new(Some(Box::new(source))),
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
            estimator: HandStateEstimator::default(),
            current: None,
        })
        .init_resource::<Grab>()
        .add_systems(Update, (read_hands, drive_grab).chain());
    }
}

#[derive(Resource)]
struct Hands {
    source: Mutex<Box<dyn HandSource>>,
    estimator: HandStateEstimator,
    /// The latest hand state; kept between frames when the source has nothing new.
    current: Option<HandState>,
}

/// The entity being held, and where it and the hand were when the pinch began.
#[derive(Resource, Default)]
struct Grab(Option<GrabStart>);

struct GrabStart {
    entity: Entity,
    hand: Vec3,
    transform: Transform,
}

fn read_hands(mut hands: ResMut<Hands>) {
    let hands = &mut *hands;
    if let Some(frame) = hands.source.get_mut().unwrap().next_frame() {
        hands.current = hands.estimator.update(&frame);
    }
}

/// Relative clutch: while pinched, the entity moves by the hand's motion since
/// the pinch began, times `GAIN`. Camera space (x right, y up, z away from the
/// camera, i.e. toward the viewer) lines up with the scene's axes.
fn drive_grab(
    hands: Res<Hands>,
    mut grab: ResMut<Grab>,
    mut grabbables: Query<(Entity, &mut Transform), With<Grabbable>>,
) {
    let Some(hand) = hands.current.filter(|h| h.pinching) else {
        grab.0 = None;
        return;
    };
    match &grab.0 {
        None => {
            if let Some((entity, transform)) = grabbables.iter().next() {
                grab.0 = Some(GrabStart {
                    entity,
                    hand: hand.position,
                    transform: *transform,
                });
            }
        }
        Some(start) => {
            if let Ok((_, mut transform)) = grabbables.get_mut(start.entity) {
                transform.translation =
                    start.transform.translation + GAIN * (hand.position - start.hand);
            }
        }
    }
}
