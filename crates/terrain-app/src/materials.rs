//! What shapes are made of: presets that set how they weigh, slide and
//! bounce. `M` gives the selected shape the next preset.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{Grabbable, Selection};

/// One kind of stuff a shape can be made of.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    /// How shapes made of it are drawn.
    pub color: Color,
    /// In kg/m³.
    pub density: f32,
    pub static_friction: f32,
    pub dynamic_friction: f32,
    /// 0 doesn't bounce, 1 bounces back as fast as it came.
    pub restitution: f32,
    /// Carries power, once electricity is built.
    pub conductive: bool,
    /// No effect yet; there for attraction and repulsion later.
    pub charge: f32,
}

/// Every preset. Editing one changes every shape made of it.
#[derive(Resource, Clone, Debug)]
pub struct Materials {
    pub presets: Vec<Preset>,
}

impl Materials {
    pub fn index(&self, name: &str) -> Option<usize> {
        self.presets.iter().position(|p| p.name == name)
    }
}

impl Default for Materials {
    fn default() -> Self {
        let preset =
            |name: &str, color, density, friction: (f32, f32), restitution, conductive| Preset {
                name: name.into(),
                color,
                density,
                static_friction: friction.0,
                dynamic_friction: friction.1,
                restitution,
                conductive,
                charge: 0.0,
            };
        Self {
            presets: vec![
                preset(
                    "glass",
                    Color::srgb(0.35, 0.65, 0.95),
                    2500.0,
                    (0.1, 0.05),
                    0.4,
                    false,
                ),
                preset(
                    "wood",
                    Color::srgb(0.95, 0.6, 0.3),
                    600.0,
                    (0.6, 0.5),
                    0.15,
                    false,
                ),
                preset(
                    "rubber",
                    Color::srgb(0.45, 0.85, 0.5),
                    1100.0,
                    (1.0, 0.9),
                    0.85,
                    false,
                ),
                preset(
                    "plastic",
                    Color::srgb(0.75, 0.5, 0.95),
                    950.0,
                    (0.4, 0.3),
                    0.4,
                    false,
                ),
                preset(
                    "metal",
                    Color::srgb(0.95, 0.85, 0.35),
                    7800.0,
                    (0.4, 0.3),
                    0.2,
                    true,
                ),
            ],
        }
    }
}

/// Which preset a shape is made of, by its place in `Materials`.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Material(pub usize);

impl Default for Material {
    /// Wood.
    fn default() -> Self {
        Self(1)
    }
}

/// `M` moves the selection on to the next preset. Hands each shape's preset
/// to avian when the shape's material, or any preset, changes.
pub(crate) fn apply_materials(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    materials: Res<Materials>,
    selection: Res<Selection>,
    mut shapes: Query<(Entity, &mut Material), With<Grabbable>>,
) {
    if keys.just_pressed(KeyCode::KeyM)
        && let Some((_, mut material)) = selection.0.and_then(|e| shapes.get_mut(e).ok())
    {
        material.0 = (material.0 + 1) % materials.presets.len();
    }
    // Iterating mutably only to read change ticks; nothing is written.
    for (entity, material) in &mut shapes {
        if !(material.is_changed() || materials.is_changed()) {
            continue;
        }
        let Some(p) = materials.presets.get(material.0) else {
            continue;
        };
        commands.entity(entity).insert((
            ColliderDensity(p.density),
            Friction::new(p.dynamic_friction).with_static_coefficient(p.static_friction),
            Restitution::new(p.restitution),
        ));
    }
}
