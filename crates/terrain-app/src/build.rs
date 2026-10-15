//! Adding and removing shapes: `N` spawns a box at the cursor, `Delete`
//! removes the selection and `D` duplicates it. Placement snaps to the
//! Arena's grid; physics moves shapes freely after that.

use bevy::{ecs::entity_disabling::Disabled, prelude::*, window::PrimaryWindow};

use crate::{Arena, Grab, Grabbable, HandCursor, Material, SceneCamera, Screen, Shape};

/// The shape edits act on: the one under a cursor, else the one grabbed last.
#[derive(Resource, Default, Debug)]
pub struct Selection(pub Option<Entity>);

/// A shape added since the scene began. `R` removes it.
#[derive(Component)]
pub struct Placed;

/// Where the cursors are in the view (0 to 1 across and down from the top
/// left): each hand's, then the mouse's.
pub(crate) fn cursors(hands: &Query<&HandCursor>, window: &Query<&Window, With<PrimaryWindow>>) -> Vec<Vec2> {
    let mouse = window
        .single()
        .ok()
        .and_then(|w| Some(w.cursor_position()? / w.size()));
    hands.iter().map(|c| c.at).chain(mouse).collect()
}

/// The selection follows the shape under a cursor, nearest the camera
/// first, and otherwise stays on the shape grabbed last.
pub(crate) fn select(
    mut selection: ResMut<Selection>,
    grab: Res<Grab>,
    hands: Query<&HandCursor>,
    window: Query<&Window, With<PrimaryWindow>>,
    shapes: Query<(Entity, &Transform, &Shape), With<Grabbable>>,
    scene_camera: SceneCamera,
) {
    let hovered = Screen::of(&scene_camera).and_then(|screen| {
        let cursors = cursors(&hands, &window);
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
    selection.0 = hovered.or(grab.last.filter(|&e| shapes.contains(e)));
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
    shapes: Query<
        (
            Entity,
            &Transform,
            &Shape,
            Has<Placed>,
            &Material,
        ),
        With<Grabbable>,
    >,
    scene_camera: SceneCamera,
) {
    let taken_at: Vec<Vec3> = shapes.iter().map(|(_, t, ..)| t.translation).collect();
    let full = taken_at.len() >= arena.cap;
    if keys.just_pressed(KeyCode::KeyN) && !full {
        // The first cursor, else the middle of the view.
        let cursor = cursors(&hands, &window)
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
