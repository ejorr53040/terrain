//! Webcam capture: MJPEG frames from a V4L2 device, decoded to RGBA on their
//! own thread. Linux-only; `Camera` is the seam a cross-platform backend
//! would replace.

use std::{
    fmt,
    io::{self, Read, Write},
    os::unix::process::CommandExt,
    process::{Child, Command, Stdio},
    sync::{Arc, Condvar, Mutex, Weak, mpsc},
    thread,
    time::{Duration, Instant},
};

use crate::RgbaImage;
use v4l::{
    Device, FourCC, Fraction, Timestamp,
    buffer::Type,
    control::{Control, Value},
    io::traits::CaptureStream,
    prelude::MmapStream,
    video::{Capture, capture::Parameters},
};
use zune_jpeg::{
    JpegDecoder,
    zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions},
};

/// Width of the capture mode asked of the camera, in pixels.
pub const WIDTH: u32 = 1280;
/// Height of the capture mode asked of the camera, in pixels.
pub const HEIGHT: u32 = 720;
/// Frame rate asked of the camera.
pub const FPS: u32 = 30;

const MJPG: FourCC = FourCC { repr: *b"MJPG" };

/// V4L2's exposure_dynamic_framerate: auto exposure may drop the frame
/// rate in dim light, exposing longer and blurring fast hands. terrain turns
/// it off while the camera is open (#78).
const DYNAMIC_FRAMERATE: u32 = 0x009a_0903;
/// Set (to anything) to leave the camera's settings alone.
const KEEP_SETTINGS_ENV: &str = "TERRAIN_CAMERA_KEEP_SETTINGS";
/// Starts this binary as the watchdog that gives the camera its setting back.
const WATCH_ARG: &str = "--restore-camera-setting";
/// What the watchdog says once it's watching, before terrain changes anything.
const READY: &[u8] = b"terrain camera watchdog ready\n";
/// How long terrain waits to hear it.
const READY_TIMEOUT: Duration = Duration::from_secs(2);

/// A camera that sends nothing for this long is treated as failed.
const STALL_TIMEOUT: Duration = Duration::from_secs(2);

/// One decoded webcam image, as the camera saw it: not mirrored. The
/// tracker works on this; mirroring is for display and landmarks only.
pub struct CameraFrame {
    /// When the camera driver stamped the frame, or failing that, when it
    /// was taken off the device (before decoding).
    pub captured: Instant,
    /// Counts frames delivered by the camera, including any dropped here.
    pub sequence: u64,
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA, 8 bits per channel.
    pub rgba: Vec<u8>,
}

/// Why a camera couldn't be started.
#[derive(Debug)]
pub enum CameraError {
    /// The device couldn't be opened or configured.
    Open { device: String, source: io::Error },
    /// The device won't stream MJPEG at the capture mode.
    Unsupported { device: String, got: String },
}

impl fmt::Display for CameraError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { device, source } => write!(f, "can't open camera {device}: {source}"),
            Self::Unsupported { device, got } => write!(
                f,
                "camera {device} won't stream MJPEG {WIDTH}x{HEIGHT}; it offered {got}"
            ),
        }
    }
}

impl CameraFrame {
    /// The frame's pixels, borrowed.
    pub fn image(&self) -> RgbaImage<'_> {
        RgbaImage {
            width: self.width,
            height: self.height,
            pixels: &self.rgba,
        }
    }
}

impl std::error::Error for CameraError {}

/// What the capture thread hands over.
#[derive(Default)]
struct Shared {
    latest: Option<CameraFrame>,
    /// Set if capture stopped; the thread has exited.
    failed: Option<String>,
}

/// The hand-over slot, and a signal for whoever waits on it.
#[derive(Default)]
struct Slot {
    shared: Mutex<Shared>,
    changed: Condvar,
}

/// A webcam streaming on a background thread. Dropping it stops capture.
pub struct Camera {
    slot: Arc<Slot>,
    _watchdog: Option<Watchdog>,
}

/// A child process that gives the camera its own setting back once terrain
/// is done with it: its pipe from terrain closes however terrain ends, a
/// SIGKILL or a crash included.
struct Watchdog(Child);

impl Drop for Watchdog {
    fn drop(&mut self) {
        // Closing the pipe tells it; waiting means a camera opened again
        // straight after reads the camera's own setting, not ours.
        drop(self.0.stdin.take());
        let _ = self.0.wait();
    }
}

