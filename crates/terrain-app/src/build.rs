//! Adding and removing shapes: `N` spawns a box at the cursor, `Delete`
//! removes the selection and `D` duplicates it. Placement snaps to the
//! Arena's grid; physics moves shapes freely after that.

use std::time::Duration;

use bevy::{ecs::entity_disabling::Disabled, prelude::*, window::PrimaryWindow};

use crate::{Arena, Grab, Grabbable, HandCursor, Material, SceneCamera, Screen, Shape};

/// The shape edits act on: the one under a cursor, else the one grabbed last.
#[derive(Resource, Default, Debug)]
pub struct Selection(pub Option<Entity>);

/// A shape added since the scene began. `R` removes it.
#[derive(Component)]
pub struct Placed;

/// Where the overlay is drawn, as fractions of the window from the top
/// left; a cursor in there isn't pointing into the scene. The overlay sets it.
#[derive(Resource, Default, Debug)]
pub struct OverlayArea(pub Option<Rect>);

/// Whether the overlay is shown. `O` toggles it; a hand resting on the
/// Hot edge opens it, and then it closes once no hand has been over it for
/// `HOT_EDGE_CLOSE`.
#[derive(Resource, Default, Debug)]
pub struct OverlayOpen {
    pub open: bool,
    /// Opened by the Hot edge, so it closes itself.
    by_edge: bool,
}

impl OverlayOpen {
    /// What `O` does: show or hide it, and leave it that way.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.by_edge = false;
    }
}

/// How far across the view the Hot edge starts.
const HOT_EDGE: f32 = 0.97;
/// How long a hand rests on the Hot edge to open the overlay.
const HOT_EDGE_OPEN: Duration = Duration::from_millis(300);
/// How long the overlay stays open with no hand over it.
const HOT_EDGE_CLOSE: Duration = Duration::from_secs(1);

/// Opens the overlay for a hand resting on the Hot edge and closes it
/// once no hand has been over it for a while.
pub(crate) fn hot_edge(
    time: Res<Time>,
    hands: Query<&HandCursor>,
    area: Res<OverlayArea>,
    mut overlay: ResMut<OverlayOpen>,
    mut on_edge_since: Local<Option<Duration>>,
    mut last_over: Local<Duration>,
) {
    let now = time.elapsed();
    let on_edge = hands.iter().any(|c| c.at.x >= HOT_EDGE);
    if !on_edge {
        *on_edge_since = None;
    } else if !overlay.open && now - *on_edge_since.get_or_insert(now) >= HOT_EDGE_OPEN {
        overlay.open = true;
        overlay.by_edge = true;
    }
    if on_edge || hands.iter().any(|c| area.covers(c.at)) {
        *last_over = now;
    }
    if overlay.open && overlay.by_edge && now - *last_over >= HOT_EDGE_CLOSE {
        overlay.open = false;
        overlay.by_edge = false;
    }
}

/// A hand cursor working the overlay like a mouse: where it points (fractions
/// of the window) and whether it's pinched. The overlay feeds it to egui.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct OverlayPointer {
    pub at: Option<Vec2>,
    pub pressed: bool,
    /// The hand that owns it: the first one over the overlay.
    owner: Option<u32>,
    /// Where the pointer and the hand were at the pinch.
    pressed_from: Option<(Vec2, Vec2)>,
}

/// How far the pointer moves for the hand's move while pinched, so a
/// slider doesn't wobble with the hand.
const DRAG_GAIN: f32 = 0.5;

/// Gives the overlay pointer to the first open hand over the overlay and
/// moves it with that hand until the hand leaves (not mid-press).
pub(crate) fn point_at_overlay(
    hands: Query<&HandCursor>,
    area: Res<OverlayArea>,
    mut pointer: ResMut<OverlayPointer>,
) {
    let owner = pointer
        .owner
        .and_then(|id| hands.iter().find(|c| c.id == id));
    let keeps = owner.is_some_and(|c| area.covers(c.at) || pointer.pressed && c.pinching);
    let owner = if keeps {
        owner
    } else {
        // An open hand takes it: one already pinching is carrying something in.
        let mut over: Vec<&HandCursor> = hands
            .iter()
            .filter(|c| !c.pinching && area.covers(c.at))
            .collect();
        over.sort_by_key(|c| c.id);
        over.first().copied()
    };
    let mut next = OverlayPointer::default();
    if let Some(c) = owner {
        next.owner = Some(c.id);
        next.pressed = c.pinching;
        next.pressed_from = match (c.pinching, pointer.pressed_from) {
            (true, Some(from)) if pointer.owner == Some(c.id) => Some(from),
            (true, _) => Some((c.at, c.at)),
            (false, _) => None,
        };
        next.at = Some(match next.pressed_from {
            Some((pointer_at, hand_at)) => pointer_at + DRAG_GAIN * (c.at - hand_at),
            None => c.at,
        });
    }
    if *pointer != next {
        *pointer = next;
    }
}

impl OverlayArea {
    /// Whether a cursor `at` (fractions of the window) is over the overlay.
    pub fn covers(&self, at: Vec2) -> bool {
        self.0.is_some_and(|area| area.contains(at))
    }
}

