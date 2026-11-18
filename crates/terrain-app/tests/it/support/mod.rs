//! The headless app harness, and the synthetic hands it's shown, placed in
//! front of a pinhole camera so tests can speak in real-world meters.

#![allow(dead_code)]

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use bevy::{prelude::*, time::TimeUpdateStrategy};
use terrain_app::{GrabPlugin, Grabbable, HeadOrbitPlugin, HeadSource, SCENE_CAMERA_AT, Shape};
use terrain_hands::{HandFrame, HandSource, ReplaySource};
use terrain_head::{HeadLook, HeadLookSettings, OffReason};

pub use terrain_hands::synthetic::{Pose, TestCamera};

/// Time between camera frames, and between app updates, in tests.
pub const FRAME_MS: u64 = 33;

/// Frames to hold a pose for the smoothed hand to settle on it.
pub const SETTLE: usize = 30;

/// Where calibration asks for the hand: straight in front of the camera.
pub const CALIBRATION_SPOT: Vec3 = Vec3::new(0.0, 0.0, terrain_app::CALIBRATION_DISTANCE_M);
/// Frames covering calibration's 3 s countdown and 1 s of measuring.
pub const CALIBRATING: usize = 130;

/// One frame of `poses`, seen by the default camera.
pub fn frame(poses: &[Pose]) -> HandFrame {
    TestCamera::default().frame(poses)
}

/// `n` identical frames of `poses`: the hand held still.
pub fn hold(poses: &[Pose], n: usize) -> Vec<HandFrame> {
    TestCamera::default().hold(poses, n)
}

/// A frame where the tracker sees no hands.
pub fn no_hands() -> HandFrame {
    frame(&[])
}

/// A headless app with one cube at the origin (or several cubes and a scene
/// camera, with `with_cubes`). Each update advances the clock by `FRAME_MS`.
pub struct Harness {
    pub app: App,
    pub cube: Entity,
    pub cubes: Vec<Entity>,
    /// Frames given up front, for `run_all`; 0 for `with_source`.
    frame_count: usize,
}

/// `frames` re-stamped `FRAME_MS` apart, one per update.
fn stamped(frames: Vec<HandFrame>) -> Vec<HandFrame> {
    frames
        .into_iter()
        .enumerate()
        .map(|(i, f)| HandFrame {
            t_ms: i as u64 * FRAME_MS,
            ..f
        })
        .collect()
}

impl Harness {
    /// Plays `frames` one per update (re-stamped `FRAME_MS` apart).
    pub fn new(frames: Vec<HandFrame>) -> Self {
        Self::timed(stamped(frames))
    }

    /// Plays `frames` at their own `t_ms` timestamps.
    pub fn timed(frames: Vec<HandFrame>) -> Self {
        Self {
            frame_count: frames.len(),
            ..Self::with_source(ReplaySource::new(frames))
        }
    }

    pub fn with_source(source: impl HandSource + 'static) -> Self {
        Self::assemble(source, |_| {})
    }

    /// The app around `source`, with `more` added before it's finished.
    fn assemble(source: impl HandSource + 'static, more: impl FnOnce(&mut App)) -> Self {
        let mut app = App::new();
        // What physics needs of `DefaultPlugins`, without a window.
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            bevy::asset::AssetPlugin::default(),
        ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            FRAME_MS,
        )))
        .add_plugins(GrabPlugin::new(source));
        more(&mut app);
        // `App::run` does this; physics sets up some resources here.
        app.finish();
        app.cleanup();
        let cube = app
            .world_mut()
            .spawn((Transform::default(), Grabbable))
            .id();
        Self {
            app,
            cube,
            cubes: vec![cube],
            frame_count: 0,
        }
    }

    /// Plays `frames` (as `new` does) in a scene with a cube at each of
    /// `at`, seen by a camera where the app puts its own.
    pub fn with_cubes(frames: Vec<HandFrame>, at: &[Vec3]) -> Self {
        Self::new(frames).into_scene(at)
    }

    /// As `with_cubes`, with Head look from `head` orbiting the camera.
    pub fn with_head(frames: Vec<HandFrame>, at: &[Vec3], head: impl HeadSource) -> Self {
        let frames = stamped(frames);
        Self {
            frame_count: frames.len(),
            ..Self::assemble(ReplaySource::new(frames), |app| {
                app.add_plugins(HeadOrbitPlugin::new(head));
            })
        }
        .into_scene(at)
    }

    /// Swaps the lone cube for a cube at each of `at` and the app's camera.
    fn into_scene(mut self, at: &[Vec3]) -> Self {
        let world = self.app.world_mut();
        world.despawn(self.cube);
        // Framed like the app's default 1280x720 window.
        world.spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection {
                aspect_ratio: 16.0 / 9.0,
                ..default()
            }),
            Transform::from_translation(SCENE_CAMERA_AT).looking_at(Vec3::ZERO, Vec3::Y),
        ));
        self.cubes = at
            .iter()
            .map(|&p| {
                world
                    .spawn((Transform::from_translation(p), Grabbable))
                    .id()
            })
            .collect();
        self.cube = self.cubes.first().copied().unwrap_or(Entity::PLACEHOLDER);
        self
    }

    /// The scene camera, of a harness `with_cubes` or `with_head`.
    pub fn camera(&mut self) -> Transform {
        let world = self.app.world_mut();
        *world
            .query_filtered::<&Transform, With<Camera3d>>()
            .single(world)
            .unwrap()
    }

    /// Cube `i`'s place, of those given to `with_cubes`.
    pub fn cube_at(&self, i: usize) -> Vec3 {
        self.app
            .world()
            .get::<Transform>(self.cubes[i])
            .unwrap()
            .translation
    }

    /// Plays every frame given to `new` or `timed`.
    pub fn run_all(&mut self) -> &mut Self {
        self.run(self.frame_count)
    }

    /// Presses and releases `key` over one update.
    pub fn press(&mut self, key: KeyCode) -> &mut Self {
        self.chord(&[key])
    }

    /// Presses `keys` together (say Shift then H) and releases them over one update.
    pub fn chord(&mut self, keys: &[KeyCode]) -> &mut Self {
        let mut input = self.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        for &key in keys {
            input.press(key);
        }
        self.run(1);
        let mut input = self.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.release_all();
        input.clear();
        self
    }

    pub fn run(&mut self, updates: usize) -> &mut Self {
        for _ in 0..updates {
            self.app.update();
        }
        self
    }

    pub fn cube(&self) -> Transform {
        *self.app.world().get::<Transform>(self.cube).unwrap()
    }

    /// The first cube's shape.
    pub fn shape(&self) -> Shape {
        *self.app.world().get::<Shape>(self.cube).unwrap()
    }
}

