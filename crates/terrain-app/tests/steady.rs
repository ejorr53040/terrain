mod support;

use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use support::*;
use terrain_app::GRAB_GAIN;
use terrain_hands::{HandFrame, ReplaySource};

const START: Vec3 = Vec3::new(0.0, 0.0, 0.5);
const REACH: Vec3 = Vec3::new(0.10, 0.0, 0.0);

#[test]
fn a_still_but_jittery_hand_holds_the_cube_steady() {
    let mut noise = Noise::new();
    let jitter_m = 0.003;
    let jitter_rad = 1.5_f32.to_radians();
    let shaky: Vec<HandFrame> = (0..90)
        .map(|_| {
            let wobble = Quat::from_scaled_axis(noise.vec3(jitter_rad));
            frame(&[Pose::pinched(START + noise.vec3(jitter_m)).rotated(wobble)])
        })
        .collect();
    let raw_spread = {
        let offsets: Vec<Vec3> = (0..90).map(|_| noise.vec3(jitter_m) * GRAB_GAIN).collect();
        rms_spread(&offsets)
    };
    let mut h = Harness::new([hold(&[Pose::pinched(START)], SETTLE), shaky].concat());
    h.run(SETTLE + 30);
    let (mut places, mut turns) = (vec![], vec![]);
    for _ in 0..60 {
        h.run(1);
        places.push(h.cube().translation);
        turns.push(h.cube().rotation.to_scaled_axis());
    }
    let spread = rms_spread(&places);
    assert!(
        spread < raw_spread / 2.0,
        "cube wanders {:.2} mm RMS; unsmoothed would be {:.2} mm",
        spread * 1e3,
        raw_spread * 1e3
    );
    let turn_spread = rms_spread(&turns).to_degrees();
    assert!(turn_spread < 0.5, "cube wobbles {turn_spread:.2}° RMS");
}

#[test]
fn a_quick_deliberate_move_is_followed_promptly() {
    // 20 cm in five frames (about 1.2 m/s), then stop.
    let sweep: Vec<HandFrame> = (1..=5)
        .map(|i| frame(&[Pose::pinched(START + 2.0 * REACH * i as f32 / 5.0)]))
        .collect();
    let mut h = Harness::new(
        [
            hold(&[Pose::pinched(START)], SETTLE),
            sweep,
            hold(&[Pose::pinched(START + 2.0 * REACH)], SETTLE),
        ]
        .concat(),
    );
    h.run(SETTLE + 5 + 4);
    let lag = 0.30 - h.cube().translation.x;
    assert!(
        lag < 0.015,
        "cube still {:.1} mm behind 4 frames after the hand stopped",
        lag * 1e3
    );
}

#[test]
fn a_brief_tracking_dropout_keeps_the_cube_held_and_frozen() {
    let mut h = Harness::new(
        [
            hold(&[Pose::pinched(START)], SETTLE),
            hold(&[Pose::pinched(START + REACH)], SETTLE),
            vec![no_hands(); 6], // ~200 ms
            hold(&[Pose::pinched(START + 2.0 * REACH)], SETTLE),
        ]
        .concat(),
    );
    h.run(2 * SETTLE);
    let before = h.cube().translation;
    for _ in 0..6 {
        h.run(1);
        assert_near(h.cube().translation, before);
    }
    h.run(SETTLE);
    assert_near(h.cube().translation, Vec3::new(0.30, 0.0, 0.0));
}

#[test]
fn a_long_tracking_dropout_lets_go_of_the_cube() {
    let mut h = Harness::new(
        [
            hold(&[Pose::pinched(START)], SETTLE),
            hold(&[Pose::pinched(START + REACH)], SETTLE),
            vec![no_hands(); 12], // ~400 ms
            hold(&[Pose::pinched(START + 2.0 * REACH)], SETTLE),
        ]
        .concat(),
    );
    h.run_all();
    // Released during the dropout; the returning pinch grabs afresh, in place.
    assert_near(h.cube().translation, Vec3::new(0.15, 0.0, 0.0));
}

#[test]
fn the_second_hand_is_ignored_while_the_first_holds() {
    let left_at = START + Vec3::new(-0.15, 0.0, 0.0);
    let mut h = Harness::new(
        [
            hold(&[Pose::pinched(START)], SETTLE),
            hold(
                &[Pose::pinched(left_at).left(), Pose::pinched(START)],
                SETTLE,
            ),
            hold(
                &[Pose::pinched(left_at + REACH).left(), Pose::pinched(START)],
                SETTLE,
            ),
        ]
        .concat(),
    );
    h.run_all();
    assert_near(h.cube().translation, Vec3::ZERO);
}

