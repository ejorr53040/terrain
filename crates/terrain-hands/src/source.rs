use std::{collections::VecDeque, io, path::Path, time::Duration};

use crate::HandFrame;

/// Where hand frames come from: the live webcam tracker, or recorded data.
pub trait HandSource: Send {
    /// The newest frame ready by `now` (the app's clock), if one arrived since
    /// the last call. Never blocks. Live sources can ignore `now`; replay
    /// uses it to play frames at their recorded times.
    fn next_frame(&mut self, now: Duration) -> Option<HandFrame>;
}

/// Plays back recorded frames at their recorded `t_ms` pace.
///
/// Frames come out re-stamped on the replay's own clock, which starts at 0
/// and keeps counting across loops, so time never runs backwards.
pub struct ReplaySource {
    pending: VecDeque<HandFrame>,
    recording: Vec<HandFrame>,
    looping: bool,
    /// App time at which playback began.
    started: Option<Duration>,
    /// Replay-clock time at which the next queued loop starts.
    next_loop_ms: u64,
}

impl ReplaySource {
    pub fn new(frames: Vec<HandFrame>) -> Self {
        let mut source = Self {
            pending: VecDeque::new(),
            recording: frames,
            looping: false,
            started: None,
            next_loop_ms: 0,
        };
        source.queue_loop();
        source
    }

    /// Loads a fixture: a JSON array of `HandFrame`s.
    pub fn from_json_file(path: impl AsRef<Path>) -> io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let frames = serde_json::from_str(&text).map_err(io::Error::other)?;
        Ok(Self::new(frames))
    }

    /// Restart from the first frame after the last one.
    pub fn looping(mut self) -> Self {
        self.looping = true;
        self
    }

    /// Queues one more playthrough, stamped to follow the previous one.
    fn queue_loop(&mut self) {
        let first = self.recording.first().map_or(0, |f| f.t_ms);
        let start = self.next_loop_ms;
        self.pending
            .extend(self.recording.iter().map(|f| HandFrame {
                t_ms: start + f.t_ms.saturating_sub(first),
                ..f.clone()
            }));
        // The first frame follows the last one as any frame follows the one before.
        let length = match self.recording.as_slice() {
            [.., prev, last] => {
                last.t_ms.saturating_sub(first) + last.t_ms.saturating_sub(prev.t_ms)
            }
            _ => 0,
        };
        self.next_loop_ms = start + length.max(1);
    }
}

impl HandSource for ReplaySource {
    fn next_frame(&mut self, now: Duration) -> Option<HandFrame> {
        let started = *self.started.get_or_insert(now);
        let played_ms = (now - started).as_millis() as u64;
        if self.looping && self.pending.back().is_none_or(|f| f.t_ms <= played_ms) {
            self.queue_loop();
        }
        // Hand back the newest due frame, dropping any older ones, like a
        // live tracker that only reports its latest result.
        let mut newest = None;
        while self.pending.front().is_some_and(|f| f.t_ms <= played_ms) {
            newest = self.pending.pop_front();
        }
        newest
    }
}