pub fn assert_near(actual: Vec3, expected: Vec3) {
    assert!(
        actual.abs_diff_eq(expected, 1e-3),
        "expected {expected}, got {actual}"
    );
}

/// Head look as a test sets it, standing in for the AirPods: Off until
/// switched on, then Starting until the test turns the head, then On at the
/// head's angles from Straight ahead (dead zone already applied, as
/// `HeadTracker` does). Clones share one head.
#[derive(Clone, Default)]
pub struct FakeHead(Arc<Mutex<Head>>);

#[derive(Default)]
struct Head {
    on: bool,
    /// Why it went Off, other than being switched off.
    off_because: Option<OffReason>,
    /// Yaw and pitch from where it started, once samples come.
    turned: Option<(f32, f32)>,
    straight_ahead: (f32, f32),
}

impl FakeHead {
    /// The head now points `yaw_deg` left and `pitch_deg` down of where it started.
    pub fn turn(&self, yaw_deg: f32, pitch_deg: f32) {
        self.0.lock().unwrap().turned = Some((yaw_deg, pitch_deg));
    }

    /// The AirPods come off the head: Off(OffHead) until switched on again.
    pub fn take_off(&self) {
        let mut head = self.0.lock().unwrap();
        head.on = false;
        head.off_because = Some(OffReason::OffHead);
    }
}

impl HeadSource for FakeHead {
    fn toggle(&mut self) {
        let mut head = self.0.lock().unwrap();
        head.on = !head.on;
        head.off_because = None;
        head.straight_ahead = (0.0, 0.0);
    }

    fn recenter(&mut self) {
        let mut head = self.0.lock().unwrap();
        if let (true, Some(turned)) = (head.on, head.turned) {
            head.straight_ahead = turned;
        }
    }

    fn set_settings(&mut self, _: HeadLookSettings) {}

    fn look(&mut self) -> HeadLook {
        let head = self.0.lock().unwrap();
        match (head.on, head.turned) {
            (false, _) => HeadLook::Off(head.off_because.unwrap_or(OffReason::SwitchedOff)),
            (true, None) => HeadLook::Starting,
            (true, Some((yaw, pitch))) => HeadLook::On {
                yaw_deg: yaw - head.straight_ahead.0,
                pitch_deg: pitch - head.straight_ahead.1,
            },
        }
    }
}

/// Small deterministic noise in [-1, 1], so jitter tests are repeatable.
pub struct Noise(u64);

impl Noise {
    pub fn new() -> Self {
        Self(0x9e37_79b9_7f4a_7c15)
    }

    pub fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }

    pub fn vec3(&mut self, amplitude: f32) -> Vec3 {
        Vec3::new(self.next(), self.next(), self.next()) * amplitude
    }
}

/// Root-mean-square distance of `samples` from their mean.
pub fn rms_spread(samples: &[Vec3]) -> f32 {
    let mean = samples.iter().sum::<Vec3>() / samples.len() as f32;
    (samples
        .iter()
        .map(|s| s.distance_squared(mean))
        .sum::<f32>()
        / samples.len() as f32)
        .sqrt()
}

pub fn assert_turned(actual: Quat, expected: Quat) {
    let off = actual.angle_between(expected).to_degrees();
    assert!(
        off < 0.5,
        "expected {expected}, got {actual} ({off:.2}° off)"
    );
}
