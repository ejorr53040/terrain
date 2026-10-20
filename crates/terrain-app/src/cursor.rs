//! Draws each `HandCursor` as a ring over the window, filled while its hand
//! pinches, so you can see where your hands are even with a cube in them.

use bevy::prelude::*;

use crate::HandCursor;

/// How wide the ring is, in pixels.
const RING_SIZE: f32 = 44.0;
const RING_THICKNESS: f32 = 4.0;
const OPEN_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.9);
const PINCH_COLOR: Color = Color::srgba(1.0, 0.55, 0.15, 0.9);
/// Shows through the ring while it's pinching.
const PINCH_FILL: Color = Color::srgba(1.0, 0.55, 0.15, 0.35);

/// Shows every `HandCursor` as a ring in the window.
pub struct CursorPlugin;

impl Plugin for CursorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dress_new_cursors, place_cursors).chain());
    }
}

/// Gives each new cursor its ring.
fn dress_new_cursors(mut commands: Commands, new: Query<Entity, Added<HandCursor>>) {
    for entity in &new {
        commands.entity(entity).insert((
            Node {
                position_type: PositionType::Absolute,
                width: px(RING_SIZE),
                height: px(RING_SIZE),
                border: UiRect::all(px(RING_THICKNESS)),
                border_radius: BorderRadius::MAX,
                // Pulls the ring's middle, not its corner, onto `left`/`top`.
                margin: UiRect::all(px(-RING_SIZE / 2.0)),
                ..default()
            },
            BorderColor::all(OPEN_COLOR),
            BackgroundColor(Color::NONE),
            GlobalZIndex(10),
        ));
    }
}

/// Puts each ring on its hand and fills it while pinching.
fn place_cursors(
    mut cursors: Query<(
        &HandCursor,
        &mut Node,
        &mut BorderColor,
        &mut BackgroundColor,
    )>,
) {
    for (cursor, mut node, mut border, mut fill) in &mut cursors {
        node.left = percent(cursor.at.x * 100.0);
        node.top = percent(cursor.at.y * 100.0);
        *border = BorderColor::all(if cursor.pinching {
            PINCH_COLOR
        } else {
            OPEN_COLOR
        });
        fill.0 = if cursor.pinching {
            PINCH_FILL
        } else {
            Color::NONE
        };
    }
}
