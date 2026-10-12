//! The gesture file: gestures as small state machines, one per starting
//! Pose (ADR 0001). Guards can only test the Pose and how long it has
//! read, time in the state, and distance moved; a state names the buttons
//! it holds, and leaving it any way releases them.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{Action, Button, Pose};

/// The gestures compiled in: today's desktop control.
pub const DEFAULT: &str = include_str!("default.ron");

#[derive(Clone, Debug, Deserialize)]
pub struct GestureFile {
    pub gestures: Vec<Gesture>,
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
    /// Ways out, the first that's enabled taken. A state with none ends
    /// the gesture as soon as it's entered.
    pub on: Vec<Transition>,
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
    /// The palm has come more than this much nearer the camera (m) than
    /// where the gesture started.
    Pushed(f32),
}
