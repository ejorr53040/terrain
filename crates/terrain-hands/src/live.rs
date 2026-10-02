//! Live hand tracking: its own thread takes each new camera frame through
//! the `HandTracker`, so slow inference never holds up capture or
//! rendering. It feeds the app two ways: the latest frame with its hands
//! (for the preview), and a `HandSource` of mirrored `HandFrame`s (for
//! interaction).

use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use crate::{
    HandFrame, HandSource, HandTracker, TrackedHand,
    capture::{Camera, CameraFrame},
};

/// How long the tracker waits for a frame before checking it's still wanted.
const POLL: Duration = Duration::from_millis(250);

/// A camera frame and the hands tracked in it.
pub struct TrackedFrame {
    pub frame: CameraFrame,
    /// In `frame`'s (unmirrored) conventions.
    pub hands: Vec<TrackedHand>,
    /// How long tracking took.
    pub track_time: Duration,
}

#[derive(Default)]
/// What the tracking thread hands over. Each slot holds only the newest
/// value: a reader that falls behind skips stale frames rather than queuing.
struct Shared {
    /// For the preview.
    latest: Option<TrackedFrame>,
    /// For the `HandSource`, mirrored.
    hands: Option<HandFrame>,
    /// Why tracking stopped, once it has.
    failed: Option<String>,
}

/// Tracks hands in a camera's frames on a background thread. Dropping it
/// stops tracking and the camera.
pub struct LiveTracker {
    shared: Arc<Mutex<Shared>>,
    running: Arc<AtomicBool>,
}

impl LiveTracker {
    /// Starts tracking `camera`'s frames with `tracker`. Fails only if the
    /// thread can't be started.
    pub fn spawn(camera: Camera, mut tracker: HandTracker) -> io::Result<Self> {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let running = Arc::new(AtomicBool::new(true));
        let (out, keep_going) = (shared.clone(), running.clone());
        let started = Instant::now();
        thread::Builder::new()
            .name("hand-tracker".into())
            .spawn(move || {
                while keep_going.load(Ordering::Relaxed) {
                    if let Some(failure) = camera.failure() {
                        out.lock().unwrap().failed = Some(failure);
                        return;
                    }
                    let Some(frame) = camera.wait_next(POLL) else {
                        continue;
                    };
                    let start = Instant::now();
                    let hands = match tracker.track(frame.image()) {
                        Ok(hands) => hands,
                        Err(e) => {
                            out.lock().unwrap().failed = Some(format!("hand tracking failed: {e}"));
                            return;
                        }
                    };
                    let hand_frame = HandFrame {
                        t_ms: frame
                            .captured
                            .saturating_duration_since(started)
                            .as_millis() as u64,
                        hands: hands.iter().map(|t| t.hand.mirrored()).collect(),
                    };
                    let mut out = out.lock().unwrap();
                    out.hands = Some(hand_frame);
                    out.latest = Some(TrackedFrame {
                        hands,
                        track_time: start.elapsed(),
                        frame,
                    });
                }
            })?;
        Ok(Self { shared, running })
    }

    /// The newest tracked frame since the last call, if any. Never blocks.
    pub fn latest(&self) -> Option<TrackedFrame> {
        self.shared.lock().unwrap().latest.take()
    }

    /// Why tracking stopped, if it has.
    pub fn failure(&self) -> Option<String> {
        self.shared.lock().unwrap().failed.clone()
    }

    /// The tracked hands as a `HandSource`, in the mirrored (selfie) view.
    /// It goes quiet once this tracker is dropped.
    pub fn hand_source(&self) -> LiveHands {
        LiveHands {
            shared: self.shared.clone(),
        }
    }
}

impl Drop for LiveTracker {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// Live tracked hands, mirrored, as a `HandSource`.
pub struct LiveHands {
    shared: Arc<Mutex<Shared>>,
}

impl HandSource for LiveHands {
    fn next_frame(&mut self, _now: Duration) -> Option<HandFrame> {
        self.shared.lock().unwrap().hands.take()
    }
}
