//! Loading the hand models into ONNX Runtime.

use std::path::Path;

use ort::session::Session;

/// Threads each model runs a node on. ONNX Runtime's default (one per
/// physical core) is about twice as slow on these small models on a hybrid
/// CPU, where the efficiency cores straggle; four keeps to fast cores and
/// leaves the rest to rendering and capture.
const THREADS: usize = 4;

/// An ONNX Runtime session for `model`. Its threads sleep between runs
/// rather than spin, since frames come only every 33 ms.
pub(crate) fn load(model: impl AsRef<Path>) -> ort::Result<Session> {
    Session::builder()?
        .with_intra_threads(THREADS)?
        .with_intra_op_spinning(false)?
        .commit_from_file(model)
}
