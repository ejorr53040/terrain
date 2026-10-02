//! `C` calibrates the camera: after a countdown, the hand held upright 50 cm
//! from the lens sets the field of view and the camera's tilt.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use bevy::prelude::*;
use terrain_hands::{CameraModel, Hand};

use crate::Hands;

/// How far from the lens the hand is held while calibrating.
pub const CALIBRATION_DISTANCE_M: f32 = 0.5;
/// Time to get the hand into place after pressing `C`...
const COUNTDOWN: Duration = Duration::from_secs(3);
/// ...then how long it's measured for.
const MEASURE: Duration = Duration::from_secs(1);

/// Where calibration stands, for the overlay.
#[derive(Resource, Default)]
pub struct Calibration {
    phase: Phase,
    /// The last calibration's outcome, shown once it's done.
    outcome: Option<String>,
    /// Where a successful calibration is saved; `None` keeps it in memory.
    save_to: Option<PathBuf>,
    /// A calibration just succeeded: the scene starts over, since hands now
    /// map into it differently.
    pub(crate) reset_scene: bool,
}

#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    /// Waiting for the hand, until this elapsed time.
    Countdown(Duration),
    /// Measuring until this elapsed time.
    Measuring(Duration, Vec<Hand>),
}

impl Calibration {
    /// Calibration that saves to `save_to`, starting from `camera`; the
    /// overlay notes when that's a saved calibration rather than the default.
    pub fn new(camera: CameraModel, save_to: Option<PathBuf>) -> Self {
        Self {
            outcome: (camera != CameraModel::default())
                .then(|| format!("using saved calibration: {}", describe(&camera))),
            save_to,
            ..default()
        }
    }

    /// One line for the overlay, or `None` when there's nothing to say.
    pub fn status(&self, now: Duration) -> Option<String> {
        match &self.phase {
            Phase::Idle => self.outcome.clone(),
            Phase::Countdown(until) => Some(format!(
                "calibrating in {:.0} s: one hand {:.0} cm from the camera, in the middle of the preview,\nfingers straight up, palm to the camera",
                until.saturating_sub(now).as_secs_f32().ceil(),
                CALIBRATION_DISTANCE_M * 100.0
            )),
            Phase::Measuring(..) => Some("calibrating: hold still".into()),
        }
    }
}

/// Starts, runs and finishes calibration. Runs after hands are read, so it
/// sees this update's frame.
pub(crate) fn calibrate(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut calibration: ResMut<Calibration>,
    mut hands: ResMut<Hands>,
) {
    let now = time.elapsed();
    if keys.just_pressed(KeyCode::KeyC) {
        calibration.phase = Phase::Countdown(now + COUNTDOWN);
    }
    match &mut calibration.phase {
        Phase::Idle => {}
        Phase::Countdown(until) => {
            if now >= *until {
                calibration.phase = Phase::Measuring(now + MEASURE, Vec::new());
            }
        }
        Phase::Measuring(until, seen) => {
            if let Some(hand) = hands.raw.as_ref().and_then(|f| middle_hand(&f.hands)) {
                seen.push(hand.clone());
            }
            if now >= *until {
                let seen = std::mem::take(seen);
                calibration.phase = Phase::Idle;
                let fitted = hands
                    .estimator
                    .camera()
                    .calibrated(&seen, CALIBRATION_DISTANCE_M);
                calibration.outcome = Some(match fitted {
                    Ok(camera) => {
                        hands.estimator.set_camera(camera);
                        calibration.reset_scene = true;
                        let saved = calibration.save_to.as_ref().map(|path| save(path, &camera));
                        let mut outcome = format!("calibrated: {}", describe(&camera));
                        if let Some(Err(e)) = saved {
                            outcome += &format!(" (not saved: {e})");
                        }
                        outcome
                    }
                    Err(why) => format!(
                        "calibration failed: {why} (hold it {:.0} cm from the camera)",
                        CALIBRATION_DISTANCE_M * 100.0
                    ),
                });
            }
        }
    }
}

/// A camera in a few words, for the overlay.
fn describe(camera: &CameraModel) -> String {
    format!(
        "{:.0} deg FOV, camera tilted {:.0} deg",
        camera.hfov_deg,
        camera.tilt.angle_between(Quat::IDENTITY).to_degrees()
    )
}

/// The hand whose palm is nearest the middle of the image: the one held up
/// to calibrate, rather than one resting at the user's side.
fn middle_hand(hands: &[Hand]) -> Option<&Hand> {
    let off_center = |hand: &Hand| hand.palm_in_image().distance(Vec2::splat(0.5));
    hands
        .iter()
        .min_by(|a, b| off_center(a).total_cmp(&off_center(b)))
}

fn save(path: &Path, camera: &CameraModel) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(camera)?)
}

/// The camera saved at `path`, if there's a readable, plausible one. A
/// missing file is normal; an unusable one is logged and ignored.
pub fn load_camera(path: &Path) -> Option<CameraModel> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<CameraModel>(&text) {
        Ok(camera) if camera.plausible() => Some(camera),
        Ok(_) => {
            warn!("ignoring implausible calibration in {}", path.display());
            None
        }
        Err(e) => {
            warn!("ignoring unreadable calibration in {}: {e}", path.display());
            None
        }
    }
}
