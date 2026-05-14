//! Built-in assertions covering the checks the bash `assertions.sh` script
//! performs for the reference 10874 scenario, plus a few generic helpers.

use std::path::PathBuf;

use regex::Regex;

use crate::assertion::{dir_is_non_empty, Assertion, AssertionContext, AssertionResult};
use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct FileExists {
    path: PathBuf,
    description: String,
}

impl FileExists {
    pub fn new(path: impl Into<PathBuf>, description: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            description: description.into(),
        }
    }
}

impl Assertion for FileExists {
    fn name(&self) -> &str {
        "file_exists"
    }

    fn run(&self, _ctx: &AssertionContext) -> AssertionResult {
        if self.path.exists() {
            AssertionResult::pass(self.description.clone())
        } else {
            AssertionResult::fail(format!(
                "{} (missing: {})",
                self.description,
                self.path.display()
            ))
        }
    }
}

#[derive(Debug, Clone)]
pub struct DirNotEmpty {
    path: PathBuf,
    description: String,
}

impl DirNotEmpty {
    pub fn new(path: impl Into<PathBuf>, description: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            description: description.into(),
        }
    }
}

impl Assertion for DirNotEmpty {
    fn name(&self) -> &str {
        "dir_not_empty"
    }

    fn run(&self, _ctx: &AssertionContext) -> AssertionResult {
        if !self.path.is_dir() {
            return AssertionResult::fail(format!(
                "{} (directory missing: {})",
                self.description,
                self.path.display()
            ));
        }
        if dir_is_non_empty(&self.path) {
            AssertionResult::pass(self.description.clone())
        } else {
            AssertionResult::fail(format!(
                "{} (empty: {})",
                self.description,
                self.path.display()
            ))
        }
    }
}

/// Selector for which file an assertion reads.
#[derive(Debug, Clone)]
enum LogTarget {
    Absolute(PathBuf),
    SessionLog,
}

impl LogTarget {
    fn resolve(&self, ctx: &AssertionContext) -> PathBuf {
        match self {
            Self::Absolute(p) => p.clone(),
            Self::SessionLog => ctx.session_log.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogContains {
    target: LogTarget,
    pattern: Regex,
    description: String,
}

impl LogContains {
    pub fn in_file(
        path: impl Into<PathBuf>,
        pattern: &str,
        description: impl Into<String>,
    ) -> Result<Self> {
        let pattern = Regex::new(pattern)
            .map_err(|e| Error::AssertionConfig(format!("invalid regex {pattern:?}: {e}")))?;
        Ok(Self {
            target: LogTarget::Absolute(path.into()),
            pattern,
            description: description.into(),
        })
    }

    pub fn in_session_log(pattern: &str, description: impl Into<String>) -> Result<Self> {
        let pattern = Regex::new(pattern)
            .map_err(|e| Error::AssertionConfig(format!("invalid regex {pattern:?}: {e}")))?;
        Ok(Self {
            target: LogTarget::SessionLog,
            pattern,
            description: description.into(),
        })
    }
}

impl Assertion for LogContains {
    fn name(&self) -> &str {
        "log_contains"
    }

    fn run(&self, ctx: &AssertionContext) -> AssertionResult {
        let path = self.target.resolve(ctx);
        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                return AssertionResult::fail(format!(
                    "{} (cannot read {}: {e})",
                    self.description,
                    path.display()
                ));
            }
        };
        if self.pattern.is_match(&content) {
            AssertionResult::pass(self.description.clone())
        } else {
            AssertionResult::fail(format!("{} (pattern not found)", self.description))
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogLacks {
    target: LogTarget,
    pattern: Regex,
    description: String,
}

impl LogLacks {
    pub fn in_file(
        path: impl Into<PathBuf>,
        pattern: &str,
        description: impl Into<String>,
    ) -> Result<Self> {
        let pattern = Regex::new(pattern)
            .map_err(|e| Error::AssertionConfig(format!("invalid regex {pattern:?}: {e}")))?;
        Ok(Self {
            target: LogTarget::Absolute(path.into()),
            pattern,
            description: description.into(),
        })
    }

    pub fn in_session_log(pattern: &str, description: impl Into<String>) -> Result<Self> {
        let pattern = Regex::new(pattern)
            .map_err(|e| Error::AssertionConfig(format!("invalid regex {pattern:?}: {e}")))?;
        Ok(Self {
            target: LogTarget::SessionLog,
            pattern,
            description: description.into(),
        })
    }
}

impl Assertion for LogLacks {
    fn name(&self) -> &str {
        "log_lacks"
    }

    fn run(&self, ctx: &AssertionContext) -> AssertionResult {
        let path = self.target.resolve(ctx);
        // Missing file vacuously satisfies "lacks pattern".
        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return AssertionResult::pass(self.description.clone());
            }
            Err(e) => {
                return AssertionResult::fail(format!(
                    "{} (cannot read {}: {e})",
                    self.description,
                    path.display()
                ));
            }
        };
        if self.pattern.is_match(&content) {
            AssertionResult::fail(format!("{} (pattern matched)", self.description))
        } else {
            AssertionResult::pass(self.description.clone())
        }
    }
}

/// Assert that the MCP log snapshot directory is non-empty (i.e., the
/// `mcp_log_paths` declared in the scenario were copied into the tape).
#[derive(Debug, Clone, Default)]
pub struct McpLogSnapshotCaptured;

impl Assertion for McpLogSnapshotCaptured {
    fn name(&self) -> &str {
        "mcp_log_snapshot_captured"
    }

    fn run(&self, ctx: &AssertionContext) -> AssertionResult {
        if !ctx.mcp_logs_dir.is_dir() {
            return AssertionResult::fail(format!(
                "MCP log snapshot missing ({})",
                ctx.mcp_logs_dir.display()
            ));
        }
        let count = std::fs::read_dir(&ctx.mcp_logs_dir)
            .map(|it| it.filter_map(|e| e.ok()).count())
            .unwrap_or(0);
        if count > 0 {
            AssertionResult::pass(format!("MCP log snapshot captured ({count} files)"))
        } else {
            AssertionResult::fail("MCP log snapshot is empty".to_string())
        }
    }
}

/// Assert that at least one rotated MCP log file (`*.log.N`) exists in the
/// snapshot. Proves the rotation event fired during the recording.
#[derive(Debug, Clone)]
pub struct McpRotationOccurred {
    description: String,
}

impl McpRotationOccurred {
    pub fn new() -> Self {
        Self {
            description: "rotated MCP log file present".to_string(),
        }
    }

    pub fn with_description(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
        }
    }
}

impl Default for McpRotationOccurred {
    fn default() -> Self {
        Self::new()
    }
}

impl Assertion for McpRotationOccurred {
    fn name(&self) -> &str {
        "mcp_rotation_occurred"
    }

