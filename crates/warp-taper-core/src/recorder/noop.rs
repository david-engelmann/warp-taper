//! No-op recorder for tests. Creates an empty file at the destination
//! so consumers can rely on the path existing post-stop, and returns a
//! zero-byte [`RecordingArtifact`].

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::error::{Error, Result};
use crate::recorder::RecordingArtifact;

#[derive(Debug, Default, Clone)]
pub struct NoOpRecorder;

impl NoOpRecorder {
    pub fn new() -> Self {
        Self
    }

    pub fn start(&self, dest: impl Into<PathBuf>) -> Result<NoOpRecordingHandle> {
        let dest = dest.into();
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        std::fs::File::create(&dest).map_err(Error::Io)?;
        Ok(NoOpRecordingHandle {
            dest,
            started_at: Instant::now(),
        })
    }
}

#[derive(Debug)]
pub struct NoOpRecordingHandle {
    dest: PathBuf,
    started_at: Instant,
}

impl NoOpRecordingHandle {
    pub fn dest(&self) -> &Path {
        &self.dest
    }

    pub fn stop(self) -> Result<RecordingArtifact> {
        let bytes = std::fs::metadata(&self.dest).map_err(Error::Io)?.len();
        Ok(RecordingArtifact {
            path: self.dest,
            bytes,
            duration: self.started_at.elapsed(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_creates_dest_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("out.mov");
        assert!(!dest.exists());
        let _handle = NoOpRecorder::new().start(&dest).unwrap();
        assert!(dest.is_file());
    }

    #[test]
    fn start_creates_parent_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("a/b/c/out.mov");
        let _handle = NoOpRecorder::new().start(&dest).unwrap();
        assert!(dest.is_file());
        assert!(dest.parent().unwrap().is_dir());
    }

    #[test]
    fn stop_produces_artifact_with_dest_and_zero_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("out.mov");
        let handle = NoOpRecorder::new().start(&dest).unwrap();
        let artifact = handle.stop().unwrap();
        assert_eq!(artifact.path, dest);
        assert_eq!(artifact.bytes, 0);
        // Duration is some non-negative value; we don't assert >0 because
        // a fast system might report 0 nanoseconds.
        assert!(artifact.duration.as_secs() < 5);
    }

    #[test]
    fn dest_returns_provided_path() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("out.mov");
        let handle = NoOpRecorder::new().start(&dest).unwrap();
        assert_eq!(handle.dest(), dest.as_path());
    }
}
