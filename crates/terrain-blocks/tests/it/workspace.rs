//! The Workspace seam: open a crate, move blocks, and assert the exact text on disk.

use std::fs;
use std::path::Path;

use terrain_blocks::{Block, MoveError, Workspace};

/// A fresh copy of the fixture crate in a temp dir.
fn shapes() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shapes"),
        dir.path(),
    );
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dest = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &dest);
        } else {
            fs::copy(entry.path(), dest).unwrap();
        }
    }
}

/// "kind name" labels of a stack, nested stacks in brackets.
fn outline(blocks: &[Block]) -> Vec<String> {
    blocks
        .iter()
        .map(|b| {
            let label = format!("{} {}", b.kind, b.name);
            if b.children.is_empty() {
                label
            } else {
                format!("{label} [{}]", outline(&b.children).join(", "))
            }
        })
        .collect()
}

#[test]
fn open_lists_files_and_their_blocks_in_source_order() {
    let dir = shapes();
    let ws = Workspace::open(dir.path()).unwrap();

    assert_eq!(
        outline(ws.files()),
        [
            "file src/geometry.rs [struct Point, fn origin]",
            "file src/lib.rs [mod geometry, struct Square, impl Square [fn area, fn perimeter], const UNIT, mod util [fn double], fn largest]",
        ]
    );
    let lib = &ws.files()[1];
    assert_eq!(
        lib.children[1].line, 7,
        "Square's block starts at its doc comment"
    );
}

fn read(dir: &tempfile::TempDir, file: &str) -> String {
    fs::read_to_string(dir.path().join(file)).unwrap()
}

#[test]
fn moving_a_method_to_the_top_of_its_impl_reorders_the_file() {
    let dir = shapes();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let perimeter = ws
        .find(&["src/lib.rs", "impl Square", "fn perimeter"])
        .unwrap();
    let square = ws.find(&["src/lib.rs", "impl Square"]).unwrap();

    ws.move_block(&perimeter, &square, 0).unwrap();

    assert!(read(&dir, "src/lib.rs").contains(
        "impl Square {
    pub fn perimeter(&self) -> f32 {
        4.0 * self.side
    }

    /// Area of the square.
    pub fn area(&self) -> f32 {
        self.side * self.side
    }
}
"
    ));
    assert_eq!(
        outline(&ws.files()[1].children)[2],
        "impl Square [fn perimeter, fn area]"
    );
}