    fn run(&self, ctx: &AssertionContext) -> AssertionResult {
        if !ctx.mcp_logs_dir.is_dir() {
            return AssertionResult::fail(format!(
                "{} (MCP logs dir missing: {})",
                self.description,
                ctx.mcp_logs_dir.display()
            ));
        }
        let pattern = Regex::new(r"\.log\.\d+$").unwrap();
        let count = std::fs::read_dir(&ctx.mcp_logs_dir)
            .map(|it| {
                it.filter_map(|e| e.ok())
                    .filter(|e| {
                        e.file_name()
                            .to_str()
                            .map(|name| pattern.is_match(name))
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0);
        if count > 0 {
            AssertionResult::pass(format!("{count} rotated MCP log file(s) present"))
        } else {
            AssertionResult::fail(format!("{} (no .log.N files found)", self.description))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tape_with_logs() -> (tempfile::TempDir, AssertionContext) {
        let tmp = tempfile::tempdir().unwrap();
        let tape = tmp.path().to_path_buf();
        fs::create_dir_all(tape.join("logs/mcp")).unwrap();
        let ctx = AssertionContext::from_tape_dir(&tape);
        (tmp, ctx)
    }

    #[test]
    fn file_exists_passes_when_path_present() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("hello");
        fs::write(&p, b"x").unwrap();
        let r =
            FileExists::new(&p, "hello exists").run(&AssertionContext::from_tape_dir(tmp.path()));
        assert_eq!(r.outcome, crate::assertion::Outcome::Pass);
    }

    #[test]
    fn file_exists_fails_when_path_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let r = FileExists::new(tmp.path().join("nope"), "thing")
            .run(&AssertionContext::from_tape_dir(tmp.path()));
        assert_eq!(r.outcome, crate::assertion::Outcome::Fail);
        assert!(r.message.contains("missing"));
    }

    #[test]
    fn dir_not_empty_passes_with_entries() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a"), b"x").unwrap();
        let r = DirNotEmpty::new(tmp.path(), "tape has entries")
            .run(&AssertionContext::from_tape_dir(tmp.path()));
        assert_eq!(r.outcome, crate::assertion::Outcome::Pass);
    }

    #[test]
    fn dir_not_empty_fails_when_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let r = DirNotEmpty::new(tmp.path(), "tape has entries")
            .run(&AssertionContext::from_tape_dir(tmp.path()));
        assert_eq!(r.outcome, crate::assertion::Outcome::Fail);
        assert!(r.message.contains("empty"));
    }

    #[test]
    fn dir_not_empty_fails_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let r = DirNotEmpty::new(tmp.path().join("nope"), "dir should be there")
            .run(&AssertionContext::from_tape_dir(tmp.path()));
        assert_eq!(r.outcome, crate::assertion::Outcome::Fail);
    }