impl Camera {
    /// Opens `device` (e.g. `/dev/video0`) and starts capturing. Fails
    /// straight away if the device is missing or can't stream MJPEG.
    pub fn open(device: &str) -> Result<Self, CameraError> {
        let open_err = |source| CameraError::Open {
            device: device.to_string(),
            source,
        };
        let dev = Device::with_path(device).map_err(open_err)?;
        let mut format = dev.format().map_err(open_err)?;
        format.width = WIDTH;
        format.height = HEIGHT;
        format.fourcc = MJPG;
        let format = dev.set_format(&format).map_err(open_err)?;
        if format.fourcc != MJPG || format.width != WIDTH || format.height != HEIGHT {
            return Err(CameraError::Unsupported {
                device: device.to_string(),
                got: format!("{} {}x{}", format.fourcc, format.width, format.height),
            });
        }
        dev.set_params(&Parameters::new(Fraction::new(1, FPS)))
            .map_err(open_err)?;
        let watchdog = steady_frame_rate(&dev, device);
        let mut stream = MmapStream::with_buffers(&dev, Type::VideoCapture, 4).map_err(open_err)?;
        stream.set_timeout(STALL_TIMEOUT);

        let slot = Arc::new(Slot::default());
        let weak = Arc::downgrade(&slot);
        thread::Builder::new()
            .name("camera".into())
            .spawn(move || {
                if let Err(e) = stream_frames(stream, &weak)
                    && let Some(slot) = weak.upgrade()
                {
                    slot.shared.lock().unwrap().failed = Some(e);
                    slot.changed.notify_all();
                }
            })
            .map_err(open_err)?;
        Ok(Self {
            slot,
            _watchdog: watchdog,
        })
    }

    /// The newest frame since the last call, if any. Never blocks.
    pub fn latest(&self) -> Option<CameraFrame> {
        self.slot.shared.lock().unwrap().latest.take()
    }

    /// The next new frame, waiting up to `timeout` for one. `None` on
    /// timeout or once capture has stopped.
    pub fn wait_next(&self, timeout: Duration) -> Option<CameraFrame> {
        let shared = self.slot.shared.lock().unwrap();
        let (mut shared, _) = self
            .slot
            .changed
            .wait_timeout_while(shared, timeout, |s| {
                s.latest.is_none() && s.failed.is_none()
            })
            .unwrap();
        shared.latest.take()
    }

    /// Why capture stopped, if it has.
    pub fn failure(&self) -> Option<String> {
        self.slot.shared.lock().unwrap().failed.clone()
    }
}

/// Turns dynamic framerate off, with a watchdog to turn it back on. Does
/// nothing if it's off already, the camera has no such control, or
/// `KEEP_SETTINGS_ENV` is set. If the watchdog can't start, it says so and
/// leaves the camera alone: tracking matters more than the frame rate.
fn steady_frame_rate(dev: &Device, device: &str) -> Option<Watchdog> {
    if std::env::var_os(KEEP_SETTINGS_ENV).is_some() {
        return None;
    }
    let Ok(Control {
        value: Value::Boolean(true),
        ..
    }) = dev.control(DYNAMIC_FRAMERATE)
    else {
        return None;
    };
    let watchdog = match start_watchdog(device) {
        Ok(watchdog) => watchdog,
        Err(e) => {
            eprintln!("camera {device}: leaving dynamic framerate on, with no watchdog: {e}");
            return None;
        }
    };
    if let Err(e) = set_dynamic_framerate(dev, false) {
        eprintln!("camera {device}: can't turn dynamic framerate off: {e}");
        return None;
    }
    Some(watchdog)
}

/// Starts this binary as the watchdog, and waits for it to say it's
/// watching before terrain changes anything.
fn start_watchdog(device: &str) -> io::Result<Watchdog> {
    let exe = std::env::current_exe()?;
    let mut child = Command::new(&exe)
        .args([WATCH_ARG, device])
        // Should it run as anything but the watchdog, it mustn't start one
        // of its own.
        .env(KEEP_SETTINGS_ENV, "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // Its own process group, so Ctrl-C at the terminal doesn't reach it.
        .process_group(0)
        .spawn()?;
    let mut says = child.stdout.take().expect("piped");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut got = vec![0; READY.len()];
        let _ = tx.send(says.read_exact(&mut got).map(|()| got));
    });
    match rx.recv_timeout(READY_TIMEOUT) {
        Ok(Ok(got)) if got == READY => Ok(Watchdog(child)),
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            Err(io::Error::other(format!(
                "{} didn't start as one (its main must call watch_if_asked)",
                exe.display()
            )))
        }
    }
}

