//! The active tool, picked with keys `1`–`4`, and what `[` and `]` do with
//! it to the selection.

use bevy::prelude::*;

use crate::{Selection, Shape};

/// What edits on the selection do.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    /// Grab, move, turn and stretch, as ever.
    #[default]
    Move,
    /// Rounds a box toward a cylinder.
    Round,
    /// Narrows a shape's top.
    Taper,
    /// A pinch grabs a copy.
    Copy,
}

/// How much one `[` or `]` changes a tool's value.
const STEP: f32 = 0.1;

pub(crate) fn use_tools(
    keys: Res<ButtonInput<KeyCode>>,
    mut tool: ResMut<Tool>,
    selection: Res<Selection>,
    mut shapes: Query<&mut Shape>,
) {
    for (key, picked) in [
        (KeyCode::Digit1, Tool::Move),
        (KeyCode::Digit2, Tool::Round),
        (KeyCode::Digit3, Tool::Taper),
        (KeyCode::Digit4, Tool::Copy),
    ] {
        if keys.just_pressed(key) {
            *tool = picked;
        }
    }
    let step = match (
        keys.just_pressed(KeyCode::BracketLeft),
        keys.just_pressed(KeyCode::BracketRight),
    ) {
        (true, false) => -STEP,
        (false, true) => STEP,
        _ => return,
    };
    let Some(mut shape) = selection.0.and_then(|e| shapes.get_mut(e).ok()) else {
        return;
    };
    // Kept on whole steps, so ten presses get exactly from one end to the other.
    let nudge = |value: f32| ((value + step) / STEP).round() * STEP;
    match *tool {
        Tool::Round => shape.roundness = nudge(shape.roundness).clamp(0.0, 1.0),
        Tool::Taper => shape.taper = nudge(shape.taper).clamp(0.0, 1.0),
        Tool::Move | Tool::Copy => {}
    }
}
