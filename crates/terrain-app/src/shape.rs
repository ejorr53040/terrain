//! Shapes are parametric primitives (ADR 0004): a size, a roundness and a
//! taper. Their mesh and collider are both built from the same outline, so
//! what you see is what collides.

use std::f32::consts::FRAC_PI_2;

use avian3d::prelude::*;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use crate::CUBE_SIZE;

/// What a grabbable is: a primitive given by parameters, not a mesh. Its
/// collider (and, in the app, its mesh) is rebuilt from them when they
/// change; its `Transform` scale stays 1.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    /// Width, height and depth, in meters.
    pub size: Vec3,
    /// 0 is a box; 1 rounds its upright edges fully, into a cylinder
    /// standing up (a rounded slot if it's wider than deep).
    pub roundness: f32,
    /// 0 is straight-sided; 1 narrows the top to a point (a pyramid or cone).
    pub taper: f32,
}

impl Default for Shape {
    /// A cube one grid cell across.
    fn default() -> Self {
        Self {
            size: Vec3::splat(CUBE_SIZE),
            roundness: 0.0,
            taper: 0.0,
        }
    }
}

/// Points along each rounded corner.
const ARC_POINTS: usize = 8;

impl Shape {
    /// The outline seen from above, around the y axis: each point with its
    /// outward direction, in (x, z).
    fn outline(&self) -> Vec<(Vec2, Vec2)> {
        let half = Vec2::new(self.size.x, self.size.z) / 2.0;
        let radius = self.roundness.clamp(0.0, 1.0) * half.min_element();
        let corners = [
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
        ];
        corners
            .iter()
            .enumerate()
            .flat_map(|(i, &corner)| {
                let centre = corner * (half - radius);
                (0..ARC_POINTS).map(move |j| {
                    let angle =
                        i as f32 * FRAC_PI_2 + j as f32 / (ARC_POINTS - 1) as f32 * FRAC_PI_2;
                    let out = Vec2::from_angle(angle);
                    (centre + radius * out, out)
                })
            })
            .collect()
    }

    /// The outline at the bottom and at the (tapered) top, as 3D rings.
    fn rings(&self) -> (Vec<Vec3>, Vec<Vec3>) {
        let h = self.size.y / 2.0;
        let top = 1.0 - self.taper.clamp(0.0, 1.0);
        let outline = self.outline();
        (
            outline
                .iter()
                .map(|(p, _)| Vec3::new(p.x, -h, p.y))
                .collect(),
            outline
                .iter()
                .map(|(p, _)| Vec3::new(p.x * top, h, p.y * top))
                .collect(),
        )
    }

    /// The shape's surface, for drawing.
    pub fn mesh(&self) -> Mesh {
        let (bottom, top) = self.rings();
        let outline = self.outline();
        let n = outline.len();
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        // Each triangle wound counter-clockwise seen from `outward`.
        let face =
            |indices: &mut Vec<u32>, positions: &[Vec3], [a, b, c]: [u32; 3], outward: Vec3| {
                let [pa, pb, pc] = [a, b, c].map(|i| positions[i as usize]);
                if (pb - pa).cross(pc - pa).dot(outward) >= 0.0 {
                    indices.extend([a, b, c]);
                } else {
                    indices.extend([a, c, b]);
                }
            };
        // Sides: one bottom and one top vertex per outline point, normal
        // across the slope.
        for (i, (_, out)) in outline.iter().enumerate() {
            let along = Vec3::new(-out.y, 0.0, out.x);
            let mut normal = (top[i] - bottom[i]).cross(along).normalize_or(Vec3::Y);
            if normal.dot(Vec3::new(out.x, 0.0, out.y)) < 0.0 {
                normal = -normal;
            }
            positions.extend([bottom[i], top[i]]);
            normals.extend([normal, normal]);
        }
        for i in 0..n {
            let j = (i + 1) % n;
            let (b0, t0, b1, t1) = (
                2 * i as u32,
                2 * i as u32 + 1,
                2 * j as u32,
                2 * j as u32 + 1,
            );
            let mid = (positions[b0 as usize] + positions[b1 as usize]) / 2.0;
            let outward = Vec3::new(mid.x, 0.0, mid.z);
            face(&mut indices, &positions, [b0, b1, t1], outward);
            face(&mut indices, &positions, [b0, t1, t0], outward);
        }
        // Flat ends, as fans from their middles.
        for (ring, normal) in [(&bottom, Vec3::NEG_Y), (&top, Vec3::Y)] {
            let middle = positions.len() as u32;
            positions.push(Vec3::new(0.0, ring[0].y, 0.0));
            normals.push(normal);
            positions.extend(ring.iter().copied());
            normals.extend(std::iter::repeat_n(normal, n));
            for i in 0..n as u32 {
                let j = (i + 1) % n as u32;
                face(
                    &mut indices,
                    &positions,
                    [middle, middle + 1 + i, middle + 1 + j],
                    normal,
                );
            }
        }
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices))
    }

    /// What it collides as: avian's own box or cylinder where one fits,
    /// otherwise the convex hull of the drawn outline.
    pub fn collider(&self) -> Collider {
        let s = self.size;
        let square = (s.x - s.z).abs() < 1e-4;
        match (self.roundness, self.taper) {
            (r, t) if r <= 0.0 && t <= 0.0 => Collider::cuboid(s.x, s.y, s.z),
            (r, t) if r >= 1.0 && t <= 0.0 && square => Collider::cylinder(s.x / 2.0, s.y),
            _ => {
                let (bottom, top) = self.rings();
                Collider::convex_hull(bottom.into_iter().chain(top).collect())
                    .unwrap_or_else(|| Collider::cuboid(s.x, s.y, s.z))
            }
        }
    }
}
