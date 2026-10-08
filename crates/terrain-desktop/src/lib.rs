//! Drives the real desktop with a hand: the palm points; a pinch works like
//! a finger on a touchscreen (a tap clicks, moving drags, holding still
//! right-clicks); a fist drags the window under the pointer.
//!
//! A hand takes control only once it's held open in the reach for a
//! moment, so hands typing at the keyboard below never click.

mod uinput;

use glam::Vec2;
use terrain_hands::{CameraModel, FORGET_AFTER_MS, HandFrame, HandState, HandStateEstimator};
pub use uinput::VirtualDesktop;

/// What the desktop should do, in the order it should happen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    /// Move the pointer to here on the screen: normalized, x right, y down.
    PointTo(Vec2),
    Press(Button),
    Release(Button),
    /// Scroll by this many wheel steps: y down the page, x to the right.
    Scroll(Vec2),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    /// The Super (logo) key: the window manager drags windows on Super +
    /// left button.
    Super,
}

/// The part of the camera's view the hand sweeps to cover the whole screen
/// (normalized view coordinates, x right, y down). Reaching the view's own
/// edges is awkward, and a palm there is half out of frame and lost.
const REACH_MIN: Vec2 = Vec2::new(0.2, 0.2);
const REACH_MAX: Vec2 = Vec2::new(0.8, 0.8);

/// A hand takes control once held open in the reach this long (ms).
const ENGAGE_MS: u64 = 400;
/// A hand in control gives it up once it's been open outside the reach
/// this long (ms): lowered back to the keyboard.
const DISENGAGE_MS: u64 = 800;

/// A grip has to read the same for this long (ms) before it counts, so a
/// one-frame misreading does nothing...
const GRIP_SETTLE_MS: u64 = 30;
/// ...and a pinch from an open hand this long: a hand closing into a fist
/// reads as a pinch for about 100 ms on the way, and mustn't click.
const PINCH_SETTLE_MS: u64 = 120;

/// A pinch that moves the pointer this far (screen widths) is a drag...
const DRAG_START: f32 = 0.02;
/// ...and one held still this long (ms) a right click.
const LONG_PRESS_MS: u64 = 600;

/// Wheel steps scrolled per view width (or height) the other hand moves:
/// about fifteen across the reach.
const SCROLL_STEPS: f32 = 25.0;

/// What the controlling hand is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Grip {
    Open,
    Pinch,
    Fist,
}

impl Grip {
    /// What the grip holds down, in the order it presses them.
    fn held(self) -> &'static [Button] {
        match self {
            // A pinch's button waits on what the pinch turns out to be.
            Grip::Open | Grip::Pinch => &[],
            Grip::Fist => &[Button::Super, Button::Left],
        }
    }

    /// The grip `hand` reads as.
    fn of(hand: &HandState) -> Grip {
        if hand.fist {
            Grip::Fist
        } else if hand.pinching {
            Grip::Pinch
        } else {
            Grip::Open
        }
    }
}

