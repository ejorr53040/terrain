//! The Workspace seam, Functionality view: Systems, Capabilities and Libraries of a
//! two-crate fixture workspace. Regrouping must never touch a `.rs` file.

use std::fs;
use std::path::Path;

use terrain_blocks::{ItemRef, LibraryKind, MoveError, Workspace};

/// A fresh copy of the fleet fixture workspace in a temp dir.
fn fleet() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fleet"),
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

#[test]
fn open_lists_each_workspace_crate_as_a_system_with_its_path_dependencies() {
    let dir = fleet();
    let ws = Workspace::open(dir.path()).unwrap();

    let systems = ws.systems();
    let names: Vec<_> = systems.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["fleet-node", "fleet-proto"]);
    assert_eq!(systems[0].depends_on, ["fleet-proto"]);
    assert!(systems[1].depends_on.is_empty());
}

/// "name" for Crate dependencies, "name (icon)" for Outside systems.
fn libraries(ws: &Workspace, system: &str) -> Vec<String> {
    let systems = ws.systems();
    let s = systems.iter().find(|s| s.name == system).unwrap();
    s.libraries
        .iter()
        .map(|l| match &l.kind {
            LibraryKind::Crate => l.name.clone(),
            LibraryKind::Outside { icon } => format!("{} ({icon})", l.name),
        })
        .collect()
}

#[test]
fn each_system_shows_its_own_libraries_with_plumbing_hidden_until_pinned() {
    let dir = fleet();
    let mut ws = Workspace::open(dir.path()).unwrap();

    // serde is plumbing, fleet-proto is a System, tempfile is a dev-dependency.
    assert_eq!(
        libraries(&ws, "fleet-node"),
        ["sha2", "tokio", "Firecracker (vm)"]
    );
    assert!(libraries(&ws, "fleet-proto").is_empty());

    let plumbing = |ws: &Workspace| ws.systems()[1].plumbing.clone();
    assert_eq!(
        plumbing(&ws),
        [("serde".to_string(), false)],
        "hidden plumbing is listed for pinning"
    );

    ws.pin("serde", true).unwrap();
    assert_eq!(libraries(&ws, "fleet-proto"), ["serde"]);
    assert_eq!(plumbing(&ws), [("serde".to_string(), true)]);
    assert_eq!(
        libraries(&Workspace::open(dir.path()).unwrap(), "fleet-node"),
        ["serde", "sha2", "tokio", "Firecracker (vm)"]
    );

    ws.pin("serde", false).unwrap();
    assert!(libraries(&ws, "fleet-proto").is_empty());
}

fn item(file: &str, label: &str) -> ItemRef {
    ItemRef {
        file: format!("crates/{file}"),
        label: label.into(),
    }
}

/// Every .rs file's text, by path.
fn sources(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(sources(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push((
                path.display().to_string(),
                fs::read_to_string(&path).unwrap(),
            ));
        }
    }
    out.sort();
    out
}

