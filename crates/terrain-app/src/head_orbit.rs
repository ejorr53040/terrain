//! it on and off, `Shift+H` Recenters.

use std::sync::Mutex;

use bevy::prelude::*;
use terrain_head::{HeadLook, HeadLookSettings, HeadTracker, OffReason};

use crate::grab::Grab;
use crate::{Grabbable, Requests, SCENE_CAMERA_AT, requests};

/// How Head look moves the camera, as the overlay's Head look section sets it.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct HeadOrbitSettings {
    /// Degrees of orbit round the vertical per degree the head turns.
    pub yaw_gain: f32,
    /// Degrees the view rises per degree the head nods down.
    pub pitch_gain: f32,
    /// Head angles closer than this to Straight ahead read as 0, in degrees.
    pub dead_zone_deg: f32,
}

impl Default for HeadOrbitSettings {
    /// 15° of head turn is a quarter turn of orbit.
    fn default() -> Self {
        Self {
            yaw_gain: 6.0,
            pitch_gain: 3.0,
            dead_zone_deg: HeadLookSettings::default().dead_zone_deg,
        }
    }
}

/// How quickly the camera closes on where Head look wants it, per second:
/// it covers about 1 − e⁻⁸ of the way in a second.
const GLIDE_PER_S: f32 = 8.0;
/// The view is kept between level and just short of straight down, in degrees.
const LOWEST: f32 = 0.0;
const HIGHEST: f32 = 89.0;

/// Where Head look comes from: the AirPods through `HeadTracker`, or a test's stand-in.
pub trait HeadSource: Send + 'static {
    /// Switches Head look on when Off, off otherwise.
    fn toggle(&mut self);
    /// Makes the head's current orientation Straight ahead; nothing unless On.
    fn recenter(&mut self);
    fn set_settings(&mut self, settings: HeadLookSettings);
    /// Head look now; never blocks.
    fn look(&mut self) -> HeadLook;
}

impl HeadSource for HeadTracker {
    fn toggle(&mut self) {
        HeadTracker::toggle(self);
    }

    fn recenter(&mut self) {
        HeadTracker::recenter(self);
    }

    fn set_settings(&mut self, settings: HeadLookSettings) {
        HeadTracker::set_settings(self, settings);
    }

    fn look(&mut self) -> HeadLook {
        HeadTracker::look(self)
    }
}

/// Orbits the scene camera from a `HeadSource`. Add after `GrabPlugin`.
pub struct HeadOrbitPlugin {
    source: Mutex<Option<Box<dyn HeadSource>>>,
}

impl HeadOrbitPlugin {
    pub fn new(source: impl HeadSource) -> Self {
        Self {
            source: Mutex::new(Some(Box::new(source))),
        }
    }
}

impl Plugin for HeadOrbitPlugin {
    fn build(&self, app: &mut App) {
        let source = self
            .source
            .lock()
            .unwrap()
            .take()
            .expect("HeadOrbitPlugin added twice");
        let (azimuth, elevation) = default_view();
        app.insert_resource(Head(Mutex::new(source)))
            .insert_resource(HeadOrbit {
                look: HeadLook::Off(OffReason::SwitchedOff),
                azimuth,
                elevation,
            })
            .init_resource::<HeadOrbitSettings>()
            .add_systems(Update, orbit.after(requests::press_keys));
    }
}

#[derive(Resource)]
struct Head(Mutex<Box<dyn HeadSource>>);

/// Head look as last read, and the orbit it drives.
#[derive(Resource, Debug)]
pub struct HeadOrbit {
    pub look: HeadLook,
    /// Where the camera is round the scene's middle, in radians: about the
    /// vertical from straight in front, and up from level.
    azimuth: f32,
    elevation: f32,
}

impl HeadOrbit {
    /// One line saying what Head look is doing and which key changes that.
    pub fn status(&self) -> String {
        match self.look {
            HeadLook::Off(OffReason::SwitchedOff) => "Head look off  (H on)".into(),
            HeadLook::Off(OffReason::OffHead) => {
                "Head look off: AirPods aren't on your head  (H on)".into()
            }
            HeadLook::Off(OffReason::Disconnected) => {
                "Head look off: AirPods disconnected  (H on)".into()
            }
            HeadLook::Off(OffReason::Unavailable) => "Head look unavailable: no motion from the \
                AirPods (connected? other Apple devices using them? reconnect)  (H retry)"
                .into(),
            HeadLook::Off(OffReason::NoDaemon) => {
                "Head look unavailable: the airpods-tui daemon isn't running  (H retry)".into()
            }
            HeadLook::Starting => "Head look starting: waiting for AirPods motion  (H off)".into(),
            HeadLook::On { yaw_deg, pitch_deg } => format!(
                "Head look on  yaw {yaw_deg:+.0}°  pitch {pitch_deg:+.0}°  (H off, Shift+H recenter)"
            ),
        }
    }
}

/// The default camera's azimuth and elevation round the scene's middle.
fn default_view() -> (f32, f32) {
    let at = SCENE_CAMERA_AT;
    (at.x.atan2(at.z), (at.y / at.length()).asin())
}

fn orbit(
    time: Res<Time>,
    settings: Res<HeadOrbitSettings>,
    mut source: ResMut<Head>,
    mut requests: ResMut<Requests>,
    grab: Res<Grab>,
    mut head: ResMut<HeadOrbit>,
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<Grabbable>)>,
) {
    let source = source.0.get_mut().unwrap();
    if settings.is_changed() {
        source.set_settings(HeadLookSettings {
            dead_zone_deg: settings.dead_zone_deg,
        });
    }
    if std::mem::take(&mut requests.head_look) {
        source.toggle();
    }
    if std::mem::take(&mut requests.recenter) {
        source.recenter();
    }
    head.look = source.look();
    let (azimuth, elevation) = default_view();
    let (azimuth, elevation) = match head.look {
        // Head turned left swings the camera round to the scene's left.
        HeadLook::On { yaw_deg, pitch_deg } => (
            azimuth - (yaw_deg * settings.yaw_gain).to_radians(),
            (elevation + (pitch_deg * settings.pitch_gain).to_radians())
                .clamp(LOWEST.to_radians(), HIGHEST.to_radians()),
        ),
        HeadLook::Starting | HeadLook::Off(_) => (azimuth, elevation),
    };
    // While a hand holds a shape the camera stays put; on release it glides on
    // from there to wherever the head points now.
    if grab.held.is_empty() {
        let k = 1.0 - (-GLIDE_PER_S * time.delta_secs()).exp();
        head.azimuth += (azimuth - head.azimuth) * k;
        head.elevation += (elevation - head.elevation) * k;
    }
    let (az, el) = (head.azimuth, head.elevation);
    let at =
        SCENE_CAMERA_AT.length() * Vec3::new(az.sin() * el.cos(), el.sin(), az.cos() * el.cos());
    for mut transform in &mut camera {
        *transform = Transform::from_translation(at).looking_at(Vec3::ZERO, Vec3::Y);
    }
}
