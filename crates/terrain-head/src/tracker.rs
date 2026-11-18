//! `HeadTracker`: Head look fed by the airpods-tui daemon over its IPC socket.

use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use serde::{Deserialize, Serialize};

use crate::look::{HeadMotionSample, Look};
use crate::{HeadLook, HeadLookSettings, OffReason};

/// The daemon refuses larger messages; so do we.
const MAX_MESSAGE: usize = 16 * 1024 * 1024;

/// Owns the connection to the airpods-tui daemon on a background thread.
pub struct HeadTracker {
    /// Where the daemon listens; `None` when `$XDG_RUNTIME_DIR` is unset.
    socket: Option<PathBuf>,
    look: Look,
    daemon: Option<Daemon>,
    /// The AirPods whose motion is being followed: the first to send a sample since switching on.
    motion_mac: Option<String>,
}

impl HeadTracker {
    /// Follows the daemon at `$XDG_RUNTIME_DIR/airpods-tui.sock`. Does not connect until
    /// switched on.
    pub fn new(settings: HeadLookSettings) -> Self {
        let socket = std::env::var_os("XDG_RUNTIME_DIR")
            .map(|dir| PathBuf::from(dir).join("airpods-tui.sock"));
        Self::at(settings, socket)
    }

    /// Follows a daemon listening at `socket` instead of the default one.
    pub fn with_socket(settings: HeadLookSettings, socket: PathBuf) -> Self {
        Self::at(settings, Some(socket))
    }

    fn at(settings: HeadLookSettings, socket: Option<PathBuf>) -> Self {
        Self {
            socket,
            look: Look::new(settings),
            daemon: None,
            motion_mac: None,
        }
    }

    /// Connects if needed and asks the daemon for motion: Starting, or Off(NoDaemon) at once
    /// if there is no daemon. Straight ahead is the orientation about 0.5 s in.
    pub fn switch_on(&mut self) {
        self.drain();
        if self.daemon.is_none() {
            self.daemon = self.socket.as_ref().and_then(|s| Daemon::connect(s).ok());
        }
        self.motion_mac = None;
        self.look.switch_on();
        if !self.send(Command::HeadMotion(true)) {
            self.look.end(OffReason::NoDaemon);
        }
    }

    /// Tells the daemon motion is no longer wanted: Off(SwitchedOff).
    pub fn switch_off(&mut self) {
        self.drain();
        self.send(Command::HeadMotion(false));
        self.look.switch_off();
    }

    /// Switches on when Off, off otherwise.
    pub fn toggle(&mut self) {
        match self.look() {
            HeadLook::Off(_) => self.switch_on(),
            HeadLook::Starting | HeadLook::On { .. } => self.switch_off(),
        }
    }

    /// Straight ahead becomes the current orientation. Nothing unless On.
    pub fn recenter(&mut self) {
        self.drain();
        self.look.recenter();
    }

    pub fn set_settings(&mut self, settings: HeadLookSettings) {
        self.look.set_settings(settings);
    }

    /// Head look now, after everything the daemon has sent so far. Never blocks.
    pub fn look(&mut self) -> HeadLook {
        self.drain();
        self.look.get()
    }

