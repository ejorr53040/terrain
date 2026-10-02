//! The mirrored webcam preview in a corner of the window, toggled with `P`,
//! with tracked hands (region and skeleton) drawn on it, and a debug
//! overlay with the capture frame rate and tracking stats.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use bevy::math::Vec2;
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use terrain_hands::{
    HandTracker, LANDMARK_MODEL, LiveTracker, PALM_MODEL, TrackedHand,
    capture::{self, Camera, CameraFrame},
};

/// The webcam the preview opens by default.
pub const DEFAULT_CAMERA: &str = "/dev/video0";

/// Preview width as a fraction of the window's.
const PREVIEW_WIDTH: f32 = 0.25;

/// Hand regions are drawn in this color (RGBA)...
const REGION_COLOR: [u8; 4] = [80, 230, 120, 255];
/// ...the skeleton in this one...
const SKELETON_COLOR: [u8; 4] = [255, 255, 255, 255];
/// ...and joints in this one...
const JOINT_COLOR: [u8; 4] = [255, 80, 80, 255];
/// ...with lines this thick, in camera pixels.
const LINE_WIDTH: i32 = 3;

/// MediaPipe's hand skeleton: pairs of landmark indices.
const BONES: [(usize, usize); 21] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 4),
    (0, 5),
    (5, 6),
    (6, 7),
    (7, 8),
    (5, 9),
    (9, 10),
    (10, 11),
    (11, 12),
    (9, 13),
    (13, 14),
    (14, 15),
    (15, 16),
    (13, 17),
    (17, 18),
    (18, 19),
    (19, 20),
    (0, 17),
];

/// How often the capture frame rate is re-measured.
const RATE_WINDOW: Duration = Duration::from_secs(1);

/// Shows live tracking's camera frames, mirrored, in the window's corner,
/// with the tracked hands drawn on. Tracking that couldn't start is
/// reported in the overlay; the rest of the app runs on.
pub struct PreviewPlugin {
    tracking: Mutex<Option<Result<LiveTracker, String>>>,
}

impl PreviewPlugin {
    /// Shows `tracking`'s frames, or why there are none.
    pub fn new(tracking: Result<LiveTracker, String>) -> Self {
        Self {
            tracking: Mutex::new(Some(tracking)),
        }
    }
}

impl Plugin for PreviewPlugin {
    fn build(&self, app: &mut App) {
        let tracker = self
            .tracking
            .lock()
            .unwrap()
            .take()
            .expect("PreviewPlugin added twice");
        if let Err(e) = &tracker {
            error!("{e}");
        }
        app.insert_resource(LiveTracking {
            tracker: tracker.ok(),
            hands: 0,
            track_time: Duration::ZERO,
            rate: FrameRate::default(),
            wanted: true,
            has_frame: false,
        })
        .add_systems(Startup, spawn_preview)
        .add_systems(
            Update,
            (show_latest_frame, toggle_preview, show_preview, show_stats).chain(),
        );
    }
}

/// Opens the webcam at `device` and starts tracking hands in it.
pub fn start_tracking(device: &str) -> Result<LiveTracker, String> {
    let camera = Camera::open(device).map_err(|e| e.to_string())?;
    let tracker = HandTracker::new(PALM_MODEL, LANDMARK_MODEL)
        .map_err(|e| format!("can't load hand models from {PALM_MODEL}, {LANDMARK_MODEL}: {e}"))?;
    LiveTracker::spawn(camera, tracker).map_err(|e| format!("can't start hand tracking: {e}"))
}

#[derive(Resource)]
struct LiveTracking {
    tracker: Option<LiveTracker>,
    /// Hands tracked in the latest frame.
    hands: usize,
    /// How long tracking took on the latest frame.
    track_time: Duration,
    rate: FrameRate,
    /// The user wants the preview shown (`P` toggles this).
    wanted: bool,
    /// A camera image has reached the preview texture.
    has_frame: bool,
}

impl LiveTracking {
    /// The overlay's camera line.
    fn status(&self) -> String {
        let Some(tracker) = &self.tracker else {
            return "tracking: unavailable (see log)".into();
        };
        if let Some(failure) = tracker.failure() {
            return format!("tracking: {failure}");
        }
        match self.rate.fps() {
            Some(fps) => format!(
                "capture: {fps:.1} fps\nhands: {} (track {:.0} ms)",
                self.hands,
                self.track_time.as_secs_f32() * 1e3
            ),
            None => "capture: waiting for camera…".into(),
        }
    }
}

/// Capture frame rate, from the camera's frame counter over about a second.
#[derive(Default)]
struct FrameRate {
    window_start: Option<(Instant, u64)>,
    measured: Option<f32>,
}

