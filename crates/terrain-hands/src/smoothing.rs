//! One Euro filters (Casiez, Roussel & Vogel, CHI 2012): heavy smoothing
//! while a signal is still, light smoothing while it moves fast, so a still
//! hand gives a still cube without making quick moves lag.

use std::f32::consts::TAU;

use glam::{FloatExt, Quat, Vec3};

/// How hard to smooth: `min_cutoff` (Hz) sets smoothing at rest, `beta`
/// (Hz per unit/s of speed) how fast it relaxes as the signal speeds up.
#[derive(Clone, Copy, Debug)]
pub struct OneEuro {
    pub min_cutoff: f32,
    pub beta: f32,
    /// Cutoff (Hz) for the speed estimate itself.
    pub d_cutoff: f32,
}

impl OneEuro {
    fn cutoff(&self, speed: f32) -> f32 {
        self.min_cutoff + self.beta * speed
    }
}

/// Blend weight toward the new sample for a low-pass at `cutoff` Hz.
fn alpha(cutoff: f32, dt: f32) -> f32 {
    1.0 / (1.0 + 1.0 / (TAU * cutoff * dt))
}

/// One Euro filter on a position.
#[derive(Clone, Debug)]
pub struct Vec3Filter {
    params: OneEuro,
    last: Option<(Vec3, Vec3)>,
}

impl Vec3Filter {
    pub fn new(params: OneEuro) -> Self {
        Self { params, last: None }
    }

    /// Smooths `x`, sampled `dt` seconds after the previous sample.
    pub fn filter(&mut self, x: Vec3, dt: f32) -> Vec3 {
        let (x_hat, dx_hat) = match self.last {
            Some((prev, prev_dx)) if dt > 0.0 => {
                let dx = prev_dx.lerp((x - prev) / dt, alpha(self.params.d_cutoff, dt));
                let a = alpha(self.params.cutoff(dx.length()), dt);
                (prev.lerp(x, a), dx)
            }
            Some(last) => last,
            None => (x, Vec3::ZERO),
        };
        self.last = Some((x_hat, dx_hat));
        x_hat
    }
}

/// One Euro filter on an orientation, blending by slerp and adapting to
/// angular speed (rad/s).
#[derive(Clone, Debug)]
pub struct QuatFilter {
    params: OneEuro,
    last: Option<(Quat, f32)>,
}

impl QuatFilter {
    pub fn new(params: OneEuro) -> Self {
        Self { params, last: None }
    }

    /// Smooths `q`, sampled `dt` seconds after the previous sample.
    pub fn filter(&mut self, q: Quat, dt: f32) -> Quat {
        let (q_hat, speed_hat) = match self.last {
            Some((prev, prev_speed)) if dt > 0.0 => {
                let raw_speed = prev.angle_between(q) / dt;
                let speed = prev_speed.lerp(raw_speed, alpha(self.params.d_cutoff, dt));
                let a = alpha(self.params.cutoff(speed), dt);
                (prev.slerp(q, a), speed)
            }
            Some(last) => last,
            None => (q, 0.0),
        };
        self.last = Some((q_hat, speed_hat));
        q_hat
    }
}
