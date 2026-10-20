//! All integration tests in one binary: each tests/*.rs used to link its own
//! copy of the crate's dependencies, which ran the machine out of memory.

mod arena;
mod bounds;
mod build;
mod calibration;
mod cursor;
mod depth_rotation;
mod grab;
mod materials;
mod overlay_pointer;
mod physics;
mod physics_menu;
mod scene;
mod steady;
mod stretch;
mod support;
mod templates;
mod tools;
mod two_cubes;
