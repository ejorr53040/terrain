//! What the user asks the scene for. A key and the overlay button beside it
//! both set the same request, taken (and cleared) by what carries it out, so
//! they always do the same thing.

use bevy::prelude::*;

#[derive(Resource, Default, Debug)]
pub struct Requests {
    /// Save the selection's build as a template.
    pub save: bool,
    /// Stamp the current template at the cursor.
    pub stamp: bool,
    /// Put the scene back as it began.
    pub reset: bool,
    pub calibrate: bool,
    /// Show or hide the webcam preview.
    pub preview: bool,
}

pub(crate) fn press_keys(keys: Res<ButtonInput<KeyCode>>, mut requests: ResMut<Requests>) {
    let r = &mut *requests;
    for (key, request) in [
        (KeyCode::KeyT, &mut r.save),
        (KeyCode::KeyP, &mut r.stamp),
        (KeyCode::KeyR, &mut r.reset),
        (KeyCode::KeyC, &mut r.calibrate),
        (KeyCode::KeyV, &mut r.preview),
    ] {
        *request |= keys.just_pressed(key);
    }
}
