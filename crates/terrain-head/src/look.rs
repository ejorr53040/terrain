//! The pure core of Head look: AirPods orientations in, yaw and pitch out.

use glam::{Quat, Vec3};
use serde::Deserialize;

use crate::{HeadLook, HeadLookSettings, OffReason};

/// The AirPods Max's body axis pointing from the left ear cup toward the right one: the only
/// body axis assumed; up comes from gravity. A turn-left/look-down calibration on the
/// prototype put looking down about body ≈ (−0.96, 0.24, 0.12), i.e. left ≈ −X.
/// To be verified on hardware: if looking down reads as negative pitch, negate it.
const EAR_AXIS: Vec3 = Vec3::X;

/// The first sample after a start is a junk identity: straight ahead is taken this long after it.
const SETTLE_NS: u64 = 500_000_000;

/// One head motion sample as the daemon sends it.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub(crate) struct HeadMotionSample {
    pub timestamp_ns: u64,
    /// Device-to-world rotation, world Z up: w, x, y, z.
    pub quaternion: [f32; 4],
}

impl HeadMotionSample {
    fn rotation(&self) -> Quat {
        let [w, x, y, z] = self.quaternion;
        Quat::from_xyzw(x, y, z, w).normalize()
    }
}

/// Head look's state, fed samples and the end of motion.
pub(crate) struct Look {
    settings: HeadLookSettings,
    phase: Phase,
}

enum Phase {
    Off(OffReason),
    /// Waiting for straight ahead; holds the first (junk) sample's time once it came.
    Starting {
        first_ns: Option<u64>,
    },
    On {
        straight: StraightAhead,
        latest: Quat,
    },
}

/// The head's forward direction at straight ahead, in the AirPods' body frame, and its
/// heading in the world.
struct StraightAhead {
    forward: Vec3,
    heading: f32,
}

impl StraightAhead {
    fn at(q: Quat) -> Self {
        let up = q.inverse() * Vec3::Z;
        let ear = (EAR_AXIS - up * EAR_AXIS.dot(up)).normalize_or(up.any_orthonormal_vector());
        let forward = up.cross(ear);
        Self {
            forward,
            heading: heading(q * forward),
        }
    }

    /// Yaw and pitch of orientation `q` in degrees: + left, + down.
    fn angles(&self, q: Quat) -> (f32, f32) {
        let f = q * self.forward;
        let mut yaw = (heading(f) - self.heading).to_degrees();
        if yaw > 180.0 {
            yaw -= 360.0;
        } else if yaw < -180.0 {
            yaw += 360.0;
        }
        let pitch = -f.z.clamp(-1.0, 1.0).asin().to_degrees();
        (yaw, pitch)
    }
}

/// Heading of `v` about world Z, counter-clockwise from +X seen from above.
fn heading(v: Vec3) -> f32 {
    v.y.atan2(v.x)
}

impl Look {
    pub fn new(settings: HeadLookSettings) -> Self {
        Self {
            settings,
            phase: Phase::Off(OffReason::SwitchedOff),
        }
    }

    pub fn switch_on(&mut self) {
        self.phase = Phase::Starting { first_ns: None };
    }

    pub fn switch_off(&mut self) {
        self.phase = Phase::Off(OffReason::SwitchedOff);
    }

    /// Motion ended for `reason`: Off with it, unless already Off. Returns whether it was on.
    pub fn end(&mut self, reason: OffReason) -> bool {
        let was_on = !matches!(self.phase, Phase::Off(_));
        if was_on {
            self.phase = Phase::Off(reason);
        }
        was_on
    }

    pub fn sample(&mut self, sample: HeadMotionSample) {
        let q = sample.rotation();
        match &mut self.phase {
            Phase::Off(_) => {}
            Phase::Starting { first_ns: None } => {
                self.phase = Phase::Starting {
                    first_ns: Some(sample.timestamp_ns),
                }
            }
            Phase::Starting {
                first_ns: Some(first),
            } => {
                if sample.timestamp_ns.saturating_sub(*first) >= SETTLE_NS {
                    self.phase = Phase::On {
                        straight: StraightAhead::at(q),
                        latest: q,
                    }
                }
            }
            Phase::On { latest, .. } => *latest = q,
        }
    }