#[test]
fn a_struct_moved_to_another_file_takes_its_doc_comment_and_attributes() {
    let dir = shapes();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let square = ws.find(&["src/lib.rs", "struct Square"]).unwrap();
    let geometry = ws.find(&["src/geometry.rs"]).unwrap();

    ws.move_block(&square, &geometry, 2).unwrap();

    assert_eq!(
        read(&dir, "src/geometry.rs"),
        "#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

pub fn origin() -> Point {
    Point { x: 0.0, y: 0.0 }
}

/// A square.
#[derive(Debug, Clone, Copy)]
pub struct Square {
    pub side: f32,
}
"
    );
    assert!(read(&dir, "src/lib.rs").contains(
        "pub use geometry::Point;

impl Square {"
    ));
}

#[test]
fn a_const_moved_into_an_inline_module_takes_its_leading_comment_and_indent() {
    let dir = shapes();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let unit = ws.find(&["src/lib.rs", "const UNIT"]).unwrap();
    let util = ws.find(&["src/lib.rs", "mod util"]).unwrap();

    ws.move_block(&unit, &util, 0).unwrap();

    assert!(read(&dir, "src/lib.rs").contains(
        "}

pub mod util {
    // Shapes are measured in metres.
    pub const UNIT: &str = \"m\";

    pub fn double(x: f32) -> f32 {"
    ));
}

#[test]
fn an_impl_moves_with_all_its_methods() {
    let dir = shapes();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let imp = ws.find(&["src/lib.rs", "impl Square"]).unwrap();
    let geometry = ws.find(&["src/geometry.rs"]).unwrap();

    ws.move_block(&imp, &geometry, 1).unwrap();

    assert!(read(&dir, "src/geometry.rs").contains(
        "}

impl Square {
    /// Area of the square.
    pub fn area(&self) -> f32 {
        self.side * self.side
    }

    pub fn perimeter(&self) -> f32 {
        4.0 * self.side
    }
}

pub fn origin() -> Point {"
    ));
    assert_eq!(
        outline(ws.files()),
        [
            "file src/geometry.rs [struct Point, impl Square [fn area, fn perimeter], fn origin]",
            "file src/lib.rs [mod geometry, struct Square, const UNIT, mod util [fn double], fn largest]",
        ]
    );
}

#[test]
fn a_move_is_refused_and_changes_nothing_if_a_file_it_touches_changed_on_disk() {
    let dir = shapes();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let origin = ws.find(&["src/geometry.rs", "fn origin"]).unwrap();
    let lib = ws.find(&["src/lib.rs"]).unwrap();
    let edited = read(&dir, "src/lib.rs") + "\npub fn added_in_nvim() {}\n";
    fs::write(dir.path().join("src/lib.rs"), &edited).unwrap();
    let geometry = read(&dir, "src/geometry.rs");

    let result = ws.move_block(&origin, &lib, 0);

    assert!(matches!(result, Err(MoveError::Stale(files)) if files == ["src/lib.rs"]));
    assert_eq!(read(&dir, "src/lib.rs"), edited);
    assert_eq!(read(&dir, "src/geometry.rs"), geometry);
}

#[test]
fn undo_backs_out_moves_in_reverse_order_to_the_exact_text() {
    let dir = shapes();
    let original = (read(&dir, "src/lib.rs"), read(&dir, "src/geometry.rs"));
    let mut ws = Workspace::open(dir.path()).unwrap();
    let square = ws.find(&["src/lib.rs", "struct Square"]).unwrap();
    let geometry = ws.find(&["src/geometry.rs"]).unwrap();
    ws.move_block(&square, &geometry, 0).unwrap();
    let after_first = (read(&dir, "src/lib.rs"), read(&dir, "src/geometry.rs"));
    let largest = ws.find(&["src/lib.rs", "fn largest"]).unwrap();
    let lib = ws.find(&["src/lib.rs"]).unwrap();
    ws.move_block(&largest, &lib, 0).unwrap();

    assert!(ws.undo().unwrap());
    assert_eq!(
        (read(&dir, "src/lib.rs"), read(&dir, "src/geometry.rs")),
        after_first
    );
    assert!(ws.undo().unwrap());
    assert_eq!(
        (read(&dir, "src/lib.rs"), read(&dir, "src/geometry.rs")),
        original
    );
    assert!(!ws.undo().unwrap(), "nothing left to undo");
    assert_eq!(
        outline(&ws.files()[0].children),
        ["struct Point", "fn origin"]
    );
}

#[test]
fn refresh_picks_up_edits_and_new_files_from_outside() {
    let dir = shapes();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let edited = read(&dir, "src/geometry.rs")
        + "\npub fn unit() -> Point {\n    Point { x: 1.0, y: 0.0 }\n}\n";
    fs::write(dir.path().join("src/geometry.rs"), edited).unwrap();
    fs::write(dir.path().join("src/area.rs"), "pub type Area = f32;\n").unwrap();

    assert_eq!(ws.refresh().unwrap(), ["src/area.rs", "src/geometry.rs"]);
    assert_eq!(
        outline(&ws.files()[..2]),
        [
            "file src/area.rs [type Area]",
            "file src/geometry.rs [struct Point, fn origin, fn unit]"
        ]
    );
    assert!(ws.refresh().unwrap().is_empty(), "nothing changed since");

    // Moves work on the refreshed text.
    let unit = ws.find(&["src/geometry.rs", "fn unit"]).unwrap();
    let area = ws.find(&["src/area.rs"]).unwrap();
    ws.move_block(&unit, &area, 1).unwrap();
    assert_eq!(
        read(&dir, "src/area.rs"),
        "pub type Area = f32;\n\npub fn unit() -> Point {\n    Point { x: 1.0, y: 0.0 }\n}\n"
    );
}

#[test]
fn an_unparseable_file_is_marked_and_never_written() {
    let dir = shapes();
    let broken = "pub fn half(x: f32) -> f32 {\n    x / 2.0\n";
    fs::write(dir.path().join("src/broken.rs"), broken).unwrap();
    let mut ws = Workspace::open(dir.path()).unwrap();

    let file = ws.find(&["src/broken.rs"]).unwrap();
    assert!(ws.block(&file).unwrap().error);
    let geometry = ws.find(&["src/geometry.rs"]).unwrap();
    assert!(
        !ws.block(&geometry).unwrap().error,
        "the other files still work"
    );
    let origin = ws.find(&["src/geometry.rs", "fn origin"]).unwrap();

    assert!(matches!(
        ws.move_block(&origin, &file, 0),
        Err(MoveError::Invalid(_))
    ));
    assert_eq!(read(&dir, "src/broken.rs"), broken);
}

#[test]
fn check_fails_after_a_move_that_breaks_a_path() {
    let dir = shapes();
    let mut ws = Workspace::open(dir.path()).unwrap();
    assert!(terrain_blocks::cargo_check(ws.root()).unwrap());

    let point = ws.find(&["src/geometry.rs", "struct Point"]).unwrap();
    let lib = ws.find(&["src/lib.rs"]).unwrap();
    ws.move_block(&point, &lib, 0).unwrap();

    assert!(
        !terrain_blocks::cargo_check(ws.root()).unwrap(),
        "geometry.rs no longer sees Point"
    );
}

#[test]
fn file_positions_are_saved_in_the_sidecar_and_survive_reopening() {
    let dir = shapes();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let lib = ws.find(&["src/lib.rs"]).unwrap();
    assert_eq!(ws.position(&lib), None, "a new crate has no layout yet");

    ws.place(&lib, [120.0, -40.5]).unwrap();

    let ws = Workspace::open(dir.path()).unwrap();
    assert_eq!(ws.position(&lib), Some([120.0, -40.5]));
    assert!(dir.path().join(".terrain-blocks.ron").exists());
}
