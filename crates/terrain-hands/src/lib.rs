//! Hand tracking for terrain: where hands come from (`HandSource`) and what
//! they are doing (`HandState`). No Bevy dependency.

pub mod capture;
mod frame;
mod palm;
mod smoothing;
mod source;
mod state;
mod tracker;

pub use frame::{Hand, HandFrame, Handedness, landmark};
pub use palm::{HandRegion, PALM_MODEL, PalmDetector, RgbaImage};
pub use source::{HandSource, ReplaySource};
pub use state::{CameraModel, FORGET_AFTER_MS, HandState, HandStateEstimator, TrackedHands};
pub use tracker::{HandTracker, TrackedFrame};
