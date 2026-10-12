//! Drives the real desktop with a hand, through the gesture engine: the
//! palm points; a pinch works like a finger on a touchscreen (a tap clicks,
//! moving drags, holding still and letting go right-clicks); a fist drags
//! the window under the pointer. What each gesture does is the built-in
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

/// Wheel steps scrolled per view width (or height) the other hand moves:
/// about fifteen across the reach.
const SCROLL_STEPS: f32 = 25.0;

/// Turns tracked hands into desktop input.
pub struct DesktopControl {
    estimator: HandStateEstimator,
    camera: CameraModel,
    engine: Engine,
    /// The other hand, while it pinches to scroll, and where in the view
    /// it was last.
    scrolling: Option<(u32, Vec2)>,
}

impl DesktopControl {
    pub fn new(camera: CameraModel) -> Self {
        Self {
            estimator: HandStateEstimator::new(camera),
            camera,
            engine: Engine::new(GestureFile::default_gestures()),
            scrolling: None,
        }
    }

    /// Whether a hand has control of the desktop.
    pub fn has_control(&self) -> bool {
        self.engine.controlling().is_some()
    }

    /// What the hands are doing now.
    pub fn status(&self) -> Status {
        match self.engine.status() {
            Status::Pointing if self.scrolling.is_some() => Status::Gesture("↕ Scroll".into()),
            status => status,
        }
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
                depth: h.position.z,
            })
            .collect();
        let mut inputs = self.engine.update(&hands, frame.t_ms);
        let Some(id) = self.engine.controlling() else {
            self.scrolling = None;
            return inputs;
        };
        if tracked.hands.iter().any(|h| h.id == id) {
            let other = tracked.hands.iter().find(|h| h.id != id);
            inputs.extend(self.scroll(other));
        }
        inputs
    }

    /// Lets go of everything and gives up control: for when hands stop
    /// arriving at all.
    pub fn let_go(&mut self) -> Vec<Input> {
        self.scrolling = None;
        self.engine.let_go()
    }

    /// Scrolling by `other`, the hand without control: pinching, it pulls
    /// the page along with it.
    // ponytail: hand-written, not in the gesture file; the one-hand Vee
    // scroll replaces it (#22).
    fn scroll(&mut self, other: Option<&HandState>) -> Option<Input> {
        let Some(hand) = other.filter(|h| h.pinching && !h.fist) else {
            self.scrolling = None;
            return None;
        };
        let at = self.in_view(hand);
        let last = match self.scrolling.replace((hand.id, at)) {
            Some((id, last)) if id == hand.id => last,
            _ => return None,
        };
        let by = (last - at) * SCROLL_STEPS;
        // Smoothing settling leaves the odd hair of motion: not a scroll.
        (by.abs().max_element() > 0.01).then_some(Input::Scroll(by))
    }

    /// Where `hand`'s palm is in the camera's view, by its direction from
    /// the camera: normalized, x right, y down.
    fn in_view(&self, hand: &HandState) -> Vec2 {
        // A palm at or behind the lens can't be: read it as far off view.
        let p = hand.position.with_z(hand.position.z.max(1e-3));
        let size = self.camera.view_size_at_1m() * p.z;
        Vec2::new(0.5 + p.x / size.x, 0.5 - p.y / size.y)
    }
}

/// What `DesktopControl` does over `frames`, as a log: each press and
/// release with its time and where the pointer was, then how far the
/// pointer ranged. The desktop isn't touched.
pub fn dry_run(frames: &[HandFrame]) -> String {
    let mut control = DesktopControl::new(CameraModel::default());
    let (mut lo, mut hi) = (Vec2::ONE, Vec2::ZERO);
    let mut at = Vec2::ZERO;
    let mut log = String::new();
    for frame in frames {
        for input in control.update(frame) {
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
