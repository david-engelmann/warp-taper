//! Screen recording.
//!
//! Two implementations:
//!
//! - [`MacOsScreencapture`] drives the system `screencapture` binary in
//!   video mode (`-v`). Real recording; needs screen-recording permission.
//! - [`NoOpRecorder`] creates an empty file at the destination and returns a
//!   zero-byte artifact. Used by tests that exercise the rest of the
//!   pipeline without spawning a real recorder.
//!
//! Each recorder exposes a `start(dest) -> Handle` method; the handle owns
//! the active recording and finalizes it via `Handle::stop() -> RecordingArtifact`.
//! A unified `Recorder` trait will be introduced alongside the pipeline
//! orchestrator in P5; until then, both concrete recorders share the same
//! method shape for ergonomic swapping in tests.

use std::path::PathBuf;
use std::time::Duration;

#[cfg(unix)]
pub mod macos;
pub mod noop;

#[cfg(unix)]
pub use macos::{MacOsScreencapture, MacOsScreencaptureHandle};
pub use noop::{NoOpRecorder, NoOpRecordingHandle};

/// The artifact produced by `Handle::stop()`. Always points at the
/// destination file the recorder wrote.
#[derive(Debug, Clone)]
pub struct RecordingArtifact {
    pub path: PathBuf,
    pub bytes: u64,
    pub duration: Duration,
}