/// A pinch under way.
#[derive(Clone, Copy, Debug)]
struct Pinch {
    /// Where the pointer was when it closed; it stays there until the pinch
    /// moves, so a click lands where it was aimed.
    at: Vec2,
    since: u64,
    phase: PinchPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PinchPhase {
    /// Not yet a click, drag or right click.
    Undecided,
    /// It moved: the left button is down, dragging.
    Dragging,
    /// It held still: it right-clicked, and does nothing more.
    RightClicked,
}

/// Who has control of the desktop.
#[derive(Clone, Copy, Debug)]
enum Control {
    /// No hand: waiting for one to be raised open into the reach.
    Free,
    /// This hand is open in the reach, and has been since then (ms).
    Raising { id: u32, since: u64 },
    /// This hand has control.
    Held { id: u32 },
}

/// Turns tracked hands into desktop input.
pub struct DesktopControl {
    estimator: HandStateEstimator,
    camera: CameraModel,
    control: Control,
    /// The grip acted on, and so the buttons held.
    grip: Grip,
    /// A different grip seen, and when it was first seen.
    next_grip: Option<(Grip, u64)>,
    /// When the controlling hand was last seen.
    last_seen_ms: u64,
    /// Since when the controlling hand has been open outside the reach.
    outside_since: Option<u64>,
    /// The pinch under way, while the grip is `Pinch`.
    pinch: Option<Pinch>,
    /// The other hand, while it pinches to scroll, and where in the view
    /// it was last.
    scrolling: Option<(u32, Vec2)>,
}

impl DesktopControl {
    pub fn new(camera: CameraModel) -> Self {
        Self {
            estimator: HandStateEstimator::new(camera),
            camera,
            control: Control::Free,
            grip: Grip::Open,
            next_grip: None,
            last_seen_ms: 0,
            outside_since: None,
            pinch: None,
            scrolling: None,
        }
    }

    /// Whether a hand has control of the desktop.
    pub fn has_control(&self) -> bool {
        matches!(self.control, Control::Held { .. })
    }

    /// The input `frame` calls for.
    pub fn update(&mut self, frame: &HandFrame) -> Vec<Input> {
        let t_ms = frame.t_ms;
        let tracked = self.estimator.update(frame);
        let id = match self.control {
            Control::Held { id } => id,
            _ => {
                self.raise(&tracked.hands, t_ms);
                match self.control {
                    Control::Held { id } => id,
                    _ => return Vec::new(),
                }
            }
        };
        let Some(hand) = tracked.hands.iter().find(|h| h.id == id) else {
            // A hand gone for good lets go of whatever it held; a blink
            // of a dropout doesn't.
            if t_ms.saturating_sub(self.last_seen_ms) > FORGET_AFTER_MS {
                return self.let_go();
            }
            return Vec::new();
        };
        self.last_seen_ms = t_ms;
        let in_view = self.in_view(hand);
        let grip = Grip::of(hand);
        if grip == Grip::Open && self.grip == Grip::Open && !in_reach(in_view) {
            let since = *self.outside_since.get_or_insert(t_ms);
            if t_ms.saturating_sub(since) >= DISENGAGE_MS {
                return self.let_go();
            }
        } else {
            self.outside_since = None;
        }
        // An open hand outside the reach is on its way down to the
        // keyboard: it can't take hold of anything new.
        let grip = if self.grip == Grip::Open && !in_reach(in_view) {
            Grip::Open
        } else {
            grip
        };
        let point = to_screen(in_view);
        let mut inputs = self.settle_grip(grip, point, t_ms);
        inputs.extend(self.follow_pinch(point, t_ms));
        let other = tracked.hands.iter().find(|h| h.id != id);
        inputs.extend(self.scroll(other));
        inputs
    }

    /// Scrolling by `other`, the hand without control: pinching, it pulls
    /// the page along with it.
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

    /// Points at `point`, unless a pinch holds the pointer where it closed;
    /// decides what an undecided pinch is.
    fn follow_pinch(&mut self, point: Vec2, t_ms: u64) -> Vec<Input> {
        let Some(pinch) = &mut self.pinch else {
            return vec![Input::PointTo(point)];
        };
        match pinch.phase {
            PinchPhase::Undecided if point.distance(pinch.at) > DRAG_START => {
                pinch.phase = PinchPhase::Dragging;
                vec![
                    Input::PointTo(pinch.at),
                    Input::Press(Button::Left),
                    Input::PointTo(point),
                ]
            }
            PinchPhase::Undecided if t_ms.saturating_sub(pinch.since) >= LONG_PRESS_MS => {
                pinch.phase = PinchPhase::RightClicked;
                vec![
                    Input::PointTo(pinch.at),
                    Input::Press(Button::Right),
                    Input::Release(Button::Right),
                ]
            }
            PinchPhase::Dragging => vec![Input::PointTo(point)],
            PinchPhase::Undecided | PinchPhase::RightClicked => vec![Input::PointTo(pinch.at)],
        }
    }

