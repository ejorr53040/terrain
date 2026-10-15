//! All integration tests in one binary: each tests/*.rs used to link its own
//! copy of the crate's dependencies, which ran the machine out of memory.

mod support;
mod arena;
mod bounds;
mod calibration;
mod cursor;
mod depth_rotation;
mod grab;
mod physics;
mod steady;
mod stretch;
mod two_cubes;
