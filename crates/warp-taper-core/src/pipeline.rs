//! Pipeline orchestrator.
//!
//! Walks the five stages (build → deploy → record → evaluate → bundle)
//! against a [`Scenario`] and a `tape_dir`, killing the deployed process
//! after recording and writing a PR-ready README on completion.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;

use crate::assertion::{run_all, Assertion, AssertionContext, EngineReport};
use crate::bundle::{self, BundleArtifacts, BundleInputs};
use crate::error::{Error, Result};
use crate::log_tail::LogTail;
use crate::recorder::{Recorder, RecordingHandle};
use crate::scenario::Scenario;
use crate::stages::{BuildStage, DeployStage};

/// What stops the recorder.
pub enum RecordTrigger {
    /// Wait for the user to hit Enter on stdin (the interactive flow the
    /// bash pipeline uses).
    Interactive,
    /// Sleep for a fixed duration then stop. Used by tests and headless
    /// scripted runs.
    Duration(Duration),
}

/// All the inputs the orchestrator needs.
pub struct Pipeline {
    pub warp_source: PathBuf,
    pub tape_dir: PathBuf,
    pub scenario: Scenario,
    pub assertions: Vec<Box<dyn Assertion>>,
    pub warp_log_path: PathBuf,
    pub branch: String,
    pub head: String,
    pub package: String,
    pub binary_name: String,
}

impl Pipeline {
    pub fn new(scenario: Scenario, warp_source: PathBuf, tape_dir: PathBuf) -> Self {
        Self {
            scenario,
            warp_source,
            tape_dir,
            assertions: Vec::new(),
            warp_log_path: default_warp_log_path(),
            branch: "<unknown>".to_string(),
            head: "<unknown>".to_string(),
            package: "warp".to_string(),
            binary_name: "warp-oss".to_string(),
        }
    }

    pub fn with_assertions(mut self, assertions: Vec<Box<dyn Assertion>>) -> Self {
        self.assertions = assertions;
        self
    }

    pub fn with_warp_log_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.warp_log_path = path.into();
        self
    }

    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = branch.into();
        self
    }

    pub fn with_head(mut self, head: impl Into<String>) -> Self {
        self.head = head.into();
        self
    }

    pub fn with_package(mut self, package: impl Into<String>) -> Self {
        self.package = package.into();
        self
    }

    pub fn with_binary_name(mut self, binary_name: impl Into<String>) -> Self {
        self.binary_name = binary_name.into();
        self
    }

    /// Run all five stages. The returned [`Tape`] points at the on-disk
    /// bundle (README + artifacts) the recording produced.
    pub fn run<R: Recorder>(&self, recorder: R, trigger: RecordTrigger) -> Result<Tape> {
        let logs_dir = self.tape_dir.join("logs");
        let mcp_logs_dir = logs_dir.join("mcp");
        let session_log = logs_dir.join("warp-oss.session.log");
        let master_mov = self.tape_dir.join("master.mov");

        std::fs::create_dir_all(&mcp_logs_dir).map_err(Error::Io)?;

        // 1. build
        let build_output = BuildStage::new(&self.warp_source)
            .with_package(&self.package)
            .with_binary_name(&self.binary_name)
            .run()?;

        // 2. deploy
        let deploy_handle = DeployStage::new(&build_output.binary_path)
            .capture_output()
            .run()?;

        // 3. record — own the recording in a sub-block so we can always
        //    kill deploy after, regardless of failure.
        let record_inner = (|| -> Result<RecordedArtifacts> {
            let log_tail = LogTail::open(&self.warp_log_path)?;
            let handle = recorder.start(master_mov.clone())?;
            wait_for_trigger(&trigger)?;
            let recording = handle.stop()?;
            let session_bytes = log_tail.slice_since_start(&session_log)?;
            let mcp_log_names = copy_mcp_logs(&self.scenario.mcp_log_paths, &mcp_logs_dir)?;
            Ok(RecordedArtifacts {
                mov_path: recording.path,
                mov_bytes: recording.bytes,
                session_bytes,
                mcp_log_names,
            })
        })();

        let kill_result = deploy_handle.kill();
        let record_artifacts = record_inner?;
        kill_result?;

        // 4. evaluate
        let ctx = AssertionContext::from_tape_dir(&self.tape_dir)
            .with_warp_source(self.warp_source.clone());
        let report = run_all(&self.assertions, &ctx);

        // 5. bundle
        let artifacts = BundleArtifacts {
            has_master_mov: record_artifacts.mov_path.exists(),
            patches: Vec::new(),
            session_log_bytes: Some(record_artifacts.session_bytes),
            mcp_logs: record_artifacts.mcp_log_names,
        };
        let eval_status = if report.passed() { "pass" } else { "fail" };
        let summary_lines = report.summary_lines();
        let inputs = BundleInputs {
            scenario: &self.scenario,
            branch: &self.branch,
            head: &self.head,
            recorded_at: Utc::now(),
            eval_status,
            artifacts: artifacts.clone(),
            stage_logs: Vec::new(),
            assertion_summary: summary_lines,
        };
        let readme_path = self.tape_dir.join("README.md");
        bundle::write_readme(&readme_path, &inputs)?;

        Ok(Tape {
            dir: self.tape_dir.clone(),
            readme_path,
            artifacts,
            evaluation: report,
            recording_bytes: record_artifacts.mov_bytes,
        })
    }
}

