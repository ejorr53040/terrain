//! `HeadTracker` against a fake airpods-tui daemon: a Unix socket in a temp runtime dir that
//! speaks the daemon's framing (u32 big-endian length, then JSON).

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use glam::{Mat3, Quat, Vec3};
use serde_json::{Value, json};
use terrain_head::{HeadLook, HeadLookSettings, HeadTracker, OffReason};

const MAC: &str = "08:FF:44:00:00:01";

/// A fake daemon listening where the tracker is told the daemon is.
struct Daemon {
    _dir: tempfile::TempDir,
    socket: PathBuf,
    listener: UnixListener,
}

impl Daemon {
    fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("airpods-tui.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        Self {
            _dir: dir,
            socket,
            listener,
        }
    }

    fn tracker(&self) -> HeadTracker {
        HeadTracker::with_socket(HeadLookSettings { dead_zone_deg: 0.0 }, self.socket.clone())
    }

    /// The next client to connect.
    fn accept(&self) -> Client {
        let (stream, _) = self.listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        Client(stream)
    }
}

/// The daemon's end of one client connection.
struct Client(UnixStream);

impl Client {
    fn send(&mut self, event: Value) {
        let body = serde_json::to_vec(&event).unwrap();
        self.0
            .write_all(&(body.len() as u32).to_be_bytes())
            .unwrap();
        self.0.write_all(&body).unwrap();
    }

