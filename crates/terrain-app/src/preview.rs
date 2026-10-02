//! The mirrored webcam preview in a corner of the window, toggled with `P`,
//! and a debug overlay with the capture frame rate.

use std::time::{Duration, Instant};

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use terrain_hands::capture::{self, Camera, CameraFrame};

/// The webcam the preview opens by default.
pub const DEFAULT_CAMERA: &str = "/dev/video0";

/// Preview width as a fraction of the window's.
const PREVIEW_WIDTH: f32 = 0.25;

/// How often the capture frame rate is re-measured.
const RATE_WINDOW: Duration = Duration::from_secs(1);

/// Captures from a webcam and shows it, mirrored, in the window's corner.
/// A camera that can't be opened is reported in the overlay; the rest of
/// the app runs on.
pub struct PreviewPlugin {
    pub device: String,
}

impl Plugin for PreviewPlugin {
    fn build(&self, app: &mut App) {
        let camera = Camera::open(&self.device).map_err(|e| e.to_string());
        if let Err(e) = &camera {
            error!("{e}");
        }
        app.insert_resource(PreviewCamera {
            camera: camera.ok(),
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

#[derive(Resource)]
struct PreviewCamera {
    camera: Option<Camera>,
    rate: FrameRate,
    /// The user wants the preview shown (`P` toggles this).
    wanted: bool,
    /// A camera image has reached the preview texture.
    has_frame: bool,
}

impl PreviewCamera {
    /// The overlay's camera line.
    fn status(&self) -> String {
        let Some(camera) = &self.camera else {
            return "camera: unavailable (see log)".into();
        };
        if let Some(failure) = camera.failure() {
            return format!("camera: {failure}");
        }
        match self.rate.fps() {
            Some(fps) => format!("capture: {fps:.1} fps"),
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
    mut preview_camera: ResMut<PreviewCamera>,
    mut images: ResMut<Assets<Image>>,
    preview: Query<&ImageNode, With<Preview>>,
) {
    let preview_camera = &mut *preview_camera;
    let Some(frame) = preview_camera.camera.as_ref().and_then(Camera::latest) else {
        return;
    };
    preview_camera.rate.record(&frame);
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
    preview_camera.has_frame = true;
}

fn toggle_preview(keys: Res<ButtonInput<KeyCode>>, mut preview_camera: ResMut<PreviewCamera>) {
    if keys.just_pressed(KeyCode::KeyP) {
        preview_camera.wanted = !preview_camera.wanted;
    }
}

/// Shown when wanted, once there's a picture to show.
fn show_preview(
    preview_camera: Res<PreviewCamera>,
    mut preview: Query<&mut Visibility, With<Preview>>,
) {
    let Ok(mut visibility) = preview.single_mut() else {
        return;
    };
    visibility.set_if_neq(if preview_camera.wanted && preview_camera.has_frame {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
}

fn show_stats(preview_camera: Res<PreviewCamera>, mut text: Query<&mut Text, With<StatsText>>) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let status = preview_camera.status();
    if text.0 != status {
        text.0 = status;
    }
}
