//! Gestures for driving a desktop by hand: the Pose a hand is in, and an
//! engine that runs the gesture file's state machines on the hand with
//! control. Engagement (raising an open hand into the Reach to take
//! control, lowering it to give control up) and letting go of a lost hand
//! are built in, not in the gesture file (ADR 0002). Spreads need no
//! control, and run on any two hands (ADR 0008).

mod file;

pub use file::{
    DEFAULT, Direction, Gesture, GestureFile, Guard, Pointer, Scroll, Spread, State, Transition,
};

use std::collections::VecDeque;

use glam::{Vec2, Vec3};
use serde::Deserialize;
use terrain_hands::{FORGET_AFTER_MS, HandState};

/// The shape a hand is in during one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Pose {
    Open,
    Pinch,
    Fist,
    /// Index and middle fingers up, the others curled.
    Vee,
}

impl Pose {
    /// The Pose `hand` reads as. A fist's thumb often rests on the index
    /// finger, so a fist can read as pinching too: the fist wins.
    pub fn of(hand: &HandState) -> Pose {
        if hand.fist {
            Pose::Fist
        } else if hand.pinching {
            Pose::Pinch
        } else if hand.vee {
            Pose::Vee
        } else {
            Pose::Open
        }
    }
}

/// What the desktop should do, in the order it should happen.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub enum Action {
    /// Move the pointer to here on the screen: normalized, x right, y down.
    PointTo(Vec2),
    Press(Button),
    Release(Button),
    /// Scroll by this many wheel steps: y down the page, x to the right.
    Scroll(Vec2),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Button {
    Left,
    Right,
    /// The Super (logo) key: the window manager drags windows on Super +
    /// left button.
    Super,
    /// The F key: with Super, the window manager's full screen.
    F,
    /// The keys of herdr's chords: Ctrl + Alt + h/j/k/l moves the focus
    /// between panes, Alt + arrows steps through tabs and workspaces.
    Ctrl,
    Alt,
    H,
    J,
    K,
    L,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    /// The X key: with Super + Ctrl, Omarchy's dictation toggle.
    X,
}

/// A tracked hand, as the engine sees it.
#[derive(Clone, Copy, Debug)]
pub struct Hand {
    /// Tells this hand apart from others from frame to frame.
    pub id: u32,
    pub pose: Pose,
    /// Where the palm is in the camera's view: normalized, x right, y down.
    pub in_view: Vec2,
    /// Where the palm is: meters, x right, y up, z away from the camera.
    pub position: Vec3,
}

/// What the hands are doing, for showing on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// No hand has control.
    Disengaged,
    /// A hand is held open in the Reach, about to take control.
    Engaging,
    /// The hand with control is pointing, in no gesture.
    Pointing,
    /// The hand with control is in a gesture: its state's label.
    Gesture(String),
}

/// The part of the camera's view the hand sweeps to cover the whole screen
/// (normalized view coordinates, x right, y down). Reaching the view's own
/// edges is awkward, and a palm there is half out of frame and lost.
const REACH_MIN: Vec2 = Vec2::new(0.2, 0.2);
const REACH_MAX: Vec2 = Vec2::new(0.8, 0.8);

/// A hand takes control once held open in the Reach this long (ms).
const ENGAGE_MS: u64 = 400;
/// A hand in control gives it up once it's been open outside the Reach
/// this long (ms): lowered back to the keyboard.
const DISENGAGE_MS: u64 = 800;

/// Who has control.
#[derive(Clone, Copy, Debug)]
enum Control {
    /// No hand: waiting for one to be raised open into the Reach.
    Free,
    /// This hand is open in the Reach, and has been since then (ms).
    Raising { id: u32, since: u64 },
    /// This hand has control.
    Held { id: u32 },
}

/// A gesture under way.
#[derive(Clone, Debug)]
struct Active {
    /// Which of the file's gestures.
    gesture: usize,
    state: State,
    /// When the state was entered (ms).
    entered_ms: u64,
    /// Where the pointer was when the gesture started.
    at: Vec2,
    /// Where the pointer has been lately (ms, screen), oldest first.
    // ponytail: kept for TRAIL_MS, so a guard window longer than that sees only TRAIL_MS.
    trail: VecDeque<(u64, Vec2)>,
}