    /// Hands control to a hand held open in the reach long enough.
    fn raise(&mut self, hands: &[HandState], t_ms: u64) {
        let raised = |h: &&HandState| Grip::of(h) == Grip::Open && in_reach(self.in_view(h));
        if let Control::Raising { id, since } = self.control
            && hands.iter().filter(raised).any(|h| h.id == id)
        {
            if t_ms.saturating_sub(since) >= ENGAGE_MS {
                self.control = Control::Held { id };
                self.last_seen_ms = t_ms;
                self.outside_since = None;
            }
            return;
        }
        self.control = match hands.iter().find(raised) {
            Some(h) => Control::Raising {
                id: h.id,
                since: t_ms,
            },
            None => Control::Free,
        };
    }

    /// Acts on `grip` once it has read steadily long enough; `point` is
    /// where the hand points now.
    fn settle_grip(&mut self, grip: Grip, point: Vec2, t_ms: u64) -> Vec<Input> {
        // A fist opening reads as a pinch on its way: still the fist.
        let grip = if grip == Grip::Pinch && self.grip == Grip::Fist {
            Grip::Fist
        } else {
            grip
        };
        if grip == self.grip {
            self.next_grip = None;
            return Vec::new();
        }
        let since = match self.next_grip {
            Some((g, since)) if g == grip => since,
            _ => t_ms,
        };
        self.next_grip = Some((grip, since));
        let settle = if grip == Grip::Pinch && self.grip == Grip::Open {
            PINCH_SETTLE_MS
        } else {
            GRIP_SETTLE_MS
        };
        if t_ms.saturating_sub(since) < settle {
            return Vec::new();
        }
        // An undecided pinch opening is a tap: a click where it closed. One
        // tightening into a fist was the fist forming, and one lost with
        // its hand never happened.
        let tapped =
            grip == Grip::Open && self.pinch.is_some_and(|p| p.phase == PinchPhase::Undecided);
        let mut inputs = self.change_grip(grip);
        if tapped {
            inputs.extend([Input::Press(Button::Left), Input::Release(Button::Left)]);
        }
        if grip == Grip::Pinch {
            self.pinch = Some(Pinch {
                at: point,
                since: t_ms,
                phase: PinchPhase::Undecided,
            });
        }
        inputs
    }

    /// Lets go of everything and gives up control: for when hands stop
    /// arriving at all.
    pub fn let_go(&mut self) -> Vec<Input> {
        self.control = Control::Free;
        self.scrolling = None;
        self.outside_since = None;
        self.change_grip(Grip::Open)
    }

    /// Lets go of what the current grip holds and takes hold for `grip`.
    fn change_grip(&mut self, grip: Grip) -> Vec<Input> {
        self.next_grip = None;
        if grip == self.grip {
            return Vec::new();
        }
        let dragging = self
            .pinch
            .take()
            .is_some_and(|p| p.phase == PinchPhase::Dragging);
        let drag_release = dragging.then_some(Input::Release(Button::Left));
        let releases = self.grip.held().iter().rev().map(|&b| Input::Release(b));
        let presses = grip.held().iter().map(|&b| Input::Press(b));
        let inputs = drag_release
            .into_iter()
            .chain(releases)
            .chain(presses)
            .collect();
        self.grip = grip;
        inputs
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

fn in_reach(in_view: Vec2) -> bool {
    in_view.cmpge(REACH_MIN).all() && in_view.cmple(REACH_MAX).all()
}

/// Where a palm at `in_view` points on the screen: across the reach,
/// clamped to the screen.
fn to_screen(in_view: Vec2) -> Vec2 {
    ((in_view - REACH_MIN) / (REACH_MAX - REACH_MIN)).clamp(Vec2::ZERO, Vec2::ONE)
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
