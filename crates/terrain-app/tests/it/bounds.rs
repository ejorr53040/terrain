
use bevy::prelude::*;
use crate::support::*;
use terrain_app::{Arena, CUBE_SIZE};
use terrain_hands::HandFrame;

/// Where the palm starts each drag, in front of the camera.
const START: Vec3 = Vec3::new(0.0, 0.0, 1.0);

/// Drags the hand `by` from `START` and lets go, `times` over: each pinch
/// begins afresh, so the cube is carried further each time.
fn drag_repeatedly(by: Vec3, times: usize) -> Vec<HandFrame> {
    let one_drag = [
        hold(&[Pose::open(START)], SETTLE),
        hold(&[Pose::pinched(START)], SETTLE),
        hold(&[Pose::pinched(START + by)], SETTLE),
    ]
    .concat();
    (0..times).flat_map(|_| one_drag.clone()).collect()
}

#[test]
fn a_cube_dragged_far_past_each_side_stops_at_the_box() {
    // Where a cube's centre can be: the box shrunk by the cube's half-width.
    let half = Vec3::splat(CUBE_SIZE / 2.0);
    let arena = Arena::default();
    let (lowest, highest) = (arena.min + half, arena.max - half);
    for axis in 0..3 {
        for (sign, wall) in [(1.0, highest[axis]), (-1.0, lowest[axis])] {
            let mut by = Vec3::ZERO;
            by[axis] = sign * if axis == 1 { 0.15 } else { 0.3 };
            let mut h = Harness::with_cubes(drag_repeatedly(by, 5), &[Vec3::ZERO]);
            h.run_all().run(SETTLE);
            assert!(
                (h.cube_at(0)[axis] - wall).abs() < 0.02,
                "dragging by {by}: cube at {}, should rest at the wall, {wall}",
                h.cube_at(0)
            );
        }
    }
}

#[test]
fn the_whole_box_is_in_view_of_the_scene_camera() {
    let mut h = Harness::with_cubes(Vec::new(), &[Vec3::ZERO]);
    let Arena { min, max, .. } = Arena::default();
    let world = h.app.world_mut();
    let (camera, projection) = world
        .query_filtered::<(&Transform, &Projection), With<Camera3d>>()
        .single(world)
        .unwrap();
    let clip_from_world = projection.get_clip_from_view() * camera.to_matrix().inverse();
    for i in 0..8 {
        let corner = Vec3::select(
            BVec3::new(i & 1 != 0, i & 2 != 0, i & 4 != 0),
            max,
            min,
        );
        let clip = clip_from_world * corner.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        assert!(
            clip.w > 0.0 && ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0,
            "box corner {corner} is off screen, at {ndc} (w {})",
            clip.w
        );
    }
}