/// How much of the pointer's path a gesture remembers (ms).
const TRAIL_MS: u64 = 1000;

/// Runs a gesture file on the hand with control.
pub struct Engine {
    gestures: GestureFile,
    control: Control,
    /// When the hand with control was last seen.
    last_seen_ms: u64,
    /// Since when the hand with control has been open outside the Reach.
    outside_since: Option<u64>,
    /// The Pose the hand with control reads, and since when.
    pose: (Pose, u64),
    active: Option<Active>,
    /// How far along each of the file's Spreads is.
    spreads: Vec<Arming>,
}

/// How far along a Spread is.
#[derive(Clone, Copy, Debug, Default)]
struct Arming {
    /// Since when the palms have been together (ms).
    together_since: Option<u64>,
    /// When the palms were last seen near enough to be coming together
    /// (ms). Like the arming, a dropout or a stray Pose keeps it, for at
    /// most `within_ms`.
    near_at: Option<u64>,
    /// When the palms were last together, having been so long enough (ms).
    armed_at: Option<u64>,
    /// Whether it fired since it was armed: it stays armed after firing, so
    /// a pinch still lingering from the palms meeting can't start a gesture.
    fired: bool,
    /// When both hands were last seen in a Spread's Poses (ms).
    seen_at: Option<u64>,
    /// How long, with both hands seen, since the palms were last together
    /// (ms): a dropout or a stray Pose stops this clock.
    parted_ms: u64,
}

impl Arming {
    /// Palms near, together or just parted: a pinch here is the palms
    /// meeting, not a gesture of the hand with control.
    fn under_way(&self) -> bool {
        self.near_at.is_some() || self.together_since.is_some() || self.armed_at.is_some()
    }
}

impl Engine {
    pub fn new(gestures: GestureFile) -> Self {
        Self {
            spreads: vec![Arming::default(); gestures.spreads.len()],
            gestures,
            control: Control::Free,
            last_seen_ms: 0,
            outside_since: None,
            pose: (Pose::Open, 0),
            active: None,
        }
    }

    /// The hand with control, if one has it.
    pub fn controlling(&self) -> Option<u32> {
        match self.control {
            Control::Held { id } => Some(id),
            _ => None,
        }
    }

    pub fn status(&self) -> Status {
        match (self.control, &self.active) {
            (Control::Free, _) => Status::Disengaged,
            (Control::Raising { .. }, _) => Status::Engaging,
            (Control::Held { .. }, None) => Status::Pointing,
            (Control::Held { .. }, Some(a)) => Status::Gesture(a.state.label.clone()),
        }
    }

    /// The actions `hands`, seen at `t_ms`, call for.
    pub fn update(&mut self, hands: &[Hand], t_ms: u64) -> Vec<Action> {
        // After the hand with control, so a gesture it starts this frame
        // already blocks a Spread.
        let mut actions = self.one_hand(hands, t_ms);
        actions.extend(self.spreads(hands, t_ms));
        actions
    }

    /// The key chords of the Spreads `hands` complete.
    fn spreads(&mut self, hands: &[Hand], t_ms: u64) -> Vec<Action> {
        let mut actions = Vec::new();
        for (g, arming) in self.gestures.spreads.iter().zip(&mut self.spreads) {
            let Arming {
                together_since: since,
                near_at,
                armed_at: armed,
                fired,
                seen_at,
                parted_ms,
            } = arming;
            if armed.is_some_and(|at| t_ms.saturating_sub(at) > g.within_ms) {
                *armed = None;
            }
            if near_at.is_some_and(|at| t_ms.saturating_sub(at) > g.within_ms) {
                *near_at = None;
            }
            let [a, b] = hands else {
                // A hand flung to the edge of the view drops out for a moment
                // mid-spread: that keeps it armed, for as long as it lasts.
                (*since, *seen_at) = (None, None);
                continue;
            };
            if self.active.is_some() {
                (*since, *armed, *near_at) = (None, None, None);
                continue;
            }
            // A hand opening to spread can read as another Pose for a frame
            // (a Vee, live): that keeps it armed, as a dropout does.
            if !g.poses.contains(&a.pose) || !g.poses.contains(&b.pose) {
                (*since, *seen_at) = (None, None);
                continue;
            }
            let gap = (a.position - b.position).truncate().length();
            *near_at = (gap < g.near).then_some(t_ms);
            *parted_ms += seen_at.map_or(0, |at| t_ms.saturating_sub(at));
            *seen_at = Some(t_ms);
            if gap < g.together {
                *parted_ms = 0;
                let from = *since.get_or_insert(t_ms);
                if t_ms.saturating_sub(from) >= g.held_ms {
                    (*armed, *fired) = (Some(t_ms), false);
                }
                continue;
            }
            *since = None;
            if armed.is_some() && !*fired && gap > g.apart && *parted_ms <= g.parting_ms {
                *fired = true;
                actions.extend(g.keys.iter().map(|&k| Action::Press(k)));
                actions.extend(g.keys.iter().rev().map(|&k| Action::Release(k)));
            }
        }
        // One parting is one Spread: hands that met in one Spread's Poses
        // and parted in another's mustn't send both chords.
        if !actions.is_empty() {
            for arming in self.spreads.iter_mut().filter(|a| !a.fired) {
                arming.armed_at = None;
            }
        }
        actions
    }

