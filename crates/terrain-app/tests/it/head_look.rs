//! Head look orbits the scene camera round the scene's middle: `H` switches
//! it on and off, `Shift+H` Recenters, and Off glides back to the default view.

use crate::support::*;
use bevy::prelude::*;
use terrain_app::{HeadOrbit, HeadOrbitSettings, SCENE_CAMERA_AT};
use terrain_head::{HeadLook, HeadLookSettings, HeadTracker, OffReason};

/// Long enough for the camera's glide to arrive.
const GLIDE: usize = 60;

/// The default camera's distance from the scene's middle, level with it.
const ACROSS: f32 = 1.2;
/// The default camera's height.
const HEIGHT: f32 = 0.25;

/// Asserts the camera is at `at`, looking at the scene's middle.
fn assert_camera(h: &mut Harness, at: Vec3) {
    let camera = h.camera();
    assert_near(camera.translation, at);
    assert_near(*camera.forward(), -at.normalize());
}

#[test]
fn turning_the_head_left_swings_the_camera_round_to_the_scenes_left() {
    let head = FakeHead::default();
    let mut h = Harness::with_head(Vec::new(), &[], head.clone());
    h.press(KeyCode::KeyH);
    // 15° of head turn is a quarter turn of orbit at the default gain of 6.
    head.turn(15.0, 0.0);
    h.run(GLIDE);
    assert_camera(&mut h, Vec3::new(-ACROSS, HEIGHT, 0.0));
}

/// The camera `degrees` up from level, straight in front of the scene, as far
/// from its middle as the default camera.
fn raised(degrees: f32) -> Vec3 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    Vec3::new(0.0, sin, cos) * Vec3::new(0.0, HEIGHT, ACROSS).length()
}

#[test]
fn nodding_raises_the_view_between_level_and_straight_down() {
    let head = FakeHead::default();
    let mut h = Harness::with_head(Vec::new(), &[], head.clone());
    h.press(KeyCode::KeyH);
    let default_elevation = HEIGHT.atan2(ACROSS).to_degrees();
    // 10° down raises the view 30° at the default gain of 3.
    head.turn(0.0, 10.0);
    h.run(GLIDE);
    assert_camera(&mut h, raised(default_elevation + 30.0));
    // Far down stops just short of straight down; far up stops level.
    head.turn(0.0, 40.0);
    h.run(GLIDE);
    assert_camera(&mut h, raised(89.0));
    head.turn(0.0, -20.0);
    h.run(GLIDE);
    assert_camera(&mut h, raised(0.0));
}

/// A harness whose camera Head look has swung a quarter turn to the scene's left.
fn turned_left(head: &FakeHead) -> Harness {
    let mut h = Harness::with_head(Vec::new(), &[], head.clone());
    h.press(KeyCode::KeyH);
    head.turn(15.0, 0.0);
    h.run(GLIDE);
    h
}

/// Asserts the camera glides back to the default view: one frame on it has
/// left where it was, but not got near, and soon it's there.
fn assert_glides_home(h: &mut Harness) {
    let from = h.camera().translation;
    let way = from.distance(SCENE_CAMERA_AT);
    h.run(1);
    let left = h.camera().translation.distance(SCENE_CAMERA_AT);
    assert!(
        left < way && left > way / 2.0,
        "one frame should start a glide, not jump: {left:.3} m of {way:.3} m to go"
    );
    h.run(GLIDE);
    assert_camera(h, SCENE_CAMERA_AT);
}

#[test]
fn switching_head_look_off_glides_back_to_the_default_view() {
    let head = FakeHead::default();
    let mut h = turned_left(&head);
    h.press(KeyCode::KeyH);
    assert_eq!(
        h.app.world().resource::<HeadOrbit>().look,
        HeadLook::Off(OffReason::SwitchedOff)
    );
    assert_glides_home(&mut h);
}

#[test]
fn taking_the_airpods_off_glides_back_to_the_default_view() {
    let head = FakeHead::default();
    let mut h = turned_left(&head);
    head.take_off();
    assert_glides_home(&mut h);
    assert_eq!(
        h.app.world().resource::<HeadOrbit>().look,
        HeadLook::Off(OffReason::OffHead)
    );
}

#[test]
fn shift_h_recenters_on_where_the_head_points_now() {
    let head = FakeHead::default();
    let mut h = turned_left(&head);
    h.chord(&[KeyCode::ShiftLeft, KeyCode::KeyH]);
    h.run(GLIDE);
    assert_camera(&mut h, SCENE_CAMERA_AT);
    // Still On, measuring from here.
    head.turn(20.0, 0.0);
    h.run(GLIDE);
    let (sin, cos) = 30f32.to_radians().sin_cos();
    assert_camera(&mut h, Vec3::new(-ACROSS * sin, HEIGHT, ACROSS * cos));
}

