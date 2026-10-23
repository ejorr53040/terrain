//! The gesture file: gestures as small state machines, one per starting
//! Pose (ADR 0001). Guards can only test the Pose and how long it has
//! read, time in the state, and distance moved; a state names the buttons
//! it holds, and leaving it any way releases them.

use std::collections::BTreeMap;

use glam::Vec2;
use serde::Deserialize;

use crate::{Action, Button, Pose};

/// The gestures compiled in: today's desktop control.
pub const DEFAULT: &str = include_str!("default.ron");

#[derive(Clone, Debug, Deserialize)]
pub struct GestureFile {
    pub gestures: Vec<Gesture>,
    #[serde(default)]
    pub spreads: Vec<Spread>,
}

/// A Spread: two hands brought together, then spread apart quickly.
/// Neither hand needs control, but neither can be in a gesture (ADR 0008).
#[derive(Clone, Debug, Deserialize)]
pub struct Spread {
    /// The Poses each hand may make throughout.
    pub poses: Vec<Pose>,
    /// The palms closer than this (meters)...
    pub together: f32,
    /// ...for at least this long (ms)...
    pub held_ms: u64,
    /// ...then further apart than this (meters)...
    pub apart: f32,
    /// ...within this long (ms) of last being together...
    pub within_ms: u64,
    /// ...and within this long of it with both hands seen: a slow drift
    /// apart isn't a Spread, but a hand flung out of view for a moment is.
    pub parting_ms: u64,
    /// Both hands in `poses` with palms closer than this (meters) are a
    /// Spread getting under way: the hand with control can't start any
    /// gesture, since hands read as pinching as they come together.
    pub near: f32,
    /// The key chord it sends: pressed in order, released in reverse.
    #[serde(deserialize_with = "keys_only")]
    pub keys: Vec<Button>,
}

/// A Spread's chord can't hold a mouse button: without Engagement, hands at
/// the keyboard must never be able to click (ADR 0008).
fn keys_only<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Button>, D::Error> {
    let keys = Vec::<Button>::deserialize(d)?;
    match keys
        .iter()
        .find(|b| matches!(b, Button::Left | Button::Right))
    {
        Some(b) => Err(serde::de::Error::custom(format!(
            "a Spread sends keys only, not the mouse button {b:?}"
        ))),
        None => Ok(keys),
    }
}

impl GestureFile {
    pub fn parse(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    /// The compiled-in gestures.
    pub fn default_gestures() -> Self {
        Self::parse(DEFAULT).expect("the built-in gesture file parses")
    }
}

/// One gesture: what starting it takes, and its states.
#[derive(Clone, Debug, Deserialize)]
pub struct Gesture {
    /// The Pose that starts it...
    pub pose: Pose,
    /// ...once read steadily this long (ms), in the Reach.
    pub settle_ms: u64,
    /// The state it starts in.
    pub start: String,
    pub states: BTreeMap<String, State>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct State {
    /// What the on-screen indicator says while in this state.
    pub label: String,
    pub pointer: Pointer,
    /// Buttons held down while in this state, in the order they're pressed.
    pub holds: Vec<Button>,
    /// Actions on entering, after any holds are pressed.
    pub enter: Vec<Action>,
    /// Scrolling every frame while in this state.
    pub scroll: Option<Scroll>,
    /// Ways out, the first that's enabled taken. A state with none ends
    /// the gesture as soon as it's entered.
    pub on: Vec<Transition>,
}

/// Rate scrolling: the further the hand is from where the gesture started,
/// the faster the page scrolls that way.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Scroll {
    /// How far the hand can stray on each axis (screen widths) without
    /// scrolling.
    pub dead_zone: f32,
    /// Wheel steps a second for each screen width past the dead zone.
    pub rate: f32,
}

impl Scroll {
    /// The wheel steps for `dt_ms` with the hand `offset` from where the
    /// gesture started: y down the page, x to the right.
    pub fn by(&self, offset: Vec2, dt_ms: u64) -> Vec2 {
        let past = (offset.abs() - self.dead_zone).max(Vec2::ZERO) * offset.signum();
        past * self.rate * dt_ms as f32 / 1000.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum Pointer {
    /// The pointer follows the hand.
    #[default]
    Follows,
    /// The pointer stays where the gesture started, so a click lands where
    /// it was aimed.
    Frozen,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Transition {
    /// All of these must hold.
    pub when: Vec<Guard>,
    pub to: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub enum Guard {
    /// The hand has read this Pose for at least this long (ms).
    Pose(Pose, u64),
    /// At least this long (ms) in the state.
    After(u64),
    /// The pointer has moved more than this far (screen widths) from where
    /// the gesture started.
    Moved(f32),
    /// The pointer has been more than this far (screen widths) from where
    /// the gesture started for at least this long (ms): long enough that
    /// a flick would already have fired.
    MovedFor(f32, u64),
    /// The pointer covered more than this far (screen widths) in this
    /// direction, more along it than across, within the last this long
    /// (ms), however long the hand was held first: a flick, not a drag.
    Flicked(Direction, f32, u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_keys(keys: &str) -> Result<GestureFile, ron::error::SpannedError> {
        GestureFile::parse(&format!(
            "(gestures: [], spreads: [(poses: [Open], together: 0.2, held_ms: 150, \
             near: 0.35, apart: 0.32, within_ms: 450, parting_ms: 250, keys: {keys})])"
        ))
    }

    #[test]
    fn a_spread_can_send_keys_but_never_click() {
        assert!(with_keys("[Super, F]").is_ok());
        let err = with_keys("[Left]").unwrap_err().to_string();
        assert!(err.contains("mouse button Left"), "{err}");
    }
}