    /// The actions of the hand with control.
    fn one_hand(&mut self, hands: &[Hand], t_ms: u64) -> Vec<Action> {
        let id = match self.control {
            Control::Held { id } => id,
            _ => {
                self.raise(hands, t_ms);
                match self.control {
                    Control::Held { id } => id,
                    _ => return Vec::new(),
                }
            }
        };
        let Some(hand) = hands.iter().find(|h| h.id == id) else {
            // A hand gone for good lets go of whatever it held; a blink
            // of a dropout doesn't.
            if t_ms.saturating_sub(self.last_seen_ms) > FORGET_AFTER_MS {
                return self.let_go();
            }
            return Vec::new();
        };
        let dt_ms = t_ms.saturating_sub(self.last_seen_ms);
        self.last_seen_ms = t_ms;
        let in_reach = in_reach(hand.in_view);
        if hand.pose == Pose::Open && self.active.is_none() && !in_reach {
            let since = *self.outside_since.get_or_insert(t_ms);
            if t_ms.saturating_sub(since) >= DISENGAGE_MS {
                return self.let_go();
            }
        } else {
            self.outside_since = None;
        }
        // An open hand outside the Reach is on its way down to the
        // keyboard: it can't start a gesture.
        let pose = if self.active.is_none() && !in_reach {
            Pose::Open
        } else {
            hand.pose
        };
        if pose != self.pose.0 {
            self.pose = (pose, t_ms);
        }
        let point = to_screen(hand.in_view);
        let mut actions = self.step(hand, point, t_ms);
        if let Some(a) = &self.active
            && let Some(scroll) = a.state.scroll
        {
            let by = scroll.by(point - a.at, dt_ms);
            if by != Vec2::ZERO {
                actions.push(Action::Scroll(by));
            }
        }
        let pointer = match &self.active {
            Some(a) if a.state.pointer == Pointer::Frozen => a.at,
            _ => point,
        };
        actions.push(Action::PointTo(pointer));
        actions
    }

    /// Lets go of everything and gives up control: for when the hand is
    /// lost or hands stop arriving at all.
    pub fn let_go(&mut self) -> Vec<Action> {
        self.control = Control::Free;
        self.outside_since = None;
        let held = self
            .active
            .take()
            .map(|a| a.state.holds)
            .unwrap_or_default();
        held.iter().rev().map(|&b| Action::Release(b)).collect()
    }

    /// Takes the gesture under way's first enabled transition, or starts a
    /// gesture whose Pose has settled.
    fn step(&mut self, hand: &Hand, point: Vec2, t_ms: u64) -> Vec<Action> {
        let mut actions = Vec::new();
        if let Some(active) = &mut self.active {
            active.trail.push_back((t_ms, to_screen(hand.in_view)));
            while active.trail.front().is_some_and(|&(t, _)| t_ms - t > TRAIL_MS) {
                active.trail.pop_front();
            }
        }
        if let Some(active) = &self.active {
            let to = active
                .state
                .on
                .iter()
                .find(|tr| tr.when.iter().all(|&g| self.holds(g, active, hand, t_ms)))
                .map(|tr| tr.to.clone());
            if let Some(to) = to {
                actions.extend(self.enter(&to, t_ms));
            }
        }
        if self.active.is_none() && !self.spreads.iter().any(Arming::under_way) {
            let (pose, since) = self.pose;
            let settled = self
                .gestures
                .gestures
                .iter()
                .position(|g| g.pose == pose && t_ms.saturating_sub(since) >= g.settle_ms);
            if let Some(i) = settled {
                let start = self.gestures.gestures[i].start.clone();
                self.active = Some(Active {
                    gesture: i,
                    state: State::default(),
                    entered_ms: t_ms,
                    at: point,
                    trail: VecDeque::from([(t_ms, to_screen(hand.in_view))]),
                });
                actions.extend(self.enter(&start, t_ms));
            }
        }
        actions
    }

