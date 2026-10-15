//! Templates: builds saved as a few numbers per shape, kept one RON file
//! each, and stamped back onto the grid. `T` saves the selection's build,
//! `Tab` steps through the library, `P` stamps the current template at the
//! cursor.

use std::{collections::VecDeque, path::PathBuf};

use bevy::{prelude::*, window::PrimaryWindow};
use serde::{Deserialize, Serialize};

use crate::{
    Arena, Grabbable, HandCursor, Material, Materials, Placed, SceneCamera, Screen, Selection,
    Shape,
    build::{cell_of, cells, cursors, middle_of, taken},
};

/// A named build: shapes on grid cells, loose (not glued) once placed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Template {
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub shapes: Vec<Part>,
}

/// One shape of a template.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Part {
    /// Grid cells from the template's first shape.
    pub cell: (i32, i32, i32),
    /// Width, height and depth, in meters.
    pub size: (f32, f32, f32),
    #[serde(default)]
    pub roundness: f32,
    #[serde(default)]
    pub taper: f32,
    /// A quaternion: x, y, z, w.
    #[serde(default = "no_turn")]
    pub rotation: (f32, f32, f32, f32),
    /// A preset's name.
    #[serde(default = "wood")]
    pub material: String,
}

fn no_turn() -> (f32, f32, f32, f32) {
    (0.0, 0.0, 0.0, 1.0)
}

fn wood() -> String {
    "wood".into()
}

/// Every template: the built-in ones, then the user's from `dir`.
#[derive(Resource, Debug)]
pub struct Library {
    pub templates: Vec<Template>,
    /// The one `P` stamps.
    pub current: usize,
    /// Files that couldn't be read, with why.
    pub errors: Vec<String>,
    /// Where saved templates go; `None` keeps them in memory only.
    pub dir: Option<PathBuf>,
}

impl Default for Library {
    fn default() -> Self {
        Self::load(None)
    }
}

impl Library {
    /// The built-in templates and every `*.ron` in `dir`. A file that fails
    /// to parse is skipped and noted in `errors`.
    pub fn load(dir: Option<PathBuf>) -> Self {
        let mut library = Self {
            templates: built_in(),
            current: 0,
            errors: Vec::new(),
            dir,
        };
        let Some(dir) = &library.dir else {
            return library;
        };
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|p| p.extension().is_some_and(|e| e == "ron"))
            .collect();
        files.sort();
        for path in files {
            match std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|text| ron::from_str::<Template>(&text).map_err(|e| e.to_string()))
            {
                Ok(template) => library.templates.push(template),
                Err(e) => library.errors.push(format!("{}: {e}", path.display())),
            }
        }
        for error in &library.errors {
            warn!("skipped template {error}");
        }
        library
    }

    /// Adds `template` and makes it current, writing it to `dir` if there is one.
    fn save(&mut self, template: Template) -> Result<(), String> {
        if let Some(dir) = &self.dir {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let text = ron::ser::to_string_pretty(&template, ron::ser::PrettyConfig::default())
                .map_err(|e| e.to_string())?;
            std::fs::write(dir.join(format!("{}.ron", template.name)), text)
                .map_err(|e| e.to_string())?;
        }
        self.templates.push(template);
        self.current = self.templates.len() - 1;
        Ok(())
    }
}

