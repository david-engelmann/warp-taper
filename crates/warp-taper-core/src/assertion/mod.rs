//! Programmatic assertions evaluated after a recording finishes.
//!
//! An [`Assertion`] is a typed check that runs against a tape directory
//! (`AssertionContext`) and emits an [`AssertionResult`]. The engine
//! ([`run_all`]) folds a list of assertions into an [`EngineReport`] with
//! pass/fail/info counts and a formatted summary suitable for embedding in
//! the bundle README.

use std::path::{Path, PathBuf};

pub mod builtins;
pub mod shell;

pub use builtins::{
    DirNotEmpty, FileExists, LogContains, LogLacks, McpLogSnapshotCaptured, McpRotationOccurred,
};
pub use shell::ShellScriptAssertion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
    Info,
}

#[derive(Debug, Clone)]
pub struct AssertionResult {
    pub outcome: Outcome,
    pub message: String,
}

impl AssertionResult {
    pub fn pass(message: impl Into<String>) -> Self {
        Self {
            outcome: Outcome::Pass,
            message: message.into(),
        }
    }

    pub fn fail(message: impl Into<String>) -> Self {
        Self {
            outcome: Outcome::Fail,
            message: message.into(),
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self {
            outcome: Outcome::Info,
            message: message.into(),
        }
    }
}

/// Paths exposed to assertions while running against a recorded tape.
#[derive(Debug, Clone)]
pub struct AssertionContext {
    pub tape_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub session_log: PathBuf,
    pub mcp_logs_dir: PathBuf,
    pub scenario_dir: Option<PathBuf>,
    pub warp_source: Option<PathBuf>,
}

impl AssertionContext {
    /// Build a context with conventional subpaths derived from `tape_dir`.
    pub fn from_tape_dir(tape_dir: impl Into<PathBuf>) -> Self {
        let tape_dir = tape_dir.into();
        let logs_dir = tape_dir.join("logs");
        let session_log = logs_dir.join("warp-oss.session.log");
        let mcp_logs_dir = logs_dir.join("mcp");
        Self {
            tape_dir,
            logs_dir,
            session_log,
            mcp_logs_dir,
            scenario_dir: None,
            warp_source: None,
        }
    }

    pub fn with_scenario_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.scenario_dir = Some(dir.into());
        self
    }

    pub fn with_warp_source(mut self, source: impl Into<PathBuf>) -> Self {
        self.warp_source = Some(source.into());
        self
    }
}

pub trait Assertion: Send + Sync {
    fn name(&self) -> &str;
    fn run(&self, ctx: &AssertionContext) -> AssertionResult;
}

#[derive(Debug, Clone)]
pub struct EngineReport {
    pub results: Vec<NamedResult>,
    pub pass_count: usize,
    pub fail_count: usize,
    pub info_count: usize,
}

#[derive(Debug, Clone)]
pub struct NamedResult {
    pub name: String,
    pub result: AssertionResult,
}

impl EngineReport {
    pub fn passed(&self) -> bool {
        self.fail_count == 0
    }

    /// Render one line per result, formatted as ` ✓ msg` / ` ✗ msg` / ` ⓘ msg`.
    /// If a result's message already contains formatted check lines (e.g. from
    /// a shell-script adapter), those lines are passed through verbatim and
    /// the result itself is not re-emitted.
    pub fn summary_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for nr in &self.results {
            let mut extracted = false;
            for line in nr.result.message.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("✓ ")
                    || trimmed.starts_with("✗ ")
                    || trimmed.starts_with("ⓘ ")
                {
                    out.push(line.to_string());
                    extracted = true;
                }
            }
            if !extracted {
                let mark = match nr.result.outcome {
                    Outcome::Pass => "✓",
                    Outcome::Fail => "✗",
                    Outcome::Info => "ⓘ",
                };
                out.push(format!("  {mark} {}", nr.result.message));
            }
        }
        out
    }
}

pub fn run_all(assertions: &[Box<dyn Assertion>], ctx: &AssertionContext) -> EngineReport {
    let mut results = Vec::with_capacity(assertions.len());
    let mut pass_count = 0;
    let mut fail_count = 0;
    let mut info_count = 0;
    for a in assertions {
        let result = a.run(ctx);
        match result.outcome {
            Outcome::Pass => pass_count += 1,
            Outcome::Fail => fail_count += 1,
            Outcome::Info => info_count += 1,
        }
        results.push(NamedResult {
            name: a.name().to_string(),
            result,
        });
    }
    EngineReport {
        results,
        pass_count,
        fail_count,
        info_count,
    }
}