    /// Whether `guard` holds for the gesture under way.
    fn holds(&self, guard: Guard, active: &Active, hand: &Hand, t_ms: u64) -> bool {
        match guard {
            Guard::Pose(pose, ms) => self.pose.0 == pose && t_ms.saturating_sub(self.pose.1) >= ms,
            Guard::After(ms) => t_ms.saturating_sub(active.entered_ms) >= ms,
            Guard::Moved(by) => moved(active, hand).length() > by,
            Guard::MovedFor(by, ms) => {
                // Where the pointer was `ms` ago: the newest point at least that old.
                let then = active.trail.iter().rev().find(|&&(t, _)| t_ms - t >= ms);
                then.is_some_and(|&(_, p)| (p - active.at).length() > by)
                    && moved(active, hand).length() > by
            }
            Guard::Flicked(direction, by, ms) => {
                let now = to_screen(hand.in_view);
                active.trail.iter().filter(|&&(t, _)| t_ms - t <= ms).any(|&(_, p)| {
                    let moved = now - p;
                    let (along, across) = match direction {
                        Direction::Left => (-moved.x, moved.y),
                        Direction::Right => (moved.x, moved.y),
                        Direction::Up => (-moved.y, moved.x),
                        Direction::Down => (moved.y, moved.x),
                    };
                    along > by && along > across.abs()
                })
            }
        }
    }

    /// Moves the gesture under way into state `to`: releases what it no
    /// longer holds, presses what it newly holds, then does its entry
    /// actions. A state with no way out ends the gesture.
    fn enter(&mut self, to: &str, t_ms: u64) -> Vec<Action> {
        let Some(active) = &mut self.active else {
            return Vec::new();
        };
        // ponytail: a state the file doesn't define ends the gesture; the
        // checker (#20) will reject such files outright.
        let next = self.gestures.gestures[active.gesture]
            .states
            .get(to)
            .cloned()
            .unwrap_or_default();
        let held = &active.state.holds;
        let releases = held.iter().rev().filter(|b| !next.holds.contains(b));
        let presses = next.holds.iter().filter(|b| !held.contains(b));
        let actions = releases
            .map(|&b| Action::Release(b))
            .chain(presses.map(|&b| Action::Press(b)))
            .chain(next.enter.iter().copied())
            .collect();
        let ends = next.on.is_empty();
        active.state = next;
        active.entered_ms = t_ms;
        if ends {
            self.active = None;
        }
        actions
    }

    /// Hands control to a hand held open in the Reach long enough.
    fn raise(&mut self, hands: &[Hand], t_ms: u64) {
        let raised = |h: &&Hand| h.pose == Pose::Open && in_reach(h.in_view);
        if let Control::Raising { id, since } = self.control
            && hands.iter().filter(raised).any(|h| h.id == id)
        {
            if t_ms.saturating_sub(since) >= ENGAGE_MS {
                self.control = Control::Held { id };
                self.last_seen_ms = t_ms;
                self.outside_since = None;
                self.pose = (Pose::Open, t_ms);
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
}

/// How far the pointer has moved since the gesture started (screen
/// widths, x right, y down).
fn moved(active: &Active, hand: &Hand) -> Vec2 {
    to_screen(hand.in_view) - active.at
}

fn in_reach(in_view: Vec2) -> bool {
    in_view.cmpge(REACH_MIN).all() && in_view.cmple(REACH_MAX).all()
}

/// Where a palm at `in_view` points on the screen: across the Reach,
/// clamped to the screen.
fn to_screen(in_view: Vec2) -> Vec2 {
    ((in_view - REACH_MIN) / (REACH_MAX - REACH_MIN)).clamp(Vec2::ZERO, Vec2::ONE)
}