#[test]
fn either_hand_can_grab() {
    let mut h = Harness::new(
        [
            hold(&[Pose::open(START), Pose::pinched(START).left()], SETTLE),
            hold(
                &[Pose::open(START), Pose::pinched(START + REACH).left()],
                SETTLE,
            ),
        ]
        .concat(),
    );
    h.run_all();
    assert_near(h.cube().translation, Vec3::new(0.15, 0.0, 0.0));
}

#[test]
fn replay_follows_the_recorded_timestamps() {
    // Recorded at 15 fps, half the app's rate, with the move half a second
    // in. Played one frame per update, the move would arrive by update 8.
    let recorded_ms = 2 * FRAME_MS;
    let frames = (0..40)
        .map(|i| {
            let t_ms = i * recorded_ms;
            let reach = if t_ms < 500 { Vec3::ZERO } else { REACH };
            HandFrame {
                t_ms,
                ..frame(&[Pose::pinched(START + reach)])
            }
        })
        .collect();
    let mut h = Harness::timed(frames);
    h.run(14); // ~460 ms
    assert_near(h.cube().translation, Vec3::ZERO);
    h.run(2 * SETTLE);
    assert_near(h.cube().translation, Vec3::new(0.15, 0.0, 0.0));
}

#[test]
fn a_hand_back_after_a_long_silence_grabs_afresh() {
    let at = |t_ms, pose| HandFrame {
        t_ms,
        ..frame(&[pose])
    };
    let held: Vec<HandFrame> = (0..2 * SETTLE as u64)
        .map(|i| {
            let reach = if i < SETTLE as u64 { Vec3::ZERO } else { REACH };
            at(i * FRAME_MS, Pose::pinched(START + reach))
        })
        .collect();
    // The tracker reports nothing for 400 ms, then the hand is back, still
    // pinching, farther on: the old grab is over, so this one starts in place.
    let back = held.last().unwrap().t_ms + 400;
    let returned =
        (0..SETTLE as u64).map(|i| at(back + i * FRAME_MS, Pose::pinched(START + 2.0 * REACH)));
    let mut h = Harness::timed(held.into_iter().chain(returned).collect());
    h.run(2 * SETTLE + 13 + SETTLE);
    assert_near(h.cube().translation, Vec3::new(0.15, 0.0, 0.0));
}

#[test]
fn a_hand_pinching_since_before_the_release_does_not_take_over() {
    let left_at = START + Vec3::new(-0.15, 0.0, 0.0);
    let mut h = Harness::new(
        [
            hold(&[Pose::pinched(START)], SETTLE),
            hold(
                &[Pose::pinched(left_at).left(), Pose::pinched(START)],
                SETTLE,
            ),
            // The right hand lets go; the left, pinching all along, moves.
            hold(&[Pose::pinched(left_at).left(), Pose::open(START)], SETTLE),
            hold(
                &[Pose::pinched(left_at + REACH).left(), Pose::open(START)],
                SETTLE,
            ),
        ]
        .concat(),
    );
    h.run_all();
    assert_near(h.cube().translation, Vec3::ZERO);
}

#[test]
fn a_quick_deliberate_turn_is_followed_promptly() {
    // A quarter turn in five frames (about 9 rad/s), then stop.
    let turn = |fraction: f32| Quat::from_rotation_y(FRAC_PI_2 * fraction);
    let sweep: Vec<HandFrame> = (1..=5)
        .map(|i| frame(&[Pose::pinched(START).rotated(turn(i as f32 / 5.0))]))
        .collect();
    let mut h = Harness::new(
        [
            hold(&[Pose::pinched(START)], SETTLE),
            sweep,
            hold(&[Pose::pinched(START).rotated(turn(1.0))], SETTLE),
        ]
        .concat(),
    );
    h.run(SETTLE + 5 + 4);
    let lag = h.cube().rotation.angle_between(turn(1.0)).to_degrees();
    assert!(
        lag < 5.0,
        "cube still {lag:.1}° behind 4 frames after the hand stopped"
    );
}

