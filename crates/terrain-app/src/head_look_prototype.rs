//! PROTOTYPE, throwaway (branch prototype/head-look); the real design is terrain#93.
//!
//! Orbits the scene camera from head yaw/pitch that `tools/PROTOTYPE_head_bridge.py` sends
//! over UDP 127.0.0.1:4243 (two little-endian f32 degrees: +yaw = head left, +pitch = down).
//! `H` switches Head look on or off, `Shift+H` recenters. Off, stale or no bridge: the camera
//! glides back to the default view.

use std::net::UdpSocket;

use bevy::prelude::*;
use terrain_app::SCENE_CAMERA_AT;

const YAW_GAIN: f32 = 6.0;
const PITCH_GAIN: f32 = 3.0;
const DEAD_ZONE_DEG: f32 = 2.0;
/// Fraction of the way to the target covered per second-ish (exponential glide).
const GLIDE: f32 = 8.0;

#[derive(Resource)]
struct HeadLook {
    socket: Option<UdpSocket>,
    on: bool,
    /// Latest head angles from the bridge, degrees, and when they arrived.
    latest: Option<(f32, f32, f64)>,
    /// Head angles counted as straight ahead.
    center: (f32, f32),
    /// Current orbit offsets in radians, eased toward the target.
    azimuth: f32,
    elevation: f32,
}

pub struct HeadLookPrototypePlugin;

impl Plugin for HeadLookPrototypePlugin {
    fn build(&self, app: &mut App) {
        let socket = UdpSocket::bind("127.0.0.1:4243")
            .and_then(|s| s.set_nonblocking(true).map(|_| s))
            .map_err(|e| warn!("head look prototype: no UDP socket: {e}"))
            .ok();
        app.insert_resource(HeadLook {
            socket,
            on: false,
            latest: None,
            center: (0.0, 0.0),
            azimuth: 0.0,
            elevation: 0.0,
        })
        .add_systems(Startup, spawn_status)
        .add_systems(Update, (read_bridge, keys, orbit, show_status).chain());
    }
}

#[derive(Component)]
struct HeadStatus;

fn spawn_status(mut commands: Commands) {
    commands.spawn((
        HeadStatus,
        Text::new(""),
        TextFont::from_font_size(18.0),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            right: px(12),
            ..default()
        },
    ));
}

fn show_status(
    head: Res<HeadLook>,
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut text: Query<&mut Text, With<HeadStatus>>,
) {
    let now = time.elapsed_secs_f64();
    let bridge = match head.latest {
        Some((y, p, at)) if now - at < 0.5 => format!("bridge live  yaw {y:+5.1}  pitch {p:+5.1}"),
        Some(_) => "bridge stalled".into(),
        None => "no bridge".into(),
    };
    let pressed: Vec<_> = keys.get_pressed().map(|k| format!("{k:?}")).collect();
    let line = format!(
        "PROTOTYPE head look {}  (H on/off, Shift+H recenter)\n{bridge}\norbit {:+.0}°  height {:+.0}°  keys {pressed:?}",
        if head.on { "ON" } else { "off" },
        head.azimuth.to_degrees(),
        head.elevation.to_degrees(),
    );
    for mut t in &mut text {
        t.0 = line.clone();
    }
}

fn read_bridge(mut head: ResMut<HeadLook>, time: Res<Time<Real>>) {
    let Some(socket) = head.socket.as_ref() else { return };
    let mut buf = [0u8; 8];
    let mut got = None;
    while let Ok(8) = socket.recv(&mut buf) {
        let yaw = f32::from_le_bytes(buf[0..4].try_into().unwrap());
        let pitch = f32::from_le_bytes(buf[4..8].try_into().unwrap());
        got = Some((yaw, pitch));
    }
    if let Some((yaw, pitch)) = got {
        head.latest = Some((yaw, pitch, time.elapsed_secs_f64()));
    }
}

fn keys(keys: Res<ButtonInput<KeyCode>>, mut head: ResMut<HeadLook>) {
    if !keys.just_pressed(KeyCode::KeyH) {
        return;
    }
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let Some((yaw, pitch, _)) = head.latest else {
        warn!("head look prototype: no samples from the bridge");
        return;
    };
    if shift {
        head.center = (yaw, pitch);
        info!("head look: recentered");
    } else {
        head.on = !head.on;
        head.center = (yaw, pitch);
        info!("head look: {}", if head.on { "on" } else { "off" });
    }
}

fn dead_zone(deg: f32) -> f32 {
    deg.signum() * (deg.abs() - DEAD_ZONE_DEG).max(0.0)
}

fn orbit(
    mut head: ResMut<HeadLook>,
    time: Res<Time<Real>>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    let now = time.elapsed_secs_f64();
    let fresh = head.latest.filter(|&(_, _, at)| now - at < 0.5);
    if head.on && fresh.is_none() {
        head.on = false;
        info!("head look: off (no samples)");
    }
    let (target_az, target_el) = match fresh {
        Some((yaw, pitch, _)) if head.on => (
            -dead_zone(yaw - head.center.0).to_radians() * YAW_GAIN,
            dead_zone(pitch - head.center.1).to_radians() * PITCH_GAIN,
        ),
        _ => (0.0, 0.0),
    };
    let k = 1.0 - (-GLIDE * time.delta_secs()).exp();
    head.azimuth += (target_az - head.azimuth) * k;
    head.elevation += (target_el - head.elevation) * k;

    let radius = SCENE_CAMERA_AT.length();
    let base_el = (SCENE_CAMERA_AT.y / radius).asin();
    let el = (base_el + head.elevation).clamp(0.0, 89f32.to_radians());
    let az = head.azimuth;
    let at = radius * Vec3::new(az.sin() * el.cos(), el.sin(), az.cos() * el.cos());
    for mut t in &mut camera {
        *t = Transform::from_translation(at).looking_at(Vec3::ZERO, Vec3::Y);
    }
}
