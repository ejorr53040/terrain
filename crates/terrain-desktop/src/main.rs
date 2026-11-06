//! terrain-desktop: drive the desktop with a hand seen by the webcam.
//!
//! Raise a hand, open, palm to the camera, and hold it a moment to take
//! control; lower it back to the keyboard to give control up. The palm
//! points; pinch thumb and index to click and drag; make a fist to drag the
//! window under the pointer (Super + drag).
//!
//! `--camera <device>` picks the webcam (default `/dev/video0`); the camera
//! calibration the 3D app saves is used if there is one. `--probe` skips
//! the camera and sweeps the pointer, to check the desktop takes input;
//! `--dry-run` tracks hands and prints what it would do, touching nothing.
//! Ctrl-C stops, letting go of anything held.
//!
//! What the hands are doing is appended, one line per change, to
//! `$XDG_RUNTIME_DIR/terrain/status` for the bar to show; an empty line
//! means terrain-desktop has stopped.

use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

use glam::Vec2;
use terrain_desktop::{Action, DesktopControl, Status, VirtualDesktop};
use terrain_hands::{CameraModel, HandSource, LiveTracker, capture};

/// How often new hand frames are looked for.
const POLL: Duration = Duration::from_millis(4);
/// With no hand frame for this long, tracking has stalled: let go of
/// everything rather than leave a button held down.
const STALLED: Duration = Duration::from_millis(500);
/// How often a dry run reports where the hand points.
const REPORT_EVERY: Duration = Duration::from_secs(1);

/// Set by Ctrl-C (or SIGTERM): time to stop.
static STOP: AtomicBool = AtomicBool::new(false);

fn main() {
    terrain_hands::capture::watch_if_asked();
    let args: Vec<String> = std::env::args().collect();
    let option = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let mut desktop = (!dry_run).then(|| {
        VirtualDesktop::new().unwrap_or_else(|e| {
            eprintln!(
                "can't create virtual input devices (needs write access to /dev/uinput): {e}"
            );
            std::process::exit(1);
        })
    });
    stop_on_signals();
    if args.iter().any(|a| a == "--probe")
        && let Some(desktop) = &mut desktop
    {
        probe(desktop);
        return;
    }

    let device = option("--camera").unwrap_or_else(|| capture::DEFAULT_DEVICE.into());
    let camera = CameraModel::saved_path()
        .and_then(|path| {
            CameraModel::load(&path).unwrap_or_else(|e| {
                eprintln!("ignoring the calibration in {}: {e}", path.display());
                None
            })
        })
        .unwrap_or_default();
    let tracker = LiveTracker::start(&device).unwrap_or_else(|e| {
        eprintln!("can't start hand tracking on {device}: {e}");
        std::process::exit(1);
    });
    let mut hands = tracker.hand_source();
    let mut control = DesktopControl::new(camera);
    let started = Instant::now();
    let mut had_control = false;
    let mut last_frame = Instant::now();
    let mut last_report = Instant::now();
    let mut status_file = status_file();
    let mut shown = None;
    eprintln!("terrain-desktop: tracking hands on {device}; Ctrl-C to stop");
    while !STOP.load(Ordering::Relaxed) {
        if let Some(failure) = tracker.failure() {
            eprintln!("hand tracking stopped: {failure}");
            break;
        }
        let frame = hands.next_frame(started.elapsed());
        let inputs = match &frame {
            Some(frame) => {
                last_frame = Instant::now();
                control.update(frame)
            }
            None if last_frame.elapsed() > STALLED && control.has_control() => {
                eprintln!("no hand frames for {STALLED:?}: letting go");
                control.let_go()
            }
            None => {
                thread::sleep(POLL);
                continue;
            }
        };
        let status = control.status();
        if shown.as_ref() != Some(&status) {
            show(&mut status_file, label(&status));
            shown = Some(status);
        }
        let t_ms = frame.as_ref().map_or(0, |f| f.t_ms);
        let hand_count = frame.as_ref().map_or(0, |f| f.hands.len());
        if control.has_control() != had_control {
            had_control = control.has_control();
            eprintln!(
                "{t_ms:>7} ms  {}",
                if had_control {
                    "hand took control"
                } else {
                    "control released"
                }
            );
        }
        for input in inputs {
            match input {
                Action::PointTo(at) if dry_run && last_report.elapsed() > REPORT_EVERY => {
                    last_report = Instant::now();
                    eprintln!(
                        "{t_ms:>7} ms  {hand_count} hand(s), pointing at ({:.2}, {:.2})",
                        at.x, at.y
                    )
                }
                Action::PointTo(_) | Action::Scroll(_) => {}
                other => eprintln!("{t_ms:>7} ms  {other:?}"),
            }
            if let Some(desktop) = &mut desktop
                && let Err(e) = desktop.apply(input)
            {
                eprintln!("can't send {input:?}: {e}");
            }
        }
    }
    show(&mut status_file, "");
    // Dropping the virtual devices lets go of anything still held.
}

/// The status file the bar watches, emptied; `None` if it can't be made
/// (the indicator is only a nicety).
fn status_file() -> Option<File> {
    let dir = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join("terrain");
    std::fs::create_dir_all(&dir).ok()?;
    File::create(dir.join("status")).ok()
}

/// Appends `line` to the status file.
fn show(file: &mut Option<File>, line: &str) {
    if let Some(f) = file {
        let _ = writeln!(f, "{line}");
    }
}

/// How the bar shows `status`: a gesture by its gesture-file label.
fn label(status: &Status) -> &str {
    match status {
        Status::Disengaged => "✋ Idle",
        Status::Engaging => "✋ Engaging…",
        Status::Pointing => "✋ Pointing",
        Status::Gesture(label) => label,
    }
}

/// Sweeps the pointer around a circle for a few seconds.
fn probe(desktop: &mut VirtualDesktop) {
    let started = Instant::now();
    while !STOP.load(Ordering::Relaxed) && started.elapsed() < Duration::from_secs(4) {
        let a = started.elapsed().as_secs_f32() * 2.0;
        let at = Vec2::splat(0.5) + 0.3 * Vec2::new(a.cos(), a.sin());
        desktop.apply(Action::PointTo(at)).expect("pointer input");
        thread::sleep(Duration::from_millis(10));
    }
}

/// Sets `STOP` on SIGINT or SIGTERM.
fn stop_on_signals() {
    extern "C" fn on_signal(_: libc::c_int) {
        STOP.store(true, Ordering::Relaxed);
    }
    // SAFETY: the handler only stores to an atomic, which is signal-safe.
    unsafe {
        libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
    }
}