impl FrameRate {
    fn record(&mut self, frame: &CameraFrame) {
        match self.window_start {
            Some((t0, seq0)) => {
                let elapsed = frame.captured.duration_since(t0);
                if elapsed >= RATE_WINDOW {
                    self.measured =
                        Some(frame.sequence.saturating_sub(seq0) as f32 / elapsed.as_secs_f32());
                    self.window_start = Some((frame.captured, frame.sequence));
                }
            }
            None => self.window_start = Some((frame.captured, frame.sequence)),
        }
    }

    /// The last measured rate, unless frames have since stopped coming.
    fn fps(&self) -> Option<f32> {
        let (t0, _) = self.window_start?;
        (t0.elapsed() < 2 * RATE_WINDOW).then_some(self.measured?)
    }
}

#[derive(Component)]
struct Preview;

#[derive(Component)]
struct StatsText;

fn spawn_preview(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = images.add(Image::new_fill(
        Extent3d {
            width: capture::WIDTH,
            height: capture::HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    commands.spawn((
        Preview,
        ImageNode {
            image,
            // Selfie view: your right hand on the right. Display only;
            // camera frames themselves stay unmirrored.
            flip_x: true,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            right: px(12),
            bottom: px(12),
            width: percent(PREVIEW_WIDTH * 100.0),
            aspect_ratio: Some(capture::WIDTH as f32 / capture::HEIGHT as f32),
            ..default()
        },
        Visibility::Hidden,
    ));
    commands.spawn((
        StatsText,
        Text::new(""),
        TextFont::from_font_size(14.0),
        Node {
            position_type: PositionType::Absolute,
            left: px(12),
            top: px(12),
            ..default()
        },
    ));
}

fn show_latest_frame(
    mut tracking: ResMut<LiveTracking>,
    mut images: ResMut<Assets<Image>>,
    preview: Query<&ImageNode, With<Preview>>,
) {
    let tracking = &mut *tracking;
    let Some(tracked) = tracking.tracker.as_ref().and_then(LiveTracker::latest) else {
        return;
    };
    let mut frame = tracked.frame;
    tracking.rate.record(&frame);
    tracking.hands = tracked.hands.len();
    tracking.track_time = tracked.track_time;
    for hand in &tracked.hands {
        draw_hand(&mut frame, hand);
    }
    let Ok(node) = preview.single() else {
        return;
    };
    let Some(mut image) = images.get_mut(&node.image) else {
        return;
    };
    if frame.width != image.width() || frame.height != image.height() {
        warn_once!(
            "camera frames are {}x{}, preview expects {}x{}; not shown",
            frame.width,
            frame.height,
            image.width(),
            image.height()
        );
        return;
    }
    image.data = Some(frame.rgba);
    tracking.has_frame = true;
}

fn toggle_preview(keys: Res<ButtonInput<KeyCode>>, mut tracking: ResMut<LiveTracking>) {
    if keys.just_pressed(KeyCode::KeyP) {
        tracking.wanted = !tracking.wanted;
    }
}

/// Shown when wanted, once there's a picture to show.
fn show_preview(tracking: Res<LiveTracking>, mut preview: Query<&mut Visibility, With<Preview>>) {
    let Ok(mut visibility) = preview.single_mut() else {
        return;
    };
    visibility.set_if_neq(if tracking.wanted && tracking.has_frame {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
}

fn show_stats(tracking: Res<LiveTracking>, mut text: Query<&mut Text, With<StatsText>>) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let status = tracking.status();
    if text.0 != status {
        text.0 = status;
    }
}

/// Draws `tracked`'s region, skeleton and joints on `frame`'s pixels.
fn draw_hand(frame: &mut CameraFrame, tracked: &TrackedHand) {
    let corners = tracked.region.corners();
    for (i, &from) in corners.iter().enumerate() {
        draw_line(frame, from, corners[(i + 1) % corners.len()], REGION_COLOR);
    }
    let size = frame.image().size();
    let joints = tracked.hand.image.map(|p| p.truncate() * size);
    for (a, b) in BONES {
        draw_line(frame, joints[a], joints[b], SKELETON_COLOR);
    }
    for joint in joints {
        draw_line(frame, joint, joint, JOINT_COLOR);
    }
}

/// A `LINE_WIDTH`-thick line from `from` to `to` (camera pixels) on `frame`.
fn draw_line(frame: &mut CameraFrame, from: Vec2, to: Vec2, color: [u8; 4]) {
    let (width, height) = (frame.width as i32, frame.height as i32);
    let steps = from.distance(to).ceil().max(1.0) as i32;
    for step in 0..=steps {
        let p = from.lerp(to, step as f32 / steps as f32).as_ivec2();
        for y in p.y - LINE_WIDTH / 2..=p.y + LINE_WIDTH / 2 {
            for x in p.x - LINE_WIDTH / 2..=p.x + LINE_WIDTH / 2 {
                if (0..width).contains(&x) && (0..height).contains(&y) {
                    let at = ((y * width + x) * 4) as usize;
                    frame.rgba[at..at + 4].copy_from_slice(&color);
                }
            }
        }
    }
}