fn set_dynamic_framerate(dev: &Device, on: bool) -> io::Result<()> {
    dev.set_control(Control {
        id: DYNAMIC_FRAMERATE,
        value: Value::Boolean(on),
    })
}

/// Every binary that opens a `Camera` calls this first thing in `main`:
/// started as the watchdog, it waits for terrain to end, turns the camera's
/// dynamic framerate back on, and exits.
pub fn watch_if_asked() {
    let args: Vec<String> = std::env::args().collect();
    let [_, flag, device] = args.as_slice() else {
        return;
    };
    if flag != WATCH_ARG {
        return;
    }
    // pkill, logging out or closing the terminal signal the watchdog too:
    // only the pipe from terrain closing ends the watch.
    // SAFETY: setting a signal to be ignored runs no handler code.
    unsafe {
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            libc::signal(signal, libc::SIG_IGN);
        }
    }
    let mut out = io::stdout();
    if out.write_all(READY).and_then(|()| out.flush()).is_err() {
        std::process::exit(1);
    }
    after_eof(io::stdin(), || {
        if let Err(e) = Device::with_path(device).and_then(|dev| set_dynamic_framerate(&dev, true)) {
            eprintln!("terrain's camera watchdog: can't turn {device}'s dynamic framerate back on: {e}");
        }
    });
    std::process::exit(0);
}

/// Runs `then` once `input` ends: the pipe from terrain closing.
fn after_eof(mut input: impl Read, then: impl FnOnce()) {
    let _ = io::copy(&mut input, &mut io::sink());
    then();
}

/// Streams until the `Camera` is dropped (`Ok`) or the device fails or
/// stalls (`Err`).
fn stream_frames(mut stream: MmapStream, slot: &Weak<Slot>) -> Result<(), String> {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    loop {
        let (jpeg, meta) = stream.next().map_err(|e| match e.kind() {
            io::ErrorKind::TimedOut => format!(
                "no frames for {} s; camera stalled",
                STALL_TIMEOUT.as_secs()
            ),
            _ => format!("capture failed: {e}"),
        })?;
        let captured = driver_instant(meta.timestamp).unwrap_or_else(Instant::now);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(jpeg), options);
        // A corrupt frame now and then is normal for MJPEG webcams; skip it.
        let (Ok(rgba), Some(info)) = (decoder.decode(), decoder.info()) else {
            continue;
        };
        let frame = CameraFrame {
            captured,
            sequence: meta.sequence.into(),
            width: info.width.into(),
            height: info.height.into(),
            rgba,
        };
        let Some(slot) = slot.upgrade() else {
            return Ok(());
        };
        slot.shared.lock().unwrap().latest = Some(frame);
        slot.changed.notify_all();
    }
}

/// The driver's capture stamp as an `Instant`. V4L2 drivers (uvcvideo
/// included) stamp buffers on `CLOCK_MONOTONIC`, the clock `Instant` uses
/// on Linux, so the stamp's age carries over. `None` if the stamp isn't
/// plausibly from that clock.
fn driver_instant(stamp: Timestamp) -> Option<Instant> {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `now` is a valid, writable timespec.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) } != 0 {
        return None;
    }
    let read_at = Instant::now();
    let now = Duration::new(u64::try_from(now.tv_sec).ok()?, now.tv_nsec as u32);
    let stamped = Duration::new(
        u64::try_from(stamp.sec).ok()?,
        u32::try_from(stamp.usec).ok()? * 1000,
    );
    let age = now.checked_sub(stamped)?;
    (age < STALL_TIMEOUT).then(|| read_at - age)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_watchdog_restores_only_once_its_pipe_from_terrain_closes() {
        let (reader, mut writer) = io::pipe().unwrap();
        let (done, restored) = mpsc::channel();
        let watching = thread::spawn(move || after_eof(reader, || done.send(()).unwrap()));
        writer.write_all(b"still here").unwrap();
        assert!(restored.recv_timeout(Duration::from_millis(200)).is_err(), "too soon");
        drop(writer); // terrain ends
        restored.recv_timeout(Duration::from_secs(2)).expect("restored");
        watching.join().unwrap();
    }
}
