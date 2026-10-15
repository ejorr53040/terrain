//! How the world behaves physically, as the overlay's physics section sets
//! it: gravity, pause and step, time scale and damping.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::Grabbable;

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct PhysicsSettings {
    /// Off by default: shapes float where they're let go.
    pub gravity_on: bool,
    /// Gravity's strength, in m/s².
    pub gravity: f32,
    pub paused: bool,
    /// Set to move a paused world on by one frame; cleared once done.
    pub step: bool,
    /// 1 is real time, 0.5 half speed.
    pub time_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
}

impl Default for PhysicsSettings {
    fn default() -> Self {
        Self {
            gravity_on: false,
            gravity: 9.81,
            paused: false,
            step: false,
            time_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
        }
    }
}

/// Hands the settings to avian. A step runs one frame unpaused.
pub(crate) fn apply_physics(
    mut commands: Commands,
    mut settings: ResMut<PhysicsSettings>,
    mut gravity: ResMut<Gravity>,
    mut time: ResMut<Time<Physics>>,
    shapes: Query<Entity, With<Grabbable>>,
    new: Query<Entity, Added<Grabbable>>,
) {
    let stepping = std::mem::take(&mut settings.bypass_change_detection().step);
    let s = &*settings;
    let wanted = if s.gravity_on { Vec3::NEG_Y * s.gravity } else { Vec3::ZERO };
    if gravity.0 != wanted {
        gravity.0 = wanted;
    }
    match (s.paused && !stepping, time.is_paused()) {
        (true, false) => time.pause(),
        (false, true) => time.unpause(),
        _ => {}
    }
    if time.relative_speed() != s.time_scale {
        time.set_relative_speed(s.time_scale.max(0.0));
    }
    let damping = (LinearDamping(s.linear_damping), AngularDamping(s.angular_damping));
    if settings.is_changed() {
        for entity in &shapes {
            commands.entity(entity).insert(damping);
        }
    } else {
        for entity in &new {
            commands.entity(entity).insert(damping);
        }
    }
}