/// Helper: read a directory and return whether it's non-empty.
pub(crate) fn dir_is_non_empty(path: &Path) -> bool {
    std::fs::read_dir(path)
        .ok()
        .map(|mut it| it.next().is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysPass;
    impl Assertion for AlwaysPass {
        fn name(&self) -> &str {
            "always_pass"
        }
        fn run(&self, _ctx: &AssertionContext) -> AssertionResult {
            AssertionResult::pass("passed")
        }
    }

    struct AlwaysFail;
    impl Assertion for AlwaysFail {
        fn name(&self) -> &str {
            "always_fail"
        }
        fn run(&self, _ctx: &AssertionContext) -> AssertionResult {
            AssertionResult::fail("failed")
        }
    }

    struct AlwaysInfo;
    impl Assertion for AlwaysInfo {
        fn name(&self) -> &str {
            "always_info"
        }
        fn run(&self, _ctx: &AssertionContext) -> AssertionResult {
            AssertionResult::info("informational")
        }
    }

    fn ctx() -> AssertionContext {
        AssertionContext::from_tape_dir("/tmp/fake-tape")
    }

    #[test]
    fn ctx_derives_conventional_subpaths() {
        let c = AssertionContext::from_tape_dir("/foo/bar");
        assert_eq!(c.tape_dir, PathBuf::from("/foo/bar"));
        assert_eq!(c.logs_dir, PathBuf::from("/foo/bar/logs"));
        assert_eq!(
            c.session_log,
            PathBuf::from("/foo/bar/logs/warp-oss.session.log")
        );
        assert_eq!(c.mcp_logs_dir, PathBuf::from("/foo/bar/logs/mcp"));
        assert!(c.scenario_dir.is_none());
        assert!(c.warp_source.is_none());
    }

    #[test]
    fn run_all_counts_outcomes_and_preserves_order() {
        let assertions: Vec<Box<dyn Assertion>> = vec![
            Box::new(AlwaysPass),
            Box::new(AlwaysFail),
            Box::new(AlwaysPass),
            Box::new(AlwaysInfo),
        ];
        let report = run_all(&assertions, &ctx());
        assert_eq!(report.pass_count, 2);
        assert_eq!(report.fail_count, 1);
        assert_eq!(report.info_count, 1);
        assert!(!report.passed());

        let names: Vec<&str> = report.results.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["always_pass", "always_fail", "always_pass", "always_info"]
        );
    }

    #[test]
    fn empty_engine_passes_trivially() {
        let assertions: Vec<Box<dyn Assertion>> = vec![];
        let report = run_all(&assertions, &ctx());
        assert!(report.passed());
        assert_eq!(report.pass_count, 0);
        assert_eq!(report.fail_count, 0);
    }

    #[test]
    fn summary_lines_format_per_outcome() {
        let assertions: Vec<Box<dyn Assertion>> = vec![
            Box::new(AlwaysPass),
            Box::new(AlwaysFail),
            Box::new(AlwaysInfo),
        ];
        let lines = run_all(&assertions, &ctx()).summary_lines();
        assert_eq!(
            lines,
            vec![
                "  ✓ passed".to_string(),
                "  ✗ failed".to_string(),
                "  ⓘ informational".to_string(),
            ]
        );
    }

    #[test]
    fn summary_lines_pass_through_embedded_check_lines() {
        struct EmbeddedCheckLines;
        impl Assertion for EmbeddedCheckLines {
            fn name(&self) -> &str {
                "embedded"
            }
            fn run(&self, _ctx: &AssertionContext) -> AssertionResult {
                AssertionResult::pass("  ✓ check one\n  ✗ check two\nother text")
            }
        }
        let assertions: Vec<Box<dyn Assertion>> = vec![Box::new(EmbeddedCheckLines)];
        let lines = run_all(&assertions, &ctx()).summary_lines();
        assert_eq!(
            lines,
            vec!["  ✓ check one".to_string(), "  ✗ check two".to_string()]
        );
    }
}