#[derive(Debug)]
pub struct Tape {
    pub dir: PathBuf,
    pub readme_path: PathBuf,
    pub artifacts: BundleArtifacts,
    pub evaluation: EngineReport,
    pub recording_bytes: u64,
}

struct RecordedArtifacts {
    mov_path: PathBuf,
    mov_bytes: u64,
    session_bytes: u64,
    mcp_log_names: Vec<String>,
}

fn wait_for_trigger(trigger: &RecordTrigger) -> Result<()> {
    match trigger {
        RecordTrigger::Duration(d) => {
            std::thread::sleep(*d);
            Ok(())
        }
        RecordTrigger::Interactive => {
            eprintln!("Recording. Perform the scenario steps now.");
            eprintln!("Press Enter here when done to finalize the session.");
            let mut buf = String::new();
            std::io::stdin().read_line(&mut buf).map_err(Error::Io)?;
            Ok(())
        }
    }
}

fn copy_mcp_logs(sources: &[PathBuf], dest_dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for src in sources {
        if !src.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(src).map_err(Error::Io)? {
            let entry = entry.map_err(Error::Io)?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let dest = dest_dir.join(&name);
            std::fs::copy(&path, &dest).map_err(Error::Io)?;
            names.push(name);
        }
    }
    Ok(names)
}

fn default_warp_log_path() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join("Library/Logs/warp-oss.log");
    }
    PathBuf::from("warp-oss.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_carry_through_with_chains() {
        let s = Scenario::builder("s").title("t").build().unwrap();
        let p = Pipeline::new(s, PathBuf::from("/x"), PathBuf::from("/y"))
            .with_branch("main")
            .with_head("abc123")
            .with_package("warp")
            .with_binary_name("warp-oss")
            .with_warp_log_path("/tmp/foo");
        assert_eq!(p.branch, "main");
        assert_eq!(p.head, "abc123");
        assert_eq!(p.package, "warp");
        assert_eq!(p.binary_name, "warp-oss");
        assert_eq!(p.warp_log_path, PathBuf::from("/tmp/foo"));
    }

    #[test]
    fn copy_mcp_logs_skips_missing_sources() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("dest");
        std::fs::create_dir_all(&dest).unwrap();
        let names = copy_mcp_logs(&[PathBuf::from("/no/such/dir")], &dest).unwrap();
        assert!(names.is_empty());
        assert!(dest.read_dir().unwrap().next().is_none());
    }

    #[test]
    fn copy_mcp_logs_copies_only_files() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dest = tmp.path().join("dest");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(src.join("a.log"), b"a").unwrap();
        std::fs::write(src.join("b.log"), b"b").unwrap();
        std::fs::create_dir(src.join("subdir")).unwrap();

        let names = copy_mcp_logs(&[src], &dest).unwrap();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["a.log".to_string(), "b.log".to_string()]);
        assert!(dest.join("a.log").is_file());
        assert!(dest.join("b.log").is_file());
        assert!(!dest.join("subdir").exists());
    }
}
