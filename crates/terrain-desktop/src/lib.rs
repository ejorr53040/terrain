//! Drives the real desktop with a hand, through the gesture engine: the
//! palm points; a pinch works like a finger on a touchscreen (a tap clicks,
//! moving drags, holding still and letting go right-clicks); a fist drags
//! the window under the pointer; a Vee scrolls; two hands brought together
//! then spread apart toggle full screen. What each gesture does is the built-in
//! gesture file's (`terrain-gestures`).
//!
//! A hand takes control only once it's held open in the reach for a
//! moment, so hands typing at the keyboard below never click.

mod uinput;

use glam::Vec2;
pub use terrain_gestures::{Action as Input, Button, Status};
use terrain_gestures::{Engine, GestureFile, Hand, Pose};
use terrain_hands::{CameraModel, HandFrame, HandState, HandStateEstimator};
pub use uinput::VirtualDesktop;

/// Turns tracked hands into desktop input.
pub struct DesktopControl {
    estimator: HandStateEstimator,
    camera: CameraModel,
    engine: Engine,
}

impl DesktopControl {
    pub fn new(camera: CameraModel) -> Self {
        Self {
            estimator: HandStateEstimator::new(camera),
            camera,
            engine: Engine::new(GestureFile::default_gestures()),
        }
    }

    /// Whether a hand has control of the desktop.
    pub fn has_control(&self) -> bool {
        self.engine.controlling().is_some()
    }

    /// What the hands are doing now.
    pub fn status(&self) -> Status {
        self.engine.status()
    }

    /// The input `frame` calls for.
    pub fn update(&mut self, frame: &HandFrame) -> Vec<Input> {
        let tracked = self.estimator.update(frame);
        let hands: Vec<Hand> = tracked
            .hands
            .iter()
            .map(|h| Hand {
                id: h.id,
                pose: Pose::of(h),
                in_view: self.in_view(h),
                position: h.position,
            })
            .collect();
        self.engine.update(&hands, frame.t_ms)
    }

    /// Lets go of everything and gives up control: for when hands stop
    /// arriving at all.
    pub fn let_go(&mut self) -> Vec<Input> {
        self.engine.let_go()
    }

    /// Where `hand`'s palm is in the camera's view, by its direction from
    /// the camera: normalized, x right, y down.
    fn in_view(&self, hand: &HandState) -> Vec2 {
        // Positions come tilted level; the view is the camera's own frame.
        let p = self.camera.tilt.inverse() * hand.position;
        // A palm at or behind the lens can't be: read it as far off view.
        let p = p.with_z(p.z.max(1e-3));
        let size = self.camera.view_size_at_1m() * p.z;
        Vec2::new(0.5 + p.x / size.x, 0.5 - p.y / size.y)
    }
}

/// What `DesktopControl` does over `frames`, as a log: each press and
/// release with its time and where the pointer was, each stretch of
/// scrolling with how far it went, then how far the pointer ranged. The
/// desktop isn't touched.
pub fn dry_run(frames: &[HandFrame]) -> String {
    let mut control = DesktopControl::new(CameraModel::default());
    let (mut lo, mut hi) = (Vec2::ONE, Vec2::ZERO);
    let mut at = Vec2::ZERO;
    let mut log = String::new();
    // Scrolling under way: from when, until when, and how far so far.
    let mut scroll: Option<(u64, u64, Vec2)> = None;
    let scrolled = |(from, to, by): (u64, u64, Vec2)| {
        format!("{from:>6}–{to} ms  Scroll({:.1}, {:.1})\n", by.x, by.y)
    };
    for frame in frames {
        let inputs = control.update(frame);
        let by: Vec2 = inputs
            .iter()
            .filter_map(|i| match i {
                Input::Scroll(by) => Some(*by),
                _ => None,
            })
            .sum();
        if by != Vec2::ZERO {
            let (from, _, sum) = scroll.unwrap_or((frame.t_ms, 0, Vec2::ZERO));
            scroll = Some((from, frame.t_ms, sum + by));
        } else if let Some(run) = scroll.take() {
            log += &scrolled(run);
        }
        for input in inputs {
            match input {
                Input::PointTo(p) => {
                    lo = lo.min(p);
                    hi = hi.max(p);
                    at = p;
                }
                Input::Scroll(_) => {}
                other => {
                    log += &format!(
                        "{:>6} ms  {other:?} at ({:.2}, {:.2})\n",
                        frame.t_ms, at.x, at.y
                    )
                }
            }
        }
    }
    if let Some(run) = scroll {
        log += &scrolled(run);
    }
    log += &format!(
        "{} frames; pointer ranged x {:.2}..{:.2}, y {:.2}..{:.2}\n",
        frames.len(),
        lo.x,
        hi.x,
        lo.y,
        hi.y
    );
    log
}
