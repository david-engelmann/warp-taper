//! macOS screen-recording driver backed by `/usr/sbin/screencapture`.
//!
//! `screencapture -v -V <secs> <dest.mov>` opens an interactive video
//! recording session. The user picks a region; recording starts; SIGINT
//! tells screencapture to finalize the .mov cleanly (which matches the
//! Ctrl-C handler the bash pipeline relied on).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Instant;

use crate::error::{Error, Result};
use crate::recorder::RecordingArtifact;

#[derive(Debug, Clone)]
pub struct MacOsScreencapture {
    binary: PathBuf,
    max_duration_seconds: u64,
}

impl Default for MacOsScreencapture {
    fn default() -> Self {
        Self::new()
    }
}

impl MacOsScreencapture {
    pub fn new() -> Self {
        Self {
            binary: PathBuf::from("/usr/sbin/screencapture"),
            max_duration_seconds: 600,
        }
    }

    pub fn with_max_duration_seconds(mut self, secs: u64) -> Self {
        self.max_duration_seconds = secs;
        self
    }

    pub fn with_binary(mut self, p: impl Into<PathBuf>) -> Self {
        self.binary = p.into();
        self
    }

    pub fn binary(&self) -> &Path {
        &self.binary
    }

    pub fn max_duration_seconds(&self) -> u64 {
        self.max_duration_seconds
    }

    /// Build the [`Command`] that would be spawned by [`start`](Self::start).
    /// Exposed for unit tests.
    pub fn command(&self, dest: &Path) -> Command {
        let mut cmd = Command::new(&self.binary);
        cmd.arg("-v");
        cmd.arg("-V").arg(self.max_duration_seconds.to_string());
        cmd.arg(dest);
        cmd
    }

    /// Spawn screencapture. The returned handle owns the running process;
    /// call [`MacOsScreencaptureHandle::stop`] to finalize the recording.
    pub fn start(&self, dest: impl Into<PathBuf>) -> Result<MacOsScreencaptureHandle> {
        let dest = dest.into();
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        // Remove a stale file so the artifact reflects only this run.
        let _ = std::fs::remove_file(&dest);

        let mut cmd = self.command(&dest);
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        let child = cmd.spawn().map_err(Error::Io)?;
        Ok(MacOsScreencaptureHandle {
            dest,
            child,
            started_at: Instant::now(),
        })
    }
}

#[derive(Debug)]
pub struct MacOsScreencaptureHandle {
    dest: PathBuf,
    child: Child,
    started_at: Instant,
}

impl MacOsScreencaptureHandle {
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn dest(&self) -> &Path {
        &self.dest
    }

    /// Signal screencapture to finalize the recording and wait for it to
    /// exit. Sends SIGINT first (screencapture's documented stop signal);
    /// falls back to SIGKILL if signalling fails.
    pub fn stop(mut self) -> Result<RecordingArtifact> {
        let pid = self.child.id() as libc::pid_t;
        // SAFETY: libc::kill is always safe to call with a valid PID and signal.
        let sent = unsafe { libc::kill(pid, libc::SIGINT) };
        if sent != 0 {
            // Could be ESRCH (already exited). Force-kill as a fallback.
            let _ = self.child.kill();
        }
        let status = self.child.wait().map_err(Error::Io)?;
        let duration = self.started_at.elapsed();

        if !self.dest.exists() {
            return Err(Error::StageFailed {
                stage: "record",
                message: format!(
                    "screencapture exited (status={status}) without writing {}",
                    self.dest.display()
                ),
            });
        }
        let bytes = std::fs::metadata(&self.dest).map_err(Error::Io)?.len();
        Ok(RecordingArtifact {
            path: self.dest,
            bytes,
            duration,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_of(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn defaults_target_system_screencapture() {
        let r = MacOsScreencapture::new();
        assert_eq!(r.binary(), Path::new("/usr/sbin/screencapture"));
        assert_eq!(r.max_duration_seconds(), 600);
    }

    #[test]
    fn command_emits_video_flag_and_duration_and_dest() {
        let r = MacOsScreencapture::new().with_max_duration_seconds(120);
        let cmd = r.command(Path::new("/tmp/out.mov"));
        assert_eq!(cmd.get_program(), "/usr/sbin/screencapture");
        assert_eq!(args_of(&cmd), vec!["-v", "-V", "120", "/tmp/out.mov"]);
    }

    #[test]
    fn with_binary_overrides_program() {
        let r = MacOsScreencapture::new().with_binary("/usr/local/bin/screencapture");
        let cmd = r.command(Path::new("/tmp/out.mov"));
        assert_eq!(cmd.get_program(), "/usr/local/bin/screencapture");
    }

    #[test]
    fn start_errors_when_binary_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let r = MacOsScreencapture::new().with_binary("/no/such/screencapture");
        let err = r.start(tmp.path().join("out.mov")).unwrap_err();
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn start_creates_parent_dirs() {
        // We can't really invoke screencapture on Linux CI, but we can verify
        // that start() pre-creates the parent dir before spawning by pointing
        // at a binary that exits immediately on its own (`/bin/true`).
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("a/b/c");
        let dest = nested.join("out.mov");
        assert!(!nested.exists());

        let r = MacOsScreencapture::new().with_binary("/usr/bin/true");
        // /usr/bin/true ignores its args and exits 0 — we just want to prove
        // start() creates parent dirs and spawns without error.
        let handle = r.start(&dest);
        assert!(handle.is_ok(), "start should succeed; got: {handle:?}");
        assert!(nested.is_dir());

        // Reap the child so it doesn't linger as a zombie. The stop() path
        // expects a non-empty dest, but `true` writes nothing — call wait
        // directly via the inner child to be sure.
        let mut handle = handle.unwrap();
        let _ = handle.child.wait();
    }
}
