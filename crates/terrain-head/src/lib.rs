//! Head look for terrain: AirPods head motion turned into yaw and pitch.
//!
//! The airpods-tui daemon owns the AirPods connection and streams head motion to the IPC
//! clients that ask for it (ADR 0009); [`HeadTracker`] is such a client, shared by the cube
//! world and gridboard (ADR 0010). No Bevy, egui or GTK dependency.
//!
//! The motion protocol research behind it (AACP RemoteHID, the `devmotion6` sensor, the junk
//! first sample) is MousePods' by Matteo Gaudioso (starman-tech),
//! <https://github.com/starman-tech/mousepods>, findings CC BY 4.0.

mod look;
mod tracker;

pub use tracker::HeadTracker;

/// Head look as one value: what the consumer draws from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeadLook {
    /// Not switched on, or switched off.
    Off(OffReason),
    /// Switched on; waiting for the first usable samples (straight ahead not taken yet).
    /// Lasts about 0.5 s once samples come; if none comes within 8 s of switching on, it turns
    /// Off(Unavailable).
    Starting,
    /// Yaw/pitch in degrees relative to straight ahead, dead zone applied.
    /// +yaw = head turned left, +pitch = looking down. Roll (sideways tilt) never shows up.
    On { yaw_deg: f32, pitch_deg: f32 },
}

/// Why Head look is Off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffReason {
    /// Switched off by the user.
    SwitchedOff,
    /// The AirPods came off the head.
    OffHead,
    /// The AirPods disconnected.
    Disconnected,
    /// No motion to be had: the daemon found no motion sensor on the AirPods within 5 s (other
    /// Apple devices connected to them, or a reconnect needed), or no sample came at all within
    /// 8 s of switching on (no AirPods connected).
    Unavailable,
    /// The airpods-tui daemon isn't running, or went away.
    NoDaemon,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadLookSettings {
    /// Angles closer than this to straight ahead read as 0; larger ones are reduced by it.
    pub dead_zone_deg: f32,
}

impl Default for HeadLookSettings {
    fn default() -> Self {
        Self { dead_zone_deg: 2.0 }
    }
}