/// Line, L, a 2×2×2 block, and a pyramid with a pointed top.
fn built_in() -> Vec<Template> {
    let cube = |x, y, z| Part {
        cell: (x, y, z),
        size: (0.15, 0.15, 0.15),
        roundness: 0.0,
        taper: 0.0,
        rotation: no_turn(),
        material: wood(),
    };
    let template = |name: &str, tags: &[&str], shapes| Template {
        name: name.into(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        shapes,
    };
    let mut pyramid: Vec<Part> = (0..3).flat_map(|x| (0..3).map(move |z| cube(x, 0, z))).collect();
    pyramid.push(Part {
        taper: 1.0,
        ..cube(1, 1, 1)
    });
    vec![
        template("line", &["row", "basic"], (0..3).map(|x| cube(x, 0, 0)).collect()),
        template(
            "L",
            &["corner", "basic"],
            vec![cube(0, 0, 0), cube(1, 0, 0), cube(2, 0, 0), cube(0, 1, 0)],
        ),
        template(
            "block",
            &["cube", "basic"],
            (0..8).map(|i| cube(i & 1, (i >> 1) & 1, (i >> 2) & 1)).collect(),
        ),
        template("pyramid", &["roof"], pyramid),
    ]
}

/// `T`, `Tab` and `P`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn use_templates(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    arena: Res<Arena>,
    materials: Res<Materials>,
    selection: Res<Selection>,
    mut library: ResMut<Library>,
    shapes: Query<(Entity, &Transform, &Shape, &Material), With<Grabbable>>,
    hands: Query<&HandCursor>,
    window: Query<&Window, With<PrimaryWindow>>,
    scene_camera: SceneCamera,
) {
    if keys.just_pressed(KeyCode::Tab) && !library.templates.is_empty() {
        library.current = (library.current + 1) % library.templates.len();
    }
    if keys.just_pressed(KeyCode::KeyT)
        && let Some(first) = selection.0.and_then(|e| shapes.get(e).ok())
    {
        // The selection and every shape touching it, and touching those.
        let mut build = vec![first];
        let mut queue = VecDeque::from([first.1.translation]);
        while let Some(at) = queue.pop_front() {
            for shape in &shapes {
                let near = (shape.1.translation - at).abs().max_element() < arena.cell * 1.5;
                if near && build.iter().all(|b| b.0 != shape.0) {
                    build.push(shape);
                    queue.push_back(shape.1.translation);
                }
            }
        }
        let anchor = first.1.translation;
        let mut n = library.templates.len();
        let name = loop {
            let name = format!("build-{n}");
            if library.templates.iter().all(|t| t.name != name) {
                break name;
            }
            n += 1;
        };
        let template = Template {
            name,
            tags: Vec::new(),
            shapes: build
                .iter()
                .map(|(_, t, shape, material)| {
                    let cell = ((t.translation - anchor) / arena.cell).round().as_ivec3();
                    Part {
                        cell: cell.into(),
                        size: shape.size.into(),
                        roundness: shape.roundness,
                        taper: shape.taper,
                        rotation: t.rotation.into(),
                        material: materials
                            .presets
                            .get(material.0)
                            .map_or_else(wood, |p| p.name.clone()),
                    }
                })
                .collect(),
        };
        if let Err(e) = library.save(template) {
            error!("couldn't save the template: {e}");
        }
    }
    if keys.just_pressed(KeyCode::KeyP)
        && let Some(template) = library.templates.get(library.current)
    {
        let cursor = cursors(&hands, &window)
            .first()
            .copied()
            .unwrap_or(Vec2::splat(0.5));
        let at = Screen::of(&scene_camera)
            .and_then(|s| s.on_middle_plane(cursor))
            .unwrap_or(Vec3::ZERO);
        let n = cells(&arena);
        // Moved in from the walls as far as the template needs to fit, if it can.
        let offsets = template.shapes.iter().map(|p| IVec3::from(p.cell));
        let lowest = offsets.clone().fold(IVec3::MAX, IVec3::min);
        let highest = offsets.fold(IVec3::MIN, IVec3::max);
        let anchor = cell_of(&arena, at)
            .min(n - IVec3::ONE - highest)
            .max(-lowest);
        let taken_at: Vec<Vec3> = shapes.iter().map(|(_, t, ..)| t.translation).collect();
        let places: Vec<Option<Vec3>> = template
            .shapes
            .iter()
            .map(|part| {
                let cell = anchor + IVec3::from(part.cell);
                let inside = cell.cmpge(IVec3::ZERO).all() && cell.cmplt(n).all();
                let at = middle_of(&arena, cell);
                (inside && !taken(&arena, at, &taken_at)).then_some(at)
            })
            .collect();
        // All or nothing: no half-stamped builds.
        if taken_at.len() + places.len() <= arena.cap && places.iter().all(Option::is_some) {
            for (part, at) in template.shapes.iter().zip(places.into_iter().flatten()) {
                commands.spawn((
                    Grabbable,
                    Placed,
                    Shape {
                        size: part.size.into(),
                        roundness: part.roundness,
                        taper: part.taper,
                    },
                    materials.index(&part.material).map_or_else(Material::default, Material),
                    Transform::from_translation(at).with_rotation(Quat::from_xyzw(
                        part.rotation.0,
                        part.rotation.1,
                        part.rotation.2,
                        part.rotation.3,
                    )),
                ));
            }
        }
    }
}
