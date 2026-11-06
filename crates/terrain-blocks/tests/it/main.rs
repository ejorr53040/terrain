//! All integration tests in one binary: each tests/*.rs used to link its own
//! copy of the crate's dependencies, which ran the machine out of memory.

mod systems;
mod workspace;

use std::{fs, path::Path};

/// Copies the directory tree `from` to `to`.
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