    /// Straight ahead becomes the current orientation; nothing unless On.
    pub fn recenter(&mut self) {
        if let Phase::On { straight, latest } = &mut self.phase {
            *straight = StraightAhead::at(*latest);
        }
    }

    pub fn set_settings(&mut self, settings: HeadLookSettings) {
        self.settings = settings;
    }

    pub fn get(&self) -> HeadLook {
        match &self.phase {
            Phase::Off(reason) => HeadLook::Off(*reason),
            Phase::Starting { .. } => HeadLook::Starting,
            Phase::On { straight, latest } => {
                let (yaw, pitch) = straight.angles(*latest);
                let dz = self.settings.dead_zone_deg;
                HeadLook::On {
                    yaw_deg: dead_zone(yaw, dz),
                    pitch_deg: dead_zone(pitch, dz),
                }
            }
        }
    }
}

/// `a` with the dead zone `dz` taken off its size: 0 inside it.
fn dead_zone(a: f32, dz: f32) -> f32 {
    a.signum() * (a.abs() - dz).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat3, Quat, Vec3};

    const MS: u64 = 1_000_000;

    /// Straight ahead faces this heading in the world (counter-clockwise from +X, from above).
    const HEADING_DEG: f32 = 70.0;

    fn forward() -> Vec3 {
        let h = HEADING_DEG.to_radians();
        Vec3::new(h.cos(), h.sin(), 0.0)
    }

    fn left() -> Vec3 {
        Vec3::Z.cross(forward())
    }

    /// The Max worn with the head straight ahead: body +X toward the right ear, +Y up through
    /// the headband, -Z forward, and the headband tipped 15° forward on the head so the body
    /// axes are not the world's.
    fn worn() -> Quat {
        let level = Quat::from_mat3(&Mat3::from_cols(-left(), Vec3::Z, -forward()));
        Quat::from_axis_angle(left(), 15f32.to_radians()) * level
    }

    /// The head rotated by `deg` about the world axis `axis`, from straight ahead.
    fn head_turned(axis: Vec3, deg: f32) -> Quat {
        Quat::from_axis_angle(axis, deg.to_radians()) * worn()
    }

    fn sample(at_ms: u64, q: Quat) -> HeadMotionSample {
        HeadMotionSample {
            timestamp_ns: at_ms * MS,
            quaternion: [q.w, q.x, q.y, q.z],
        }
    }

    /// Switched on with no dead zone, fed the junk identity first sample, then the head held
    /// straight ahead at 50 Hz for 0.6 s.
    fn settled() -> Look {
        let mut look = Look::new(HeadLookSettings { dead_zone_deg: 0.0 });
        look.switch_on();
        look.sample(sample(0, Quat::IDENTITY));
        for i in 1..=30 {
            look.sample(sample(i * 20, worn()));
        }
        look
    }

    fn assert_reads(look: &Look, yaw: f32, pitch: f32) {
        match look.get() {
            HeadLook::On { yaw_deg, pitch_deg } => assert!(
                (yaw_deg - yaw).abs() < 0.5 && (pitch_deg - pitch).abs() < 0.5,
                "read yaw {yaw_deg} pitch {pitch_deg}, expected yaw {yaw} pitch {pitch}"
            ),
            other => panic!("expected On, got {other:?}"),
        }
    }

    #[test]
    fn turning_left_30_reads_yaw_30() {
        let mut look = settled();
        look.sample(sample(620, head_turned(Vec3::Z, 30.0)));
        assert_reads(&look, 30.0, 0.0);
    }

    #[test]
    fn nodding_down_20_reads_pitch_20() {
        let mut look = settled();
        look.sample(sample(620, head_turned(left(), 20.0)));
        assert_reads(&look, 0.0, 20.0);
    }

    #[test]
    fn rolling_25_reads_nothing() {
        let mut look = settled();
        look.sample(sample(620, head_turned(forward(), 25.0)));
        assert_reads(&look, 0.0, 0.0);
    }

    #[test]
    fn turning_left_then_nodding_down_reads_both() {
        let mut look = settled();
        let q = Quat::from_rotation_z(30f32.to_radians()) * head_turned(left(), 20.0);
        look.sample(sample(620, q));
        assert_reads(&look, 30.0, 20.0);
    }

    #[test]
    fn straight_ahead_is_taken_half_a_second_after_the_junk_first_sample() {
        let mut look = Look::new(HeadLookSettings { dead_zone_deg: 0.0 });
        look.switch_on();
        assert_eq!(look.get(), HeadLook::Starting);
        look.sample(sample(1000, Quat::IDENTITY));
        for i in 1..=24 {
            look.sample(sample(1000 + i * 20, worn()));
        }
        assert_eq!(look.get(), HeadLook::Starting, "0.48 s in");
        look.sample(sample(1500, worn()));
        assert_reads(&look, 0.0, 0.0);
    }

    #[test]
    fn the_dead_zone_swallows_small_turns_and_is_taken_off_larger_ones() {
        let mut look = settled();
        look.set_settings(HeadLookSettings { dead_zone_deg: 2.0 });
        look.sample(sample(620, head_turned(Vec3::Z, 1.5)));
        assert_eq!(
            look.get(),
            HeadLook::On {
                yaw_deg: 0.0,
                pitch_deg: 0.0
            }
        );
        look.sample(sample(640, head_turned(Vec3::Z, -30.0)));
        assert_reads(&look, -28.0, 0.0);
    }

    #[test]
    fn recentering_makes_the_current_orientation_straight_ahead() {
        let mut look = settled();
        let turned = Quat::from_rotation_z(30f32.to_radians()) * head_turned(left(), 20.0);
        look.sample(sample(620, turned));
        look.recenter();
        assert_reads(&look, 0.0, 0.0);
        look.sample(sample(
            640,
            Quat::from_rotation_z(10f32.to_radians()) * turned,
        ));
        assert_reads(&look, 10.0, 0.0);
    }

    #[test]
    fn recentering_while_starting_does_nothing() {
        let mut look = Look::new(HeadLookSettings { dead_zone_deg: 0.0 });
        look.switch_on();
        look.sample(sample(0, Quat::IDENTITY));
        look.recenter();
        assert_eq!(look.get(), HeadLook::Starting);
    }

    #[test]
    fn motion_ending_turns_it_off_with_the_reason_until_switched_on_again() {
        let mut look = settled();
        assert!(look.end(OffReason::OffHead), "it was on");
        assert_eq!(look.get(), HeadLook::Off(OffReason::OffHead));
        look.sample(sample(640, head_turned(Vec3::Z, 30.0)));
        assert_eq!(look.get(), HeadLook::Off(OffReason::OffHead));
        assert!(!look.end(OffReason::Disconnected), "it was already off");
        assert_eq!(look.get(), HeadLook::Off(OffReason::OffHead));

        look.switch_on();
        assert_eq!(look.get(), HeadLook::Starting);
        look.sample(sample(5000, Quat::IDENTITY));
        look.sample(sample(5500, head_turned(Vec3::Z, 30.0)));
        look.sample(sample(5520, worn()));
        assert_reads(&look, -30.0, 0.0);
    }

    #[test]
    fn motion_ending_while_starting_turns_it_off() {
        let mut look = Look::new(HeadLookSettings::default());
        look.switch_on();
        assert!(look.end(OffReason::Unavailable));
        assert_eq!(look.get(), HeadLook::Off(OffReason::Unavailable));
    }

    #[test]
    fn switching_off_ignores_the_samples_after_it() {
        let mut look = settled();
        look.switch_off();
        assert_eq!(look.get(), HeadLook::Off(OffReason::SwitchedOff));
        look.sample(sample(640, worn()));
        assert_eq!(look.get(), HeadLook::Off(OffReason::SwitchedOff));
    }
}
