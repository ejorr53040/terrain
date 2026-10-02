//! Hand regions: the rotated squares the landmark model looks inside.

use glam::Vec2;

/// Where a hand is: a square in image pixels (x right, y down), turned so
/// the hand points up inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandRegion {
    pub center: Vec2,
    /// Side length, in pixels.
    pub size: f32,
    /// Radians; the region's up (-y) axis is the image's turned by this,
    /// clockwise on screen.
    pub rotation: f32,
    /// Confidence in what produced the region, 0..1.
    pub score: f32,
}

impl HandRegion {
    /// MediaPipe's rect transform: a `size` box at `center`, turned so
    /// `up` points up, moved `shift` box heights toward `up`, squared on
    /// its long side and scaled by `scale`.
    pub(crate) fn around(
        center: Vec2,
        size: Vec2,
        up: Vec2,
        scale: f32,
        shift: f32,
        score: f32,
    ) -> Self {
        // Angle from the region's up (0, -1) to `up`, clockwise on screen.
        let rotation = Vec2::NEG_Y.angle_to(up);
        Self {
            center: center + Vec2::from_angle(rotation).rotate(Vec2::NEG_Y) * size.y * shift,
            size: size.max_element() * scale,
            rotation,
            score,
        }
    }

    /// Whether `point` (image pixels) lies inside the region.
    pub fn contains(&self, point: Vec2) -> bool {
        let local = self.local_point(point);
        local.abs().max_element() <= self.size / 2.0
    }

    /// Corners in image pixels, clockwise on screen from top-left.
    pub fn corners(&self) -> [Vec2; 4] {
        let h = self.size / 2.0;
        [(-h, -h), (h, -h), (h, h), (-h, h)].map(|(x, y)| self.image_point(Vec2::new(x, y)))
    }

    /// Region-local pixels (origin at the center, axes turned with the
    /// region) to image pixels.
    pub(crate) fn image_point(&self, local: Vec2) -> Vec2 {
        self.center + Vec2::from_angle(self.rotation).rotate(local)
    }

    fn local_point(&self, point: Vec2) -> Vec2 {
        Vec2::from_angle(-self.rotation).rotate(point - self.center)
    }

    /// Overlap with `other` as intersection over union, ignoring rotation.
    pub(crate) fn iou(&self, other: &HandRegion) -> f32 {
        let (a, b) = (self.size / 2.0, other.size / 2.0);
        let overlap = ((self.center + a).min(other.center + b)
            - (self.center - a).max(other.center - b))
        .max(Vec2::ZERO);
        let intersection = overlap.x * overlap.y;
        let union = self.size * self.size + other.size * other.size - intersection;
        if union > 0.0 {
            intersection / union
        } else {
            0.0
        }
    }
}