#[test]
fn without_the_airpods_daemon_h_says_head_look_is_unavailable_and_the_camera_stays_put() {
    let nowhere =
        std::env::temp_dir().join(format!("terrain-no-daemon-{}.sock", std::process::id()));
    let tracker = HeadTracker::with_socket(HeadLookSettings::default(), nowhere);
    let mut h = Harness::with_head(Vec::new(), &[], tracker);
    h.press(KeyCode::KeyH).run(10);
    let orbit = h.app.world().resource::<HeadOrbit>();
    assert_eq!(orbit.look, HeadLook::Off(OffReason::NoDaemon));
    let status = orbit.status();
    assert!(
        status.contains("unavailable"),
        "the status should say Head look is unavailable: {status}"
    );
    assert_camera(&mut h, SCENE_CAMERA_AT);
}

#[test]
fn the_gains_set_in_the_menu_take_effect_at_once() {
    let head = FakeHead::default();
    let mut h = turned_left(&head);
    head.turn(15.0, 10.0);
    let mut settings = h.app.world_mut().resource_mut::<HeadOrbitSettings>();
    settings.yaw_gain = 3.0;
    settings.pitch_gain = 0.0;
    h.run(GLIDE);
    // 15° of head turn at 3 is an eighth of a turn; the nod does nothing.
    let (sin, cos) = 45f32.to_radians().sin_cos();
    assert_camera(&mut h, Vec3::new(-ACROSS * sin, HEIGHT, ACROSS * cos));
}

/// Seen from a quarter turn to the scene's left: a cube just to the right of
/// the middle, and one farther off, dead ahead. From the default view the
/// second is the one on the right.
const NEAR_RIGHT: Vec3 = Vec3::new(0.0, 0.0, 0.15);
const FAR_AHEAD: Vec3 = Vec3::new(0.25, 0.0, 0.0);
/// A hand on the right of the webcam's view, and a move up.
const RIGHT_HAND: Vec3 = Vec3::new(0.12, 0.0, 0.5);
const UP: Vec3 = Vec3::new(0.0, 0.05, 0.0);

/// No hands while Head look swings the camera round, then `then`.
fn after_the_swing(then: Vec<Vec<terrain_hands::HandFrame>>) -> Vec<terrain_hands::HandFrame> {
    [vec![no_hands(); GLIDE + 1]]
        .into_iter()
        .chain(then)
        .collect::<Vec<_>>()
        .concat()
}

#[test]
fn a_pinch_from_an_orbited_camera_takes_the_cube_under_the_hand_on_screen() {
    let head = FakeHead::default();
    let frames = after_the_swing(vec![
        hold(&[Pose::open(RIGHT_HAND)], SETTLE),
        hold(&[Pose::pinched(RIGHT_HAND)], SETTLE),
        hold(&[Pose::pinched(RIGHT_HAND + UP)], SETTLE),
    ]);
    let mut h = Harness::with_head(frames, &[NEAR_RIGHT, FAR_AHEAD], head.clone());
    h.press(KeyCode::KeyH);
    head.turn(15.0, 0.0);
    h.run_all();
    let lifted = h.cube_at(0) - NEAR_RIGHT;
    assert!(
        lifted.y > UP.y && lifted.normalize().y > 0.9,
        "the cube on the right on screen should be carried up, moved {lifted}"
    );
    assert_near(h.cube_at(1), FAR_AHEAD);
}

#[test]
fn the_orbit_holds_still_while_a_shape_is_held_and_glides_on_after_release() {
    let head = FakeHead::default();
    let frames = after_the_swing(vec![
        hold(&[Pose::open(RIGHT_HAND)], SETTLE),
        hold(&[Pose::pinched(RIGHT_HAND)], 2 * SETTLE),
        hold(&[Pose::open(RIGHT_HAND)], 2 * GLIDE),
    ]);
    let mut h = Harness::with_head(frames, &[NEAR_RIGHT, FAR_AHEAD], head.clone());
    h.press(KeyCode::KeyH);
    head.turn(15.0, 0.0);
    h.run(GLIDE + 2 * SETTLE);
    let quarter_left = Vec3::new(-ACROSS, HEIGHT, 0.0);
    assert_camera(&mut h, quarter_left);
    // Held: the head turns on, the camera stays.
    head.turn(20.0, 0.0);
    for _ in 0..SETTLE {
        h.run(1);
        assert_camera(&mut h, quarter_left);
    }
    // Let go: it glides on to 120° round, a little at a time.
    let (sin, cos) = 120f32.to_radians().sin_cos();
    let onward = Vec3::new(-ACROSS * sin, HEIGHT, ACROSS * cos);
    let way = quarter_left.distance(onward);
    let mut was = quarter_left;
    for _ in 0..2 * GLIDE {
        h.run(1);
        let now = h.camera().translation;
        assert!(
            now.distance(was) < way * 0.3,
            "the camera jumped {:.3} m of the {way:.3} m to go",
            now.distance(was)
        );
        was = now;
    }
    assert_camera(&mut h, onward);
}
