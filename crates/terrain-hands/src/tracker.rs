//! The hand tracker: its own thread, taking each new camera frame through
//! detection so slow inference never holds up capture or rendering.

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
    HandRegion, PalmDetector,
    capture::{Camera, CameraFrame},
};

/// How long the tracker waits for a frame before checking it's still wanted.
const POLL: Duration = Duration::from_millis(250);

/// A camera frame and what was found in it.
pub struct TrackedFrame {
    pub frame: CameraFrame,
    /// Hand regions in `frame`'s pixels, best first.
    pub regions: Vec<HandRegion>,
    /// How long detection took.
    pub detect_time: Duration,
}

#[derive(Default)]
struct Shared {
    latest: Option<TrackedFrame>,
    failed: Option<String>,
}

/// Tracks hands in a camera's frames on a background thread. Dropping it
/// stops tracking and the camera.
pub struct HandTracker {
    shared: Arc<Mutex<Shared>>,
    running: Arc<AtomicBool>,
}

impl HandTracker {
    /// Starts tracking `camera`'s frames with `detector`. Fails only if the
    /// thread can't be started.
    pub fn spawn(camera: Camera, mut detector: PalmDetector) -> io::Result<Self> {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let running = Arc::new(AtomicBool::new(true));
        let (out, keep_going) = (shared.clone(), running.clone());
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
                    match detector.detect(frame.image()) {
                        Ok(regions) => {
                            out.lock().unwrap().latest = Some(TrackedFrame {
                                regions,
                                detect_time: start.elapsed(),
                                frame,
                            });
                        }
                        Err(e) => {
                            out.lock().unwrap().failed =
                                Some(format!("palm detection failed: {e}"));
                            return;
                        }
                    }
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
}

impl Drop for HandTracker {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
