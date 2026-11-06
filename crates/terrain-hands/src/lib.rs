//! Hand tracking for terrain: where hands come from (`HandSource`) and what
//! they are doing (`HandState`). No Bevy dependency.

pub mod capture;
mod frame;
mod hand_tracker;
mod image;
mod landmarks;
mod live;
mod model;
mod palm;
mod region;
mod smoothing;
mod source;
mod state;

pub use frame::{Hand, HandFrame, Handedness, landmark};
pub use hand_tracker::{HandTracker, TrackedHand};
pub use image::RgbaImage;
pub use landmarks::{LANDMARK_MODEL, LandmarkModel};
pub use live::{LiveHands, LiveTracker, TrackedFrame};
pub use palm::{PALM_MODEL, PalmDetector};
pub use region::HandRegion;
pub use source::{HandSource, ReplaySource};
pub use state::{
    CalibrationError, CameraModel, FORGET_AFTER_MS, HandState, HandStateEstimator, TrackedHands,
};

use std::path::PathBuf;

/// Where terrain keeps what the user saves: `$XDG_CONFIG_HOME/terrain`, or
/// `~/.config/terrain`. An empty or relative `XDG_CONFIG_HOME` is ignored, as
/// the spec says.
pub fn config_dir() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".config")))?;
    Some(config.join("terrain"))
}