    /// Applies what the daemon sent since last time.
    fn drain(&mut self) {
        while let Some(daemon) = &self.daemon {
            match daemon.events.try_recv() {
                Ok(event) => self.apply(event),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.daemon = None;
                    self.look.end(OffReason::NoDaemon);
                }
            }
        }
    }

    fn apply(&mut self, event: DaemonEvent) {
        match event {
            DaemonEvent::AACPEvent(mac, AacpEvent::HeadMotion(sample)) => {
                if *self.motion_mac.get_or_insert_with(|| mac.clone()) == mac {
                    self.look.sample(sample);
                }
            }
            DaemonEvent::AACPEvent(mac, AacpEvent::HeadMotionEnded(end)) => {
                self.motion_ended(&mac, end.into());
            }
            DaemonEvent::DeviceDisconnected(mac) => {
                self.motion_ended(&mac, OffReason::Disconnected);
            }
        }
    }

    /// Motion from `mac` stopped: Off with `reason` if it was the AirPods being followed (or
    /// none had sent a sample yet), and no more motion wanted.
    fn motion_ended(&mut self, mac: &str, reason: OffReason) {
        if self.motion_mac.as_deref().is_none_or(|m| m == mac) && self.look.end(reason) {
            self.send(Command::HeadMotion(false));
        }
    }

    /// Sends `command` to the daemon; false if there is none or the connection broke.
    fn send(&mut self, command: Command) -> bool {
        let Some(daemon) = &mut self.daemon else {
            return false;
        };
        let sent = daemon.send(&command).is_ok();
        if !sent {
            self.daemon = None;
        }
        sent
    }
}

/// One connection to the daemon. Its reader thread ends when the connection does.
struct Daemon {
    stream: UnixStream,
    events: Receiver<DaemonEvent>,
}

impl Daemon {
    fn connect(socket: &PathBuf) -> io::Result<Self> {
        let stream = UnixStream::connect(socket)?;
        let reader = stream.try_clone()?;
        let (sender, events) = mpsc::channel();
        thread::Builder::new()
            .name("terrain-head daemon".into())
            .spawn(move || read_events(reader, sender))?;
        Ok(Self { stream, events })
    }

    /// One message: u32 big-endian length, then the JSON of `(mac, command)`. The daemon
    /// ignores the mac of a HeadMotion command; "*" by convention.
    fn send(&mut self, command: &Command) -> io::Result<()> {
        let body = serde_json::to_vec(&("*", command))?;
        let mut message = Vec::with_capacity(4 + body.len());
        message.extend_from_slice(&(body.len() as u32).to_be_bytes());
        message.extend_from_slice(&body);
        self.stream.write_all(&message)
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        // Wakes the reader thread so it ends; the daemon drops this client's motion demand.
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

/// Passes on the daemon's messages that matter to Head look until the connection ends,
/// skipping everything else (the snapshot replay, battery, ear detection, ...).
fn read_events(mut stream: UnixStream, events: Sender<DaemonEvent>) {
    while let Ok(message) = read_message(&mut stream) {
        if let Ok(event) = serde_json::from_slice(&message)
            && events.send(event).is_err()
        {
            return;
        }
    }
}

fn read_message(stream: &mut UnixStream) -> io::Result<Vec<u8>> {
    let mut len = [0; 4];
    stream.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message too large",
        ));
    }
    let mut message = vec![0; len];
    stream.read_exact(&mut message)?;
    Ok(message)
}

/// The client → daemon `DeviceCommand`s Head look sends.
#[derive(Serialize)]
enum Command {
    HeadMotion(bool),
}

/// The daemon → client `AppEvent`s Head look reads; any other fails to parse and is skipped.
#[derive(Deserialize)]
enum DaemonEvent {
    DeviceDisconnected(String),
    #[allow(clippy::upper_case_acronyms)]
    AACPEvent(String, AacpEvent),
}

#[derive(Deserialize)]
enum AacpEvent {
    HeadMotion(HeadMotionSample),
    HeadMotionEnded(HeadMotionEnd),
}

/// Why the daemon stopped sending motion.
#[derive(Clone, Copy, Deserialize)]
enum HeadMotionEnd {
    OffHead,
    Disconnected,
    Unavailable,
}

impl From<HeadMotionEnd> for OffReason {
    fn from(end: HeadMotionEnd) -> Self {
        match end {
            HeadMotionEnd::OffHead => OffReason::OffHead,
            HeadMotionEnd::Disconnected => OffReason::Disconnected,
            HeadMotionEnd::Unavailable => OffReason::Unavailable,
        }
    }
}