#[test]
fn a_looping_replay_keeps_time_moving_forward() {
    // Each loop opens with a 400 ms dropout, which must release the cube
    // even though the recording's clock restarts there.
    let recording = [
        vec![no_hands(); 12],
        hold(&[Pose::pinched(START)], SETTLE),
        hold(&[Pose::pinched(START + REACH)], SETTLE),
    ]
    .concat();
    let per_loop = recording.len();
    let stamped = recording
        .into_iter()
        .enumerate()
        .map(|(i, f)| HandFrame {
            t_ms: i as u64 * FRAME_MS,
            ..f
        })
        .collect();
    let mut h = Harness::with_source(ReplaySource::new(stamped).looping());
    h.run(2 * per_loop + 1);
    assert_near(h.cube().translation, Vec3::new(0.30, 0.0, 0.0));
}

#[test]
fn a_held_pinch_survives_one_frame_that_reads_open() {
    // A blurred frame can read as an open hand mid-drag; the loose pinch
    // after it (too open to start a grab) must still be holding.
    let loose = 0.04;
    let mut h = Harness::new(
        [
            hold(&[Pose::pinched(START)], SETTLE),
            hold(&[Pose::gap(START + REACH, loose)], SETTLE),
            vec![frame(&[Pose::open(START + REACH)])],
            hold(&[Pose::gap(START + 2.0 * REACH, loose)], SETTLE),
        ]
        .concat(),
    );
    h.run_all();
    assert_near(h.cube().translation, Vec3::new(0.30, 0.0, 0.0));
}

#[test]
fn a_still_hand_with_noisy_depth_holds_the_cube_steady() {
    // A webcam judges distance far less steadily than left-right or up-down:
    // a still hand's depth reads within about ±2 cm frame to frame.
    let mut noise = Noise::new();
    let noisy: Vec<HandFrame> = (0..90)
        .map(|_| frame(&[Pose::pinched(START + Vec3::Z * 0.02 * noise.next())]))
        .collect();
    let mut h = Harness::new([hold(&[Pose::pinched(START)], SETTLE), noisy].concat());
    h.run(SETTLE + 30);
    let mut places = vec![];
    for _ in 0..60 {
        h.run(1);
        places.push(h.cube().translation);
    }
    let spread = rms_spread(&places);
    assert!(spread < 0.004, "cube wanders {:.1} mm RMS", spread * 1e3);
}

#[test]
fn a_quick_deliberate_move_in_depth_is_followed_promptly() {
    // 20 cm toward the camera in five frames, then stop.
    let toward = Vec3::new(0.0, 0.0, -0.20);
    let sweep: Vec<HandFrame> = (1..=5)
        .map(|i| frame(&[Pose::pinched(START + toward * i as f32 / 5.0)]))
        .collect();
    let mut h = Harness::new(
        [
            hold(&[Pose::pinched(START)], SETTLE),
            sweep,
            hold(&[Pose::pinched(START + toward)], SETTLE),
        ]
        .concat(),
    );
    h.run(SETTLE + 5 + 4);
    let lag = h.cube().translation.z - GRAB_GAIN * toward.z;
    assert!(
        lag < 0.015,
        "cube still {:.1} mm behind 4 frames after the hand stopped",
        lag * 1e3
    );
}

#[test]
fn two_hands_read_as_the_same_hand_are_still_told_apart() {
    // The tracker often labels a second hand the same as the first.
    let open_at = START + Vec3::new(0.20, 0.0, 0.0);
    let mut h = Harness::new(
        [
            hold(&[Pose::open(open_at), Pose::open(START)], SETTLE),
            hold(&[Pose::open(open_at), Pose::pinched(START)], SETTLE),
            hold(&[Pose::open(open_at), Pose::pinched(START + REACH)], SETTLE),
            hold(&[Pose::open(open_at), Pose::open(START + REACH)], SETTLE),
        ]
        .concat(),
    );
    h.run_all();
    assert_near(h.cube().translation, Vec3::new(0.15, 0.0, 0.0));
}

#[test]
fn a_hand_appearing_across_the_view_does_not_inherit_the_grab() {
    let left_side = START + Vec3::new(-0.15, 0.0, 0.0);
    let right_side = START + Vec3::new(0.15, 0.0, 0.0);
    let mut h = Harness::new(
        [
            hold(&[Pose::pinched(left_side)], SETTLE),
            // The holding hand leaves; another, already pinching, shows up
            // across the view a frame later and moves.
            vec![no_hands()],
            hold(&[Pose::pinched(right_side)], 2),
            hold(&[Pose::pinched(right_side + REACH)], SETTLE),
        ]
        .concat(),
    );
    h.run_all();
    assert_near(h.cube().translation, Vec3::ZERO);
}
