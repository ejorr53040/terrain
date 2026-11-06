//! Grabbing: a pinch takes the shape nearest its hand, moves and turns it
//! with the hand, and a second hand's pinch stretches it.

use avian3d::prelude::*;
use bevy::prelude::*;
use terrain_hands::{FORGET_AFTER_MS, HandState};

use crate::{
    Arena, Grabbable, Hands, Material, OverlayArea, Placed, Shape, Tool,
    screen::{SceneCamera, Screen},
};

/// Hand motion is amplified by this much when applied to a grabbed entity
/// in a scene with no camera. With one, the gain is set at the pinch so the
/// entity keeps pace with its hand's cursor on screen.
pub const GRAB_GAIN: f32 = 1.5;

/// A held entity closes the gap to where the hand wants it with this time
/// constant, in seconds: short enough to follow a quick move, long enough
/// not to jitter.
const STEER_SECONDS: f32 = 0.03;

/// The fastest a held entity is steered, in meters per second, so a jump in
/// tracking can't fling it or what it hits.
const STEER_MAX_SPEED: f32 = 4.0;

/// What each hand is holding: at most one entity per hand, one hand per entity.
#[derive(Resource, Default)]
pub(crate) struct Grab {
    pub(crate) held: Vec<Held>,
    /// The entity most recently picked up.
    pub(crate) last: Option<Entity>,
}

/// A grab in progress: what's held, by which hand, where the hand and the
/// entity were when the pinch began, and where the hand now wants it.
pub(crate) struct Held {
    pub(crate) entity: Entity,
    hand: HandState,
    transform: Transform,
    target: Transform,
    /// The entity's size when the pinch began, and the size hands now want.
    size: Vec3,
    target_size: Vec3,
    /// How far the entity moves per meter the hand moves, along each axis.
    gain: Vec3,
    /// Where the entity sits on screen from its hand's cursor, kept while
    /// one hand holds it; `None` without a scene camera.
    offset: Option<Vec2>,
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
pub(crate) struct Second {
    hand: HandState,
    /// The entity's axis being stretched: 0, 1 or 2 for its x, y or z.
    axis: usize,
    last_seen_ms: u64,
}

/// The shortest a stretch makes a side, in meters. The longest is set by the Arena.
const STRETCH_MIN: f32 = 0.03;

impl Held {
    /// Carries on from where the entity is now, measuring `hand`'s motion from here.
    fn reclutch(&mut self, hand: &HandState, screen: Option<&Screen>) {
        self.hand = *hand;
        self.transform = self.target;
        self.size = self.target_size;
        self.offset = screen_offset(screen, self.target.translation, hand);
    }
}

/// Where `at` sits on screen from `hand`'s cursor; `None` without a screen.
fn screen_offset(screen: Option<&Screen>, at: Vec3, hand: &HandState) -> Option<Vec2> {
    let screen = screen?;
    Some(screen.project(at)? - screen.pointing(hand.in_image))
}

/// A copy still being carried off its original.
#[derive(Component)]
pub(crate) struct Copying;

/// Makes a copy solid once it's let go.
pub(crate) fn settle_copies(
    mut commands: Commands,
    grab: Res<Grab>,
    copies: Query<Entity, With<Copying>>,
) {
    for entity in &copies {
        if grab.held.iter().all(|held| held.entity != entity) {
            commands.entity(entity).remove::<(Sensor, Copying)>();
        }
    }
}

/// Pushes each held entity toward its hand's target by setting its velocity,
/// so it shoves what's in the way and is stopped by what won't move. With no
/// gravity every other entity is stilled, so it stays where it was let go or
/// shoved to; with gravity they fall, bounce and slide freely.
pub(crate) fn steer(
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
pub(crate) fn drive_grab(
    mut commands: Commands,
    tool: Res<Tool>,
    made_of: Query<&Material>,
    mut hands: ResMut<Hands>,
    mut grab: ResMut<Grab>,
    mut grabbables: Query<(Entity, &Transform, &mut Shape), With<Grabbable>>,
    scene_camera: SceneCamera,
    arena: Res<Arena>,
    overlay: Res<OverlayArea>,
) {
    let Some(frame) = hands.fresh.take() else {
        return;
    };
    // Checked against each new frame, not only hand-less ones: the tracker
    // may go quiet and come back with the hand already there.
    let screen = Screen::of(&scene_camera);
    grab.held
        .retain(|held| frame.t_ms.saturating_sub(held.last_seen_ms) <= FORGET_AFTER_MS);
    grab.held.retain_mut(|held| {
        let first = frame.hands.iter().find(|h| h.id == held.hand.id);
        let Some(second) = &mut held.second else {
            return match first {
                Some(hand) if hand.pinching => {
                    held.last_seen_ms = frame.t_ms;
                    // ponytail: kept in by the shape's unturned size; a turned shape can
                    // still meet a wall early and slip off the cursor.
                    let half = held.target_size / 2.0;
                    let moved = (held.transform.translation
                        + held.gain * (hand.position - held.hand.position))
                        .clamp(arena.min + half, arena.max - half);
                    // Only the distance comes from the hand's motion; on screen it
                    // stays on the cursor, which a misjudged hand depth doesn't move.
                    held.target.translation = screen
                        .as_ref()
                        .zip(held.offset)
                        .and_then(|(s, offset)| {
                            s.on_sight_line(s.pointing(hand.in_image) + offset, moved)
                        })
                        .unwrap_or(moved);
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
                held.reclutch(&b, screen.as_ref());
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
                held.reclutch(&a, screen.as_ref());
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
    // A pinch over the overlay works the overlay, not the scene.
    for hand in frame
        .hands
        .iter()
        .filter(|h| h.pinch_started && !overlay.covers(h.in_image))
    {
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
            held.reclutch(first, screen.as_ref());
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
                    .spawn((
                        Grabbable, Placed, *shape, material, *transform, Sensor, Copying,
                    ))
                    .id();
            }
            // The view each camera has, at the hand's and the entity's depth,
            // so a move fills the same share of the screen as the cursor's.
            let gain = match scene_camera.single() {
                Ok((camera, Projection::Perspective(p))) => {
                    let depth = (transform.translation - camera.translation).dot(*camera.forward());
                    let scene_height = 2.0 * depth * (p.fov / 2.0).tan();
                    let webcam =
                        hands.estimator.camera().view_size_at_1m() * hand.position.z.max(0.1);
                    let across = scene_height * p.aspect_ratio / webcam.x;
                    let up = scene_height / webcam.y;
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
                offset: screen_offset(screen.as_ref(), transform.translation, hand),
                last_seen_ms: frame.t_ms,
                second: None,
            });
        }
    }
}
