//! Adding and removing shapes: `N` spawns a box at the cursor, `Delete`
//! removes the selection and `D` duplicates it. Placement snaps to the
//! Arena's grid; physics moves shapes freely after that.

use bevy::{ecs::entity_disabling::Disabled, prelude::*};

use crate::{Arena, Grabbable, Material, Shape, grab::Grab, screen::Aim};

/// The shape edits act on: the one under a cursor, else the one grabbed last.
#[derive(Resource, Default, Debug)]
pub struct Selection(pub Option<Entity>);

/// A shape added since the scene began. `R` removes it.
#[derive(Component)]
pub struct Placed;

/// The selection follows the shape under a cursor, nearest the camera
/// first, or the shape just grabbed, and stays put otherwise, so the
/// cursor can leave for the overlay.
pub(crate) fn select(
    mut selection: ResMut<Selection>,
    grab: Res<Grab>,
    shapes: Query<(Entity, &Transform, &Shape), With<Grabbable>>,
    aim: Aim,
    mut last_grab: Local<Option<Entity>>,
) {
    let hovered = aim.screen().and_then(|screen| {
        let cursors = aim.cursors();
        let camera = aim.camera_at()?;
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

/// `N`, `Delete` and `D`.
pub(crate) fn edit(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    arena: Res<Arena>,
    mut selection: ResMut<Selection>,
    mut grab: ResMut<Grab>,
    shapes: Query<(Entity, &Transform, &Shape, Has<Placed>, &Material), With<Grabbable>>,
    aim: Aim,
) {
    let taken_at: Vec<Vec3> = shapes.iter().map(|(_, t, ..)| t.translation).collect();
    let full = taken_at.len() >= arena.cap;
    if keys.just_pressed(KeyCode::KeyN) && !full {
        let at = arena.middle_of(arena.cell_of(aim.on_middle_plane()));
        if !arena.taken(at, &taken_at) {
            commands.spawn((Grabbable, Placed, Transform::from_translation(at)));
        }
    }
    let Some((entity, transform, shape, placed, material)) =
        selection.0.and_then(|e| shapes.get(e).ok())
    else {
        return;
    };
    if keys.just_pressed(KeyCode::KeyD) && !full {
        let n = arena.cells();
        let free = (0..n.x)
            .flat_map(|x| (0..n.y).flat_map(move |y| (0..n.z).map(move |z| IVec3::new(x, y, z))))
            .map(|c| arena.middle_of(c))
            .filter(|&at| !arena.taken(at, &taken_at))
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