/// "Capability [labels]" for each Capability, then "Unsorted [labels]".
fn outline(ws: &Workspace, system: &str) -> Vec<String> {
    let systems = ws.systems();
    let s = systems.iter().find(|s| s.name == system).unwrap();
    let labels = |items: &[terrain_blocks::Item]| {
        items
            .iter()
            .map(|i| i.label.clone())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut out: Vec<String> = s
        .capabilities
        .iter()
        .map(|c| format!("{} [{}]", c.name, labels(&c.items)))
        .collect();
    out.push(format!("Unsorted [{}]", labels(&s.unsorted)));
    out
}

#[test]
fn capabilities_are_made_renamed_and_regrouped_items_without_touching_code() {
    let dir = fleet();
    let before = sources(dir.path());
    let mut ws = Workspace::open(dir.path()).unwrap();
    assert_eq!(
        outline(&ws, "fleet-node"),
        ["Unsorted [mod vm, fn serve, fn digest, const PORT, fn boot, fn stop]"]
    );

    let vm = ws
        .new_capability_from(
            &item("fleet-node/src/vm.rs", "fn boot"),
            &item("fleet-node/src/vm.rs", "fn stop"),
        )
        .unwrap();
    assert_eq!(
        outline(&ws, "fleet-node")[0],
        "boot [fn boot, fn stop]",
        "named after the first item"
    );
    ws.rename(vm, "VM lifecycle").unwrap();
    ws.assign(&item("fleet-node/src/lib.rs", "fn serve"), Some(vm))
        .unwrap();
    ws.assign(&item("fleet-node/src/vm.rs", "fn stop"), None)
        .unwrap();
    assert!(
        matches!(
            ws.assign(&item("fleet-proto/src/lib.rs", "fn encode"), Some(vm)),
            Err(MoveError::Invalid(_))
        ),
        "a Capability stays inside its System"
    );

    let reopened = Workspace::open(dir.path()).unwrap();
    assert_eq!(
        outline(&reopened, "fleet-node"),
        [
            "VM lifecycle [fn boot, fn serve]",
            "Unsorted [mod vm, fn digest, const PORT, fn stop]"
        ]
    );
    assert_eq!(
        sources(dir.path()),
        before,
        "regrouping never changes a .rs file"
    );
}

#[test]
fn an_empty_capability_can_be_made_and_removed() {
    let dir = fleet();
    let mut ws = Workspace::open(dir.path()).unwrap();

    let net = ws.new_capability("fleet-node", "Networking").unwrap();
    ws.assign(&item("fleet-node/src/lib.rs", "const PORT"), Some(net))
        .unwrap();
    assert_eq!(outline(&ws, "fleet-node")[0], "Networking [const PORT]");

    ws.remove_capability(net).unwrap();
    assert_eq!(
        outline(&ws, "fleet-node"),
        ["Unsorted [mod vm, fn serve, fn digest, const PORT, fn boot, fn stop]"]
    );
}

fn capability(ws: &Workspace, system: &str, index: usize) -> terrain_blocks::Capability {
    let systems = ws.systems();
    let s = systems.iter().find(|s| s.name == system).unwrap();
    s.capabilities
        .iter()
        .find(|c| c.index == index)
        .unwrap()
        .clone()
}

#[test]
fn a_capability_uses_the_libraries_its_items_name_and_the_outside_systems_its_files_drive() {
    let dir = fleet();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let vm = ws
        .new_capability_from(
            &item("fleet-node/src/vm.rs", "fn boot"),
            &item("fleet-node/src/lib.rs", "fn serve"),
        )
        .unwrap();
    let hashing = ws.new_capability("fleet-node", "Hashing").unwrap();
    ws.assign(&item("fleet-node/src/lib.rs", "fn digest"), Some(hashing))
        .unwrap();

    assert_eq!(
        capability(&ws, "fleet-node", vm).uses,
        ["tokio", "Firecracker"]
    );
    assert_eq!(capability(&ws, "fleet-node", hashing).uses, ["sha2"]);
}

#[test]
fn an_item_renamed_on_disk_goes_back_to_unsorted_and_is_counted_missing_until_dismissed() {
    let dir = fleet();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let vm = ws
        .new_capability_from(
            &item("fleet-node/src/vm.rs", "fn boot"),
            &item("fleet-node/src/vm.rs", "fn stop"),
        )
        .unwrap();
    fs::write(
        dir.path().join("crates/fleet-node/src/vm.rs"),
        "pub fn boot() {}\n\npub fn halt() {}\n",
    )
    .unwrap();
    ws.refresh().unwrap();

    let cap = capability(&ws, "fleet-node", vm);
    assert_eq!((cap.items.len(), cap.missing), (1, 1));
    assert!(
        outline(&ws, "fleet-node")[1].ends_with("fn halt]"),
        "no guessing: halt is a new, unsorted item"
    );

    ws.dismiss_missing(vm).unwrap();
    assert_eq!(
        capability(&Workspace::open(dir.path()).unwrap(), "fleet-node", vm).missing,
        0
    );
}

#[test]
fn system_positions_survive_reopening() {
    let dir = fleet();
    let mut ws = Workspace::open(dir.path()).unwrap();
    ws.place_system("fleet-node", [10.0, 20.0]).unwrap();
    assert_eq!(
        Workspace::open(dir.path())
            .unwrap()
            .system_position("fleet-node"),
        Some([10.0, 20.0])
    );
}

#[test]
fn a_sidecar_saved_by_the_files_view_release_opens_with_its_file_positions() {
    let dir = fleet();
    fs::write(
        dir.path().join(".terrain-blocks.ron"),
        "{\"crates/fleet-node/src/vm.rs\": (3.0, 4.0)}",
    )
    .unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    let vm = ws.find(&["crates/fleet-node/src/vm.rs"]).unwrap();
    assert_eq!(ws.position(&vm), Some([3.0, 4.0]));
}

#[test]
fn a_files_view_move_to_another_file_keeps_the_item_in_its_capability_and_undo_too() {
    let dir = fleet();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let vm = ws
        .new_capability_from(
            &item("fleet-node/src/vm.rs", "fn boot"),
            &item("fleet-node/src/vm.rs", "fn stop"),
        )
        .unwrap();
    let stop = ws
        .find(&["crates/fleet-node/src/vm.rs", "fn stop"])
        .unwrap();
    let lib = ws.find(&["crates/fleet-node/src/lib.rs"]).unwrap();
    let end = ws.block(&lib).unwrap().children.len();
    ws.move_block(&stop, &lib, end).unwrap();

    let cap = capability(&Workspace::open(dir.path()).unwrap(), "fleet-node", vm);
    let files: Vec<_> = cap.items.iter().map(|i| i.file.as_str()).collect();
    assert_eq!(
        (files.as_slice(), cap.missing),
        (
            &[
                "crates/fleet-node/src/vm.rs",
                "crates/fleet-node/src/lib.rs"
            ][..],
            0
        )
    );

    assert!(ws.undo().unwrap());
    let cap = capability(&Workspace::open(dir.path()).unwrap(), "fleet-node", vm);
    assert_eq!(
        (cap.items.len(), cap.missing),
        (2, 0),
        "undo puts the reference back too"
    );
}

#[test]
fn items_in_a_file_with_a_syntax_error_are_not_missing_and_survive_dismiss() {
    let dir = fleet();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let vm = ws
        .new_capability_from(
            &item("fleet-node/src/vm.rs", "fn boot"),
            &item("fleet-node/src/vm.rs", "fn stop"),
        )
        .unwrap();
    let vm_rs = dir.path().join("crates/fleet-node/src/vm.rs");
    let good = fs::read_to_string(&vm_rs).unwrap();
    fs::write(&vm_rs, format!("{good}\nfn half(")).unwrap();
    ws.refresh().unwrap();
    assert_eq!(capability(&ws, "fleet-node", vm).missing, 0);
    ws.dismiss_missing(vm).unwrap();

    fs::write(&vm_rs, good).unwrap();
    ws.refresh().unwrap();
    assert_eq!(outline(&ws, "fleet-node")[0], "boot [fn boot, fn stop]");
}
