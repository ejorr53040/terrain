//! Webcam capture: MJPEG frames from a V4L2 device, decoded to RGBA on their
//! own thread. Linux-only; `Camera` is the seam a cross-platform backend
//! would replace.

use std::{
    fmt, io,
    sync::{Arc, Mutex, Weak},
    thread,
    time::{Duration, Instant},
};

use v4l::{
    Device, FourCC, Fraction, Timestamp,
    buffer::Type,
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

impl std::error::Error for CameraError {}

/// What the capture thread hands over.
#[derive(Default)]
struct Shared {
    latest: Option<CameraFrame>,
    /// Set if capture stopped; the thread has exited.
    failed: Option<String>,
}

/// A webcam streaming on a background thread. Dropping it stops capture.
pub struct Camera {
    shared: Arc<Mutex<Shared>>,
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
        let mut stream = MmapStream::with_buffers(&dev, Type::VideoCapture, 4).map_err(open_err)?;
        stream.set_timeout(STALL_TIMEOUT);

        let shared = Arc::new(Mutex::new(Shared::default()));
        let weak = Arc::downgrade(&shared);
        thread::Builder::new()
            .name("camera".into())
            .spawn(move || {
                if let Err(e) = stream_frames(stream, &weak)
                    && let Some(shared) = weak.upgrade()
                {
                    shared.lock().unwrap().failed = Some(e);
                }
            })
            .map_err(open_err)?;
        Ok(Self { shared })
    }

    /// The newest frame since the last call, if any. Never blocks.
    pub fn latest(&self) -> Option<CameraFrame> {
        self.shared.lock().unwrap().latest.take()
    }

    /// Why capture stopped, if it has.
    pub fn failure(&self) -> Option<String> {
        self.shared.lock().unwrap().failed.clone()
    }
}

/// Streams until the `Camera` is dropped (`Ok`) or the device fails or
/// stalls (`Err`).
fn stream_frames(mut stream: MmapStream, shared: &Weak<Mutex<Shared>>) -> Result<(), String> {
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
        let Some(shared) = shared.upgrade() else {
            return Ok(());
        };
        shared.lock().unwrap().latest = Some(frame);
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