    #[test]
    fn log_contains_passes_on_match() {
        let (_tmp, ctx) = tape_with_logs();
        fs::write(&ctx.session_log, "INFO foo\nWARN something bad\n").unwrap();
        let a = LogContains::in_session_log("WARN", "session has WARN").unwrap();
        let r = a.run(&ctx);
        assert_eq!(r.outcome, crate::assertion::Outcome::Pass);
    }

    #[test]
    fn log_contains_fails_on_no_match() {
        let (_tmp, ctx) = tape_with_logs();
        fs::write(&ctx.session_log, "INFO foo\n").unwrap();
        let a = LogContains::in_session_log("WARN", "session has WARN").unwrap();
        let r = a.run(&ctx);
        assert_eq!(r.outcome, crate::assertion::Outcome::Fail);
    }

    #[test]
    fn log_contains_fails_when_file_missing() {
        let (_tmp, ctx) = tape_with_logs();
        let a = LogContains::in_session_log("anything", "should have file").unwrap();
        let r = a.run(&ctx);
        assert_eq!(r.outcome, crate::assertion::Outcome::Fail);
    }

    #[test]
    fn log_contains_rejects_invalid_regex() {
        let err = LogContains::in_session_log("[unterminated", "x").unwrap_err();
        assert!(matches!(err, Error::AssertionConfig(_)));
    }

    #[test]
    fn log_lacks_passes_when_pattern_absent() {
        let (_tmp, ctx) = tape_with_logs();
        fs::write(&ctx.session_log, "INFO foo\n").unwrap();
        let a = LogLacks::in_session_log("rotation failed", "no rotation failures").unwrap();
        let r = a.run(&ctx);
        assert_eq!(r.outcome, crate::assertion::Outcome::Pass);
    }

    #[test]
    fn log_lacks_fails_when_pattern_present() {
        let (_tmp, ctx) = tape_with_logs();
        fs::write(&ctx.session_log, "WARN rotation failed for ...\n").unwrap();
        let a = LogLacks::in_session_log("rotation failed", "no rotation failures").unwrap();
        let r = a.run(&ctx);
        assert_eq!(r.outcome, crate::assertion::Outcome::Fail);
    }

    #[test]
    fn log_lacks_passes_when_file_missing() {
        // Missing file vacuously lacks any pattern — match grep's "no
        // matches" semantics rather than treating absence as failure.
        let (_tmp, ctx) = tape_with_logs();
        let a = LogLacks::in_session_log("anything", "should pass").unwrap();
        let r = a.run(&ctx);
        assert_eq!(r.outcome, crate::assertion::Outcome::Pass);
    }

    #[test]
    fn mcp_log_snapshot_passes_with_files() {
        let (_tmp, ctx) = tape_with_logs();
        fs::write(ctx.mcp_logs_dir.join("a.log"), b"x").unwrap();
        fs::write(ctx.mcp_logs_dir.join("b.log"), b"y").unwrap();
        let r = McpLogSnapshotCaptured.run(&ctx);
        assert_eq!(r.outcome, crate::assertion::Outcome::Pass);
        assert!(r.message.contains("2 files"));
    }

    #[test]
    fn mcp_log_snapshot_fails_when_empty() {
        let (_tmp, ctx) = tape_with_logs();
        let r = McpLogSnapshotCaptured.run(&ctx);
        assert_eq!(r.outcome, crate::assertion::Outcome::Fail);
    }

    #[test]
    fn mcp_rotation_passes_when_rotated_file_present() {
        let (_tmp, ctx) = tape_with_logs();
        fs::write(ctx.mcp_logs_dir.join("server-uuid.log"), b"active").unwrap();
        fs::write(ctx.mcp_logs_dir.join("server-uuid.log.1"), b"rotated").unwrap();
        let r = McpRotationOccurred::new().run(&ctx);
        assert_eq!(r.outcome, crate::assertion::Outcome::Pass);
        assert!(r.message.contains("1"));
    }

    #[test]
    fn mcp_rotation_fails_when_no_rotation() {
        let (_tmp, ctx) = tape_with_logs();
        fs::write(ctx.mcp_logs_dir.join("server-uuid.log"), b"active").unwrap();
        let r = McpRotationOccurred::new().run(&ctx);
        assert_eq!(r.outcome, crate::assertion::Outcome::Fail);
    }

    #[test]
    fn mcp_rotation_fails_when_mcp_dir_missing() {
        let tmp = tempfile::tempdir().unwrap();
        // Note: tape_dir's logs/mcp does NOT exist.
        let ctx = AssertionContext::from_tape_dir(tmp.path());
        let r = McpRotationOccurred::new().run(&ctx);
        assert_eq!(r.outcome, crate::assertion::Outcome::Fail);
        assert!(r.message.contains("missing"));
    }
}
