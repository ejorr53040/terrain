//! Templates: saved builds, the built-in ones, and stamping them back in.

use std::path::PathBuf;

use bevy::prelude::*;
use crate::support::*;
use terrain_app::{Arena, Grabbable, Library, Material, Materials, Shape};

/// A fresh, empty folder for one test's templates.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("terrain-templates-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn shapes(h: &mut Harness) -> Vec<(Vec3, Shape, Material)> {
    let world = h.app.world_mut();
    let mut all: Vec<_> = world
        .query_filtered::<(&Transform, &Shape, &Material), With<Grabbable>>()
        .iter(world)
        .map(|(t, s, m)| (t.translation, *s, *m))
        .collect();
    all.sort_by(|a, b| a.0.to_array().partial_cmp(&b.0.to_array()).unwrap());
    all
}

/// Makes `name` the template `P` stamps.
fn pick(h: &mut Harness, name: &str) {
    let mut library = h.app.world_mut().resource_mut::<Library>();
    library.current = library.templates.iter().position(|t| t.name == name).unwrap();
}

#[test]
fn the_built_in_templates_are_there_with_no_files() {
    let library = Library::load(Some(temp_dir("builtins")));
    let names: Vec<_> = library.templates.iter().map(|t| t.name.as_str()).collect();
    for name in ["line", "L", "block", "pyramid"] {
        assert!(names.contains(&name), "{name} missing from {names:?}");
    }
}

#[test]
fn p_stamps_the_current_template_on_the_grid() {
    let mut h = Harness::with_cubes(Vec::new(), &[]);
    pick(&mut h, "line");
    h.run(1).press(KeyCode::KeyP).run(1);
    let made = shapes(&mut h);
    assert_eq!(made.len(), 3, "a line of three");
    let cell = Arena::default().cell;
    for pair in made.windows(2) {
        assert!((pair[1].0 - pair[0].0 - Vec3::X * cell).length() < 1e-4, "{:?}", made);
    }
}

#[test]
fn a_stamp_that_would_overlap_or_pass_the_cap_is_refused() {
    let mut h = Harness::with_cubes(Vec::new(), &[]);
    pick(&mut h, "line");
    h.run(1).press(KeyCode::KeyP).run(1).press(KeyCode::KeyP).run(1);
    assert_eq!(shapes(&mut h).len(), 3, "the second line lands on the first");

    let mut h = Harness::with_cubes(Vec::new(), &[]);
    h.app.world_mut().resource_mut::<Arena>().cap = 7;
    pick(&mut h, "block");
    h.run(1).press(KeyCode::KeyP).run(1);
    assert_eq!(shapes(&mut h).len(), 0, "eight blocks don't fit under a cap of seven");
}

#[test]
fn a_saved_build_comes_back_the_same_from_its_file() {
    let dir = temp_dir("roundtrip");
    let cell = Arena::default().cell;
    // Two touching cubes, one rounded rubber; the hand hovers the left one.
    let mut h = Harness::with_cubes(
        hold(&[Pose::open(Vec3::new(0.0, 0.0, 0.5))], 3 * SETTLE),
        &[Vec3::ZERO, Vec3::new(cell, 0.0, 0.0)],
    );
    h.app.world_mut().insert_resource(Library::load(Some(dir.clone())));
    let rubber = h.app.world().resource::<Materials>().index("rubber").unwrap();
    let world = h.app.world_mut();
    world.entity_mut(h.cubes[1]).insert((
        Material(rubber),
        Shape { roundness: 0.5, ..default() },
    ));
    h.run(SETTLE).press(KeyCode::KeyT).run(1);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "one file saved");

    // A new app reads it back and stamps it.
    let library = Library::load(Some(dir));
    let saved = library.templates.last().unwrap().name.clone();
    let mut fresh = Harness::with_cubes(Vec::new(), &[]);
    fresh.app.world_mut().insert_resource(library);
    pick(&mut fresh, &saved);
    fresh.run(1).press(KeyCode::KeyP).run(1);
    let made = shapes(&mut fresh);
    assert_eq!(made.len(), 2);
    assert!((made[1].0 - made[0].0 - Vec3::X * cell).length() < 1e-4, "kept side by side");
    assert_eq!(made[0].1, Shape::default());
    assert_eq!(made[1].1.roundness, 0.5);
    assert_eq!(made[1].2, Material(rubber));
}

#[test]
fn a_file_that_fails_to_parse_is_skipped_with_a_message() {
    let dir = temp_dir("broken");
    std::fs::write(dir.join("broken.ron"), "(name: \"oops\", shapes: [").unwrap();
    let library = Library::load(Some(dir));
    assert_eq!(library.errors.len(), 1, "{:?}", library.errors);
    assert!(library.errors[0].contains("broken.ron"));
    assert!(library.templates.iter().any(|t| t.name == "line"), "the rest still load");
}

#[test]
fn a_stamp_near_a_wall_moves_in_to_fit() {
    // The middle of the view is only a cell from the back wall; the pyramid is three deep.
    let mut h = Harness::with_cubes(Vec::new(), &[]);
    pick(&mut h, "pyramid");
    h.run(1).press(KeyCode::KeyP).run(1);
    assert_eq!(shapes(&mut h).len(), 10, "the whole pyramid");
}

#[test]
fn search_matches_part_of_a_name_or_tag_ignoring_case() {
    let library = Library::load(None);
    let names = |query: &str| -> Vec<String> {
        library
            .search(query)
            .into_iter()
            .map(|i| library.templates[i].name.clone())
            .collect()
    };
    assert_eq!(names("PYR"), ["pyramid"], "part of a name, any case");
    assert_eq!(names("roo"), ["pyramid"], "part of a tag");
    assert_eq!(names("basic"), ["line", "L", "block"]);
    assert_eq!(names(""), ["line", "L", "block", "pyramid"], "empty lists all");
    assert!(names("zzz").is_empty());
}

#[test]
fn requests_save_and_stamp_like_the_keys() {
    let mut h = Harness::with_cubes(Vec::new(), &[]);
    pick(&mut h, "line");
    h.run(1);
    h.app.world_mut().resource_mut::<terrain_app::Requests>().stamp = true;
    h.run(2);
    assert_eq!(shapes(&mut h).len(), 3, "a requested stamp places the line");
}