    fn receive(&mut self) -> Value {
        let mut len = [0; 4];
        self.0.read_exact(&mut len).unwrap();
        let mut body = vec![0; u32::from_be_bytes(len) as usize];
        self.0.read_exact(&mut body).unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    /// What the daemon replays to every new client before anything else.
    fn send_snapshot(&mut self) {
        self.send(
            json!({"DeviceConnected": {"mac": MAC, "name": "AirPods Max", "product_id": 0x200a}}),
        );
        self.send(json!({"AACPEvent": [MAC, {"BatteryInfo": [{"component": "Headphone", "level": 80, "status": "Discharging"}]}]}));
        self.send(json!("AudioUnavailable"));
    }

    fn send_sample(&mut self, at_ms: u64, q: Quat) {
        self.send(json!({"AACPEvent": [MAC, {"HeadMotion": {
            "timestamp_ns": at_ms * 1_000_000,
            "quaternion": [q.w, q.x, q.y, q.z],
        }}]}));
    }

    /// The junk identity first sample, then the head held straight ahead for 0.6 s at 50 Hz.
    fn send_settling(&mut self) {
        self.send_sample(0, Quat::IDENTITY);
        for i in 1..=30 {
            self.send_sample(i * 20, worn());
        }
    }
}

/// The Max worn facing world +Y: body +X toward the right ear, +Y up, -Z forward.
fn worn() -> Quat {
    Quat::from_mat3(&Mat3::from_cols(Vec3::X, Vec3::Z, -Vec3::Y))
}

fn switch_on() -> Value {
    json!(["*", {"HeadMotion": true}])
}

fn switch_off() -> Value {
    json!(["*", {"HeadMotion": false}])
}

/// Polls `look()` until `done` holds, at most 2 s.
fn look_until(tracker: &mut HeadTracker, done: impl Fn(HeadLook) -> bool) -> HeadLook {
    let start = Instant::now();
    loop {
        let look = tracker.look();
        if done(look) || start.elapsed() > Duration::from_secs(2) {
            return look;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Switched on against `daemon`, the snapshot replayed and straight ahead taken.
fn on(daemon: &Daemon) -> (HeadTracker, Client) {
    let mut tracker = daemon.tracker();
    tracker.switch_on();
    let mut client = daemon.accept();
    assert_eq!(client.receive(), switch_on());
    assert_eq!(tracker.look(), HeadLook::Starting);
    client.send_snapshot();
    client.send_settling();
    let look = look_until(&mut tracker, |l| matches!(l, HeadLook::On { .. }));
    assert_eq!(
        look,
        HeadLook::On {
            yaw_deg: 0.0,
            pitch_deg: 0.0
        }
    );
    (tracker, client)
}

#[test]
fn switching_on_asks_the_daemon_for_motion_and_samples_turn_into_head_look() {
    let daemon = Daemon::start();
    let (mut tracker, mut client) = on(&daemon);
    client.send_sample(620, Quat::from_rotation_z(30f32.to_radians()) * worn());
    let look = look_until(&mut tracker, |l| {
        l != HeadLook::On {
            yaw_deg: 0.0,
            pitch_deg: 0.0,
        }
    });
    let HeadLook::On { yaw_deg, pitch_deg } = look else {
        panic!("expected On, got {look:?}");
    };
    assert!((yaw_deg - 30.0).abs() < 0.5, "yaw {yaw_deg}");
    assert!(pitch_deg.abs() < 0.5, "pitch {pitch_deg}");
}

#[test]
fn the_airpods_coming_off_the_head_turn_it_off_and_motion_is_no_longer_wanted() {
    let daemon = Daemon::start();
    let (mut tracker, mut client) = on(&daemon);
    client.send(json!({"AACPEvent": [MAC, {"HeadMotionEnded": "OffHead"}]}));
    assert_eq!(
        look_until(&mut tracker, |l| matches!(l, HeadLook::Off(_))),
        HeadLook::Off(OffReason::OffHead)
    );
    assert_eq!(client.receive(), switch_off());
}

#[test]
fn the_airpods_disconnecting_turns_it_off() {
    let daemon = Daemon::start();
    let (mut tracker, mut client) = on(&daemon);
    client.send(json!({"DeviceDisconnected": "AA:BB:CC:00:00:02"}));
    client.send_sample(620, Quat::from_rotation_z(30f32.to_radians()) * worn());
    let look = look_until(&mut tracker, |l| {
        l != HeadLook::On {
            yaw_deg: 0.0,
            pitch_deg: 0.0,
        }
    });
    assert!(
        matches!(look, HeadLook::On { yaw_deg, .. } if yaw_deg > 29.0),
        "other AirPods disconnecting left it {look:?}"
    );
    client.send(json!({"DeviceDisconnected": MAC}));
    assert_eq!(
        look_until(&mut tracker, |l| matches!(l, HeadLook::Off(_))),
        HeadLook::Off(OffReason::Disconnected)
    );
    assert_eq!(client.receive(), switch_off());
}

#[test]
fn switching_off_tells_the_daemon() {
    let daemon = Daemon::start();
    let (mut tracker, mut client) = on(&daemon);
    tracker.switch_off();
    assert_eq!(tracker.look(), HeadLook::Off(OffReason::SwitchedOff));
    assert_eq!(client.receive(), switch_off());

    tracker.toggle();
    assert_eq!(tracker.look(), HeadLook::Starting);
    assert_eq!(client.receive(), switch_on(), "the same connection again");
}

#[test]
fn the_daemon_going_away_turns_it_off_and_switching_on_reconnects() {
    let daemon = Daemon::start();
    let (mut tracker, client) = on(&daemon);
    drop(client);
    assert_eq!(
        look_until(&mut tracker, |l| matches!(l, HeadLook::Off(_))),
        HeadLook::Off(OffReason::NoDaemon)
    );
    tracker.switch_on();
    assert_eq!(tracker.look(), HeadLook::Starting);
    assert_eq!(daemon.accept().receive(), switch_on());
}

#[test]
fn with_no_daemon_switching_on_is_off_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut tracker = HeadTracker::with_socket(
        HeadLookSettings::default(),
        dir.path().join("airpods-tui.sock"),
    );
    assert_eq!(tracker.look(), HeadLook::Off(OffReason::SwitchedOff));
    let start = Instant::now();
    tracker.switch_on();
    assert_eq!(tracker.look(), HeadLook::Off(OffReason::NoDaemon));
    assert!(start.elapsed() < Duration::from_millis(100));
}