/// Where the cursors are in the view (0 to 1 across and down from the top
/// left): each hand's, then the mouse's unless it's over the overlay.
pub(crate) fn cursors(
    hands: &Query<&HandCursor>,
    window: &Query<&Window, With<PrimaryWindow>>,
    overlay: &OverlayArea,
) -> Vec<Vec2> {
    let mouse = window
        .single()
        .ok()
        .and_then(|w| Some(w.cursor_position()? / w.size()));
    hands
        .iter()
        .map(|c| c.at)
        .chain(mouse)
        .filter(|&at| !overlay.covers(at))
        .collect()
}

/// The selection follows the shape under a cursor, nearest the camera
/// first, or the shape just grabbed, and stays put otherwise, so the
/// cursor can leave for the overlay.
pub(crate) fn select(
    mut selection: ResMut<Selection>,
    grab: Res<Grab>,
    hands: Query<&HandCursor>,
    window: Query<&Window, With<PrimaryWindow>>,
    over_ui: Res<OverlayArea>,
    shapes: Query<(Entity, &Transform, &Shape), With<Grabbable>>,
    scene_camera: SceneCamera,
    mut last_grab: Local<Option<Entity>>,
) {
    let hovered = Screen::of(&scene_camera).and_then(|screen| {
        let cursors = cursors(&hands, &window, &over_ui);
        let camera = scene_camera.single().ok()?.0.translation;
        shapes
            .iter()
            .filter(|(_, t, shape)| cursors.iter().any(|&c| screen.covers(t, shape, c)))
            .min_by(|(_, a, _), (_, b, _)| {
                a.translation
                    .distance(camera)
                    .total_cmp(&b.translation.distance(camera))
            })
            .map(|(entity, ..)| entity)
    });
    // A new grab selects what it took; otherwise the selection stays where
    // it was until the cursor is on another shape.
    let grabbed = grab.last.filter(|_| grab.last != *last_grab);
    *last_grab = grab.last;
    let kept = selection.0.filter(|&e| shapes.contains(e));
    selection.0 = hovered.or(grabbed).or(kept);
}

/// The grid cell `at` is in, kept inside the Arena.
pub(crate) fn cell_of(arena: &Arena, at: Vec3) -> IVec3 {
    ((at - arena.min) / arena.cell)
        .floor()
        .as_ivec3()
        .clamp(IVec3::ZERO, cells(arena) - IVec3::ONE)
}

/// How many whole cells fit along each side.
pub(crate) fn cells(arena: &Arena) -> IVec3 {
    ((arena.max - arena.min) / arena.cell)
        .floor()
        .as_ivec3()
        .max(IVec3::ONE)
}

pub(crate) fn middle_of(arena: &Arena, cell: IVec3) -> Vec3 {
    arena.min + (cell.as_vec3() + 0.5) * arena.cell
}

/// Whether a shape placed at `at` would start inside one already there.
// ponytail: treats every shape as one cell big; a stretched shape can still be overlapped.
pub(crate) fn taken(arena: &Arena, at: Vec3, shapes: &[Vec3]) -> bool {
    shapes
        .iter()
        .any(|s| (*s - at).abs().max_element() < arena.cell - 1e-3)
}

/// `N`, `Delete` and `D`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn edit(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    arena: Res<Arena>,
    mut selection: ResMut<Selection>,
    mut grab: ResMut<Grab>,
    hands: Query<&HandCursor>,
    window: Query<&Window, With<PrimaryWindow>>,
    over_ui: Res<OverlayArea>,
    shapes: Query<(Entity, &Transform, &Shape, Has<Placed>, &Material), With<Grabbable>>,
    scene_camera: SceneCamera,
) {
    let taken_at: Vec<Vec3> = shapes.iter().map(|(_, t, ..)| t.translation).collect();
    let full = taken_at.len() >= arena.cap;
    if keys.just_pressed(KeyCode::KeyN) && !full {
        // The first cursor, else the middle of the view.
        let cursor = cursors(&hands, &window, &over_ui)
            .first()
            .copied()
            .unwrap_or(Vec2::splat(0.5));
        let at = Screen::of(&scene_camera)
            .and_then(|s| s.on_middle_plane(cursor))
            .unwrap_or(Vec3::ZERO);
        let at = middle_of(&arena, cell_of(&arena, at));
        if !taken(&arena, at, &taken_at) {
            commands.spawn((Grabbable, Placed, Transform::from_translation(at)));
        }
    }
    let Some((entity, transform, shape, placed, material)) =
        selection.0.and_then(|e| shapes.get(e).ok())
    else {
        return;
    };
    if keys.just_pressed(KeyCode::KeyD) && !full {
        let n = cells(&arena);
        let free = (0..n.x)
            .flat_map(|x| (0..n.y).flat_map(move |y| (0..n.z).map(move |z| IVec3::new(x, y, z))))
            .map(|c| middle_of(&arena, c))
            .filter(|&at| !taken(&arena, at, &taken_at))
            .min_by(|a, b| {
                a.distance(transform.translation)
                    .total_cmp(&b.distance(transform.translation))
            });
        if let Some(at) = free {
            commands.spawn((
                Grabbable,
                Placed,
                *shape,
                *material,
                Transform {
                    translation: at,
                    ..*transform
                },
            ));
        }
    }
    if keys.just_pressed(KeyCode::Delete) {
        grab.held.retain(|held| held.entity != entity);
        if grab.last == Some(entity) {
            grab.last = None;
        }
        selection.0 = None;
        // Shapes the scene began with are only put away, for `R` to bring back.
        if placed {
            commands.entity(entity).despawn();
        } else {
            commands.entity(entity).insert(Disabled);
        }
    }
}
