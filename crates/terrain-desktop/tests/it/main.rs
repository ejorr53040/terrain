//! All integration tests in one binary: each tests/*.rs used to link its own
//! copy of the crate's dependencies, which ran the machine out of memory.

mod camera_settings;
mod clicks;
mod dictation;
mod engage;
mod full_screen;
mod goldens;
mod herdr;
mod no_stuck_buttons;
mod pointer;
mod scroll;
mod status;
mod support;
mod windows;
