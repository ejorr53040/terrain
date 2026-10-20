use crate::support::*;
use bevy::prelude::*;
use terrain_app::HandCursor;

/// Straight ahead of the camera, so the middle of the screen.
const AHEAD: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const LEFT: Vec3 = Vec3::new(-0.3, 0.0, 0.0);

fn cursors(h: &mut Harness) -> Vec<HandCursor> {
    let world = h.app.world_mut();
    world.query::<&HandCursor>().iter(world).copied().collect()
}

#[test]
fn a_tracked_hand_shows_where_it_is_on_screen() {
    let mut h = Harness::with_cubes(hold(&[Pose::open(AHEAD)], SETTLE), &[Vec3::ZERO]);
    h.run_all();
    let [cursor] = cursors(&mut h)[..] else {
        panic!("one hand should show one cursor");
    };
    assert!(
        cursor.at.abs_diff_eq(Vec2::splat(0.5), 0.03),
        "a hand dead ahead should be mid-screen, cursor is at {}",
        cursor.at
    );
    assert!(!cursor.pinching);

    // Moved left, and pinched: the cursor follows, and says it's holding.
    let mut h = Harness::with_cubes(
        [
            hold(&[Pose::open(AHEAD + LEFT)], SETTLE),
            hold(&[Pose::pinched(AHEAD + LEFT)], SETTLE),
        ]
        .concat(),
        &[Vec3::ZERO],
    );
    h.run_all();
    let [cursor] = cursors(&mut h)[..] else {
        panic!("one hand should show one cursor");
    };
    assert!(
        cursor.at.x < 0.35 && (cursor.at.y - 0.5).abs() < 0.03,
        "a hand on the left should be left of mid-screen, cursor is at {}",
        cursor.at
    );
    assert!(cursor.pinching);
}

#[test]
fn each_hand_has_its_own_cursor_and_loses_it_when_the_hand_goes() {
    let both = [Pose::open(AHEAD + LEFT), Pose::open(AHEAD - LEFT)];
    let mut h = Harness::with_cubes(
        [hold(&both, SETTLE), hold(&[both[0]], 1)].concat(),
        &[Vec3::ZERO],
    );
    h.run(SETTLE);
    let mut two = cursors(&mut h);
    two.sort_by(|a, b| a.at.x.total_cmp(&b.at.x));
    let [left, right] = two[..] else {
        panic!("two hands should show two cursors, got {two:?}");
    };
    assert!(left.at.x < 0.5 && right.at.x > 0.5 && left.id != right.id);

    // The right hand drops out: only its cursor goes.
    h.run(1);
    let [still] = cursors(&mut h)[..] else {
        panic!("one hand left should leave one cursor");
    };
    assert_eq!(still.id, left.id);
}

/// Where `at` lands in the window the harness's scene camera draws, in the
/// cursor's terms: 0 to 1 across and down from the top left.
fn on_screen(h: &mut Harness, at: Vec3) -> Vec2 {
    let world = h.app.world_mut();
    let (camera, projection) = world
        .query::<(&Transform, &Projection)>()
        .single(world)
        .unwrap();
    let clip = projection.get_clip_from_view() * camera.to_matrix().inverse() * at.extend(1.0);
    let ndc = clip.truncate().truncate() / clip.w;
    Vec2::new((ndc.x + 1.0) / 2.0, (1.0 - ndc.y) / 2.0)
}

#[test]
fn a_held_cube_stays_under_its_cursor_with_the_hand_near_or_far() {
    // From dead ahead, where the cube starts under the cursor, carry it
    // 10 cm right and 5 cm up.
    let carry = Vec3::new(0.10, 0.05, 0.0);
    for distance in [0.5, 1.0, 1.5] {
        let ahead = Vec3::new(0.0, 0.0, distance);
        let mut h = Harness::with_cubes(
            [
                hold(&[Pose::open(ahead)], SETTLE),
                hold(&[Pose::pinched(ahead)], SETTLE),
                hold(&[Pose::pinched(ahead + carry)], SETTLE),
            ]
            .concat(),
            &[Vec3::ZERO],
        );
        h.run_all();
        let [cursor] = cursors(&mut h)[..] else {
            panic!("one hand should show one cursor");
        };
        let at = h.cube_at(0);
        let cube = on_screen(&mut h, at);
        assert!(
            cube.abs_diff_eq(cursor.at, 0.01),
            "hand {distance} m away: cube is on screen at {cube}, its cursor at {}",
            cursor.at
        );
    }
}

#[test]
fn a_held_cube_stays_under_its_cursor_when_the_hand_moves_along_its_sight_line() {
    // Carry the cube off centre, then slide the hand along the camera's line
    // of sight through it, as a misjudged depth does: the cursor doesn't
    // move, so the cube mustn't move off it on screen.
    let carry = Vec3::new(0.15, 0.08, 0.0);
    let ahead = Vec3::new(0.0, 0.0, 1.0);
    let off = ahead + carry;
    let mut frames = [
        hold(&[Pose::open(ahead)], SETTLE),
        hold(&[Pose::pinched(ahead)], SETTLE),
        hold(&[Pose::pinched(off)], SETTLE),
    ]
    .concat();
    for scale in [0.8, 1.25] {
        frames.extend(hold(&[Pose::pinched(off * scale)], SETTLE));
    }
    let mut h = Harness::with_cubes(frames, &[Vec3::ZERO]);
    h.run(3 * SETTLE);
    let [cursor] = cursors(&mut h)[..] else {
        panic!("one hand should show one cursor");
    };
    let at = h.cube_at(0);
    let start = on_screen(&mut h, at);
    assert!(
        start.abs_diff_eq(cursor.at, 0.01),
        "carried off centre under the cursor"
    );
    for scale in [0.8, 1.25] {
        h.run(SETTLE);
        let at = h.cube_at(0);
        let cube = on_screen(&mut h, at);
        assert!(
            cube.abs_diff_eq(cursor.at, 0.01),
            "hand at {scale}x along its sight line: cube is on screen at {cube}, its cursor at {}",
            cursor.at
        );
    }
}
