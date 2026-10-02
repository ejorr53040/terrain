use std::{collections::VecDeque, io, path::Path};

use crate::HandFrame;

/// Where hand frames come from: the live webcam tracker, or recorded data.
pub trait HandSource: Send {
    /// The next frame, if one is ready. Never blocks.
    fn next_frame(&mut self) -> Option<HandFrame>;
}

/// Plays back recorded frames, one per call.
pub struct ReplaySource {
    frames: VecDeque<HandFrame>,
    recording: Vec<HandFrame>,
    looping: bool,
}

impl ReplaySource {
    pub fn new(frames: Vec<HandFrame>) -> Self {
        Self {
            frames: frames.clone().into(),
            recording: frames,
            looping: false,
        }
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
}

impl HandSource for ReplaySource {
    fn next_frame(&mut self) -> Option<HandFrame> {
        if self.frames.is_empty() && self.looping {
            self.frames = self.recording.clone().into();
        }
        self.frames.pop_front()
    }
}
