//! Where things are on screen, as the scene camera sees them.

use bevy::{ecs::system::SystemParam, prelude::*, window::PrimaryWindow};

use crate::{Grabbable, HandCursor, OverlayArea, Shape};

/// The camera the scene is drawn from, for where things appear on screen.
pub(crate) type SceneCamera<'w, 's> =
    Query<'w, 's, (&'static Transform, &'static Projection), (With<Camera3d>, Without<Grabbable>)>;

/// The scene camera's view. Points on screen are in clip space with x
/// scaled by the aspect ratio, so distances are as the eye sees them; a hand
/// or cursor's place in the view is 0 to 1 across and down from the top left.
pub(crate) struct Screen {
    clip_from_world: Mat4,
    aspect: f32,
}

impl Screen {
    /// `None` without a scene camera. The scene has one 3D camera, unparented.
    pub(crate) fn of(scene_camera: &SceneCamera) -> Option<Self> {
        let (camera, projection) = scene_camera.single().ok()?;
        Some(Self {
            clip_from_world: projection.get_clip_from_view() * camera.to_matrix().inverse(),
            aspect: match projection {
                Projection::Perspective(p) => p.aspect_ratio,
                _ => 1.0,
            },
        })
    }

    /// Where a point lands on screen; `None` behind the camera.
    pub(crate) fn project(&self, at: Vec3) -> Option<Vec2> {
        let clip = self.clip_from_world * at.extend(1.0);
        (clip.w > 0.0).then(|| clip.truncate().truncate() / clip.w * Vec2::new(self.aspect, 1.0))
    }

    /// A place in the view, in the same terms as a projected point.
    pub(crate) fn pointing(&self, in_view: Vec2) -> Vec2 {
        Vec2::new(2.0 * in_view.x - 1.0, 1.0 - 2.0 * in_view.y) * Vec2::new(self.aspect, 1.0)
    }

    /// Whether `in_view` is on a shape's outline on screen.
    pub(crate) fn covers(&self, t: &Transform, shape: &Shape, in_view: Vec2) -> bool {
        // ponytail: the outline is the corners' bounding rectangle, a little
        // generous on a turned cube; use their convex hull if that misfires.
        let corners = (0..8).filter_map(|i| {
            let corner = Vec3::new(
                if i & 1 == 0 { -0.5 } else { 0.5 },
                if i & 2 == 0 { -0.5 } else { 0.5 },
                if i & 4 == 0 { -0.5 } else { 0.5 },
            );
            self.project(t.transform_point(corner * shape.size))
        });
        let (min, max) = corners.fold((Vec2::INFINITY, Vec2::NEG_INFINITY), |(min, max), c| {
            (min.min(c), max.max(c))
        });
        let pointing = self.pointing(in_view);
        pointing.cmpge(min).all() && pointing.cmple(max).all()
    }

    /// The point on screen at `point`, as far from the camera as `depth_of`.
    pub(crate) fn on_sight_line(&self, point: Vec2, depth_of: Vec3) -> Option<Vec3> {
        let clip = self.clip_from_world * depth_of.extend(1.0);
        let ndc = point / Vec2::new(self.aspect, 1.0);
        (clip.w > 0.0).then(|| {
            self.clip_from_world
                .inverse()
                .project_point3(ndc.extend(clip.z / clip.w))
        })
    }

    /// Where the line of sight through `in_view` meets the plane `z = 0`,
    /// which faces the camera through the middle of the scene.
    pub(crate) fn on_middle_plane(&self, in_view: Vec2) -> Option<Vec3> {
        let world_from_clip = self.clip_from_world.inverse();
        let ndc = self.pointing(in_view) / Vec2::new(self.aspect, 1.0);
        // Two depths in front of the camera, finite under Bevy's reversed-z projection.
        let near = world_from_clip.project_point3(ndc.extend(1.0));
        let far = world_from_clip.project_point3(ndc.extend(0.5));
        let along = far - near;
        let t = -near.z / along.z;
        (t.is_finite() && t > 0.0).then(|| near + t * along)
    }
}

/// Where the user points into the scene: each hand's cursor, and the mouse.
#[derive(SystemParam)]
pub(crate) struct Aim<'w, 's> {
    hands: Query<'w, 's, &'static HandCursor>,
    window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    overlay: Res<'w, OverlayArea>,
    scene_camera: SceneCamera<'w, 's>,
}

impl Aim<'_, '_> {
    /// Where the cursors are in the view (0 to 1 across and down from the
    /// top left): each hand's, then the mouse's, leaving out any over the overlay.
    pub(crate) fn cursors(&self) -> Vec<Vec2> {
        let mouse = self
            .window
            .single()
            .ok()
            .and_then(|w| Some(w.cursor_position()? / w.size()));
        self.hands
            .iter()
            .map(|c| c.at)
            .chain(mouse)
            .filter(|&at| !self.overlay.covers(at))
            .collect()
    }

    /// The scene camera's view; `None` without one.
    pub(crate) fn screen(&self) -> Option<Screen> {
        Screen::of(&self.scene_camera)
    }

    /// Where the scene camera is.
    pub(crate) fn camera_at(&self) -> Option<Vec3> {
        Some(self.scene_camera.single().ok()?.0.translation)
    }

    /// Where the first cursor, else the middle of the view, meets the middle
    /// plane; the scene's middle without a camera.
    pub(crate) fn on_middle_plane(&self) -> Vec3 {
        let cursor = self.cursors().first().copied().unwrap_or(Vec2::splat(0.5));
        self.screen()
            .and_then(|s| s.on_middle_plane(cursor))
            .unwrap_or(Vec3::ZERO)
    }
}
