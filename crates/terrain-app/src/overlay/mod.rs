//! The overlay, a panel down the right: toggled with `O`, or opened by a
//! hand resting on the Hot edge, and worked by a hand cursor as a mouse.

mod panel;

pub use panel::OverlayPlugin;

use std::time::Duration;

use bevy::prelude::*;
use bevy_egui::egui;

use crate::HandCursor;

/// Where the overlay is drawn, as fractions of the window from the top
/// left; a cursor in there isn't pointing into the scene. The overlay sets it.
#[derive(Resource, Default, Debug)]
pub struct OverlayArea(pub Option<Rect>);

/// Whether the overlay is shown. `O` toggles it; a hand resting on the
/// Hot edge opens it, and then it closes once no hand has been over it for
/// `HOT_EDGE_CLOSE`.
#[derive(Resource, Default, Debug)]
pub struct OverlayOpen {
    pub open: bool,
    /// Opened by the Hot edge, so it closes itself.
    by_edge: bool,
}

impl OverlayOpen {
    /// What `O` does: show or hide it, and leave it that way.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.by_edge = false;
    }
}

/// How far across the view the Hot edge starts.
const HOT_EDGE: f32 = 0.97;
/// How long a hand rests on the Hot edge to open the overlay.
const HOT_EDGE_OPEN: Duration = Duration::from_millis(300);
/// How long the overlay stays open with no hand over it.
const HOT_EDGE_CLOSE: Duration = Duration::from_secs(1);

/// Opens the overlay for a hand resting on the Hot edge and closes it
/// once no hand has been over it for a while.
pub(crate) fn hot_edge(
    time: Res<Time>,
    hands: Query<&HandCursor>,
    area: Res<OverlayArea>,
    mut overlay: ResMut<OverlayOpen>,
    mut on_edge_since: Local<Option<Duration>>,
    mut last_over: Local<Duration>,
) {
    let now = time.elapsed();
    let on_edge = hands.iter().any(|c| c.at.x >= HOT_EDGE);
    if !on_edge {
        *on_edge_since = None;
    } else if !overlay.open && now - *on_edge_since.get_or_insert(now) >= HOT_EDGE_OPEN {
        overlay.open = true;
        overlay.by_edge = true;
    }
    if on_edge || hands.iter().any(|c| area.covers(c.at)) {
        *last_over = now;
    }
    if overlay.open && overlay.by_edge && now - *last_over >= HOT_EDGE_CLOSE {
        overlay.open = false;
        overlay.by_edge = false;
    }
}

/// A hand cursor working the overlay like a mouse: where it points (fractions
/// of the window) and whether it's pinched. The overlay feeds it to egui.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct OverlayPointer {
    pub at: Option<Vec2>,
    pub pressed: bool,
    /// The hand that owns it: the first one over the overlay.
    owner: Option<u32>,
    /// Where the pointer and the hand were at the pinch.
    pressed_from: Option<(Vec2, Vec2)>,
}

/// How far the pointer moves for the hand's move while pinched, so a
/// slider doesn't wobble with the hand.
const DRAG_GAIN: f32 = 0.5;
/// How far a pinched hand moves (fraction of the window) before the pointer
/// follows: a real tap drifts about 0.01, and egui reads any move as a drag.
const CLICK_SLOP: f32 = 0.02;

/// Gives the overlay pointer to the first open hand over the overlay and
/// moves it with that hand until the hand leaves (not mid-press).
pub(crate) fn point_at_overlay(
    hands: Query<&HandCursor>,
    area: Res<OverlayArea>,
    mut pointer: ResMut<OverlayPointer>,
) {
    let owner = pointer
        .owner
        .and_then(|id| hands.iter().find(|c| c.id == id));
    let keeps = owner.is_some_and(|c| area.covers(c.at) || pointer.pressed && c.pinching);
    let owner = if keeps {
        owner
    } else {
        // An open hand takes it: one already pinching is carrying something in.
        let mut over: Vec<&HandCursor> = hands
            .iter()
            .filter(|c| !c.pinching && area.covers(c.at))
            .collect();
        over.sort_by_key(|c| c.id);
        over.first().copied()
    };
    let mut next = OverlayPointer::default();
    if let Some(c) = owner {
        next.owner = Some(c.id);
        next.pressed = c.pinching;
        next.pressed_from = match (c.pinching, pointer.pressed_from) {
            (true, Some(from)) if pointer.owner == Some(c.id) => Some(from),
            (true, _) => Some((c.at, c.at)),
            (false, _) => None,
        };
        next.at = Some(match next.pressed_from {
            Some((pointer_at, hand_at)) => {
                let moved = c.at - hand_at;
                let past_slop = (moved.length() - CLICK_SLOP).max(0.0);
                pointer_at + DRAG_GAIN * past_slop * moved.normalize_or_zero()
            }
            None => c.at,
        });
    }
    if *pointer != next {
        *pointer = next;
    }
}

impl OverlayPointer {
    /// The egui mouse events that take the pointer from `last` to here, in a
    /// window `size` (logical pixels).
    pub fn egui_events(&self, last: &OverlayPointer, size: Vec2) -> Vec<egui::Event> {
        let pos = |at: Vec2| {
            let at = at * size;
            egui::pos2(at.x, at.y)
        };
        let button = |at: Vec2, pressed: bool| egui::Event::PointerButton {
            pos: pos(at),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let mut events = Vec::new();
        match (last.at, self.at) {
            (_, Some(at)) => {
                // A release lands where the pointer was, before it moves on,
                // or the hand's last twitch turns the click into a drag.
                if let (true, false, Some(was)) = (last.pressed, self.pressed, last.at) {
                    events.push(button(was, false));
                }
                events.push(egui::Event::PointerMoved(pos(at)));
                if self.pressed && !last.pressed {
                    events.push(button(at, true));
                }
            }
            (Some(at), None) => {
                if last.pressed {
                    events.push(button(at, false));
                }
                events.push(egui::Event::PointerGone);
            }
            (None, None) => {}
        }
        events
    }
}

impl OverlayArea {
    /// Whether a cursor `at` (fractions of the window) is over the overlay.
    pub fn covers(&self, at: Vec2) -> bool {
        self.0.is_some_and(|area| area.contains(at))
    }
}
