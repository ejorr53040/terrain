//! All integration tests in one binary: each tests/*.rs used to link its own
//! copy of the crate's dependencies, which ran the machine out of memory.

mod support;
mod hand_landmarks;
mod palm_detection;
