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

use crate::error::Result;

#[cfg(unix)]
pub mod macos;
pub mod noop;

#[cfg(unix)]
pub use macos::{discover_window_for_pid, MacOsScreencapture, MacOsScreencaptureHandle};
pub use noop::{NoOpRecorder, NoOpRecordingHandle};

/// The artifact produced by `Handle::stop()`. Always points at the
/// destination file the recorder wrote.
#[derive(Debug, Clone)]
pub struct RecordingArtifact {
    pub path: PathBuf,
    pub bytes: u64,
    pub duration: Duration,
}

/// Generic recorder interface. Concrete impls expose their own handle
/// types via an associated type, so the pipeline can dispatch at zero
/// runtime cost while tests can swap in [`NoOpRecorder`].
pub trait Recorder {
    type Handle: RecordingHandle;
    fn start(&self, dest: PathBuf) -> Result<Self::Handle>;

    /// Reconfigure the recorder to capture only the given window. Default
    /// is a no-op. The pipeline calls this between deploy and start when
    /// `with_auto_window_id` is enabled and a window has been discovered
    /// for the deployed binary.
    fn set_window_id(&mut self, _window_id: u32) {}
}

pub trait RecordingHandle {
    fn stop(self) -> Result<RecordingArtifact>;
}

impl Recorder for NoOpRecorder {
    type Handle = NoOpRecordingHandle;
    fn start(&self, dest: PathBuf) -> Result<NoOpRecordingHandle> {
        NoOpRecorder::start(self, dest)
    }
}

impl RecordingHandle for NoOpRecordingHandle {
    fn stop(self) -> Result<RecordingArtifact> {
        NoOpRecordingHandle::stop(self)
    }
}

#[cfg(unix)]
impl Recorder for MacOsScreencapture {
    type Handle = MacOsScreencaptureHandle;
    fn start(&self, dest: PathBuf) -> Result<MacOsScreencaptureHandle> {
        MacOsScreencapture::start(self, dest)
    }
    fn set_window_id(&mut self, window_id: u32) {
        *self = std::mem::take(self).with_window_id(window_id);
    }
}

#[cfg(unix)]
impl RecordingHandle for MacOsScreencaptureHandle {
    fn stop(self) -> Result<RecordingArtifact> {
        MacOsScreencaptureHandle::stop(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_via_trait<R: Recorder>(r: R, dest: PathBuf) -> Result<RecordingArtifact> {
        let handle = r.start(dest)?;
        handle.stop()
    }

    #[cfg(unix)]
    #[test]
    fn set_window_id_via_trait_updates_macos_recorder() {
        use crate::recorder::macos::MacOsScreencapture;
        let mut r: MacOsScreencapture = MacOsScreencapture::new().with_region(0, 0, 100, 100);
        assert_eq!(r.window_id(), None);
        // Through the trait.
        Recorder::set_window_id(&mut r, 4242);
        assert_eq!(r.window_id(), Some(4242));
        // Setting window_id should have cleared the region.
        assert_eq!(r.region(), None);
    }

    #[test]
    fn set_window_id_default_impl_is_a_noop_on_noop_recorder() {
        // NoOpRecorder uses the default trait impl: should compile + do
        // nothing observable.
        let mut r = NoOpRecorder::new();
        Recorder::set_window_id(&mut r, 1234);
    }

    #[test]
    fn noop_recorder_round_trips_through_trait_api() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("out.mov");

        let artifact = run_via_trait(NoOpRecorder::new(), dest.clone()).unwrap();
        assert_eq!(artifact.path, dest);
        assert_eq!(artifact.bytes, 0);
        assert!(dest.is_file());
    }
}
