//! While running, terrain-desktop keeps the webcam at a fixed 30 fps
//! (dynamic framerate off); however it's closed, the camera gets its own
//! settings back (#78). Needs the real webcam, so ignored by default:
//! `cargo test -p terrain-desktop --test it camera -- --ignored`.

use std::{
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const DEVICE: &str = "/dev/video0";

/// The webcam's `exposure_dynamic_framerate`, read with v4l2-ctl.
fn dynamic_framerate() -> u32 {
    let out = Command::new("v4l2-ctl")
        .args(["-d", DEVICE, "-C", "exposure_dynamic_framerate"])
        .output()
        .expect("v4l2-ctl");
    String::from_utf8_lossy(&out.stdout)
        .rsplit(':')
        .next()
        .and_then(|v| v.trim().parse().ok())
        .expect("a value")
}

/// Waits up to 10 s for the setting to read `want`.
fn becomes(want: u32) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if dynamic_framerate() == want {
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    false
}

/// terrain-desktop, killed if a test fails before it's stopped, so it
/// doesn't hold the camera.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start() -> Running {
    assert_eq!(dynamic_framerate(), 1, "the camera's own setting, before");
    let child = Command::new(env!("CARGO_BIN_EXE_terrain-desktop"))
        .args(["--dry-run", "--camera", DEVICE])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let running = Running(child);
    assert!(becomes(0), "dynamic framerate off while running");
    running
}

#[test]
#[ignore = "needs the webcam"]
fn the_camera_gets_its_settings_back_after_a_kill_9() {
    let mut running = start();
    running.0.kill().unwrap(); // SIGKILL: no code of terrain's runs
    running.0.wait().unwrap();
    assert!(becomes(1), "restored after SIGKILL");
}

#[test]
#[ignore = "needs the webcam"]
fn the_camera_gets_its_settings_back_after_ctrl_c() {
    let mut running = start();
    Command::new("kill")
        .args(["-INT", &running.0.id().to_string()])
        .status()
        .unwrap();
    running.0.wait().unwrap();
    assert!(becomes(1), "restored after Ctrl-C");
}
