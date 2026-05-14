//! Pipeline orchestrator.
//!
//! Walks the five stages (build → deploy → record → evaluate → bundle)
//! against a [`Scenario`] and a `tape_dir`, killing the deployed process
//! after recording and writing a PR-ready README on completion.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::assertion::{run_all, Assertion, AssertionContext, EngineReport};
use crate::bundle::{self, BundleArtifacts, BundleInputs, StageLog};
use crate::error::{Error, Result};
use crate::log_tail::LogTail;
use crate::recorder::{Recorder, RecordingHandle};
use crate::scenario::Scenario;
use crate::stages::{BuildOutput, BuildStage, DeployStage};

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
    pub build_timeout: Option<Duration>,
    /// Called after `cargo build` completes with the path to the produced
    /// binary. Useful for CLI binaries that want to print progress.
    #[allow(clippy::type_complexity)]
    pub on_build_finished: Option<Box<dyn Fn(&Path) + Send + Sync>>,
    /// Called after deploy spawns with the child PID. CLIs use this to wire
    /// the PID into a signal handler so Ctrl-C kills the deployed process.
    #[allow(clippy::type_complexity)]
    pub on_deploy_spawned: Option<Box<dyn Fn(u32) + Send + Sync>>,
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
            build_timeout: None,
            on_build_finished: None,
            on_deploy_spawned: None,
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

    pub fn with_build_timeout(mut self, timeout: Duration) -> Self {
        self.build_timeout = Some(timeout);
        self
    }

    pub fn with_build_finished_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        self.on_build_finished = Some(Box::new(callback));
        self
    }

    pub fn with_deploy_spawned_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(u32) + Send + Sync + 'static,
    {
        self.on_deploy_spawned = Some(Box::new(callback));
        self
    }

    /// Run all five stages. The returned [`Tape`] points at the on-disk
    /// bundle (README + artifacts) the recording produced.
    pub fn run<R: Recorder>(&self, recorder: R, trigger: RecordTrigger) -> Result<Tape> {
        let logs_dir = self.tape_dir.join("logs");
        let mcp_logs_dir = logs_dir.join("mcp");
        let stages_dir = self.tape_dir.join("stages");
        let session_log = logs_dir.join("warp-oss.session.log");
        let master_mov = self.tape_dir.join("master.mov");

        std::fs::create_dir_all(&mcp_logs_dir).map_err(Error::Io)?;
        std::fs::create_dir_all(&stages_dir).map_err(Error::Io)?;

        // 1. build
        let build_started = Utc::now();
        let build_output = BuildStage::new(&self.warp_source)
            .with_package(&self.package)
            .with_binary_name(&self.binary_name)
            .with_timeout_opt(self.build_timeout)
            .run()?;
        write_stage_log(
            &stages_dir,
            "01-build",
            &render_build_log(&build_output, build_started),
        )?;
        if let Some(cb) = &self.on_build_finished {
            cb(&build_output.binary_path);
        }

        // 2. deploy
        //
        // Deliberately do NOT call .capture_output() — we want the deployed
        // binary's stdout/stderr to inherit from the parent so its output
        // is visible to a screen capture and to the user watching the run.
        // (Stage logs still record build output via render_build_log.)
        let deploy_started = Utc::now();
        let deploy_handle = DeployStage::new(&build_output.binary_path).run()?;
        let deploy_pid = deploy_handle.pid();
        if let Some(cb) = &self.on_deploy_spawned {
            cb(deploy_pid);
        }

        // 3. record — own the recording in a sub-block so we can always
        //    kill deploy after, regardless of failure.
        let record_started = Utc::now();
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

        write_stage_log(
            &stages_dir,
            "02-deploy",
            &render_deploy_log(
                &build_output.binary_path,
                deploy_pid,
                deploy_started,
                Utc::now(),
            ),
        )?;
        write_stage_log(
            &stages_dir,
            "03-record",
            &render_record_log(&record_artifacts, record_started),
        )?;

        // 4. evaluate
        let ctx = AssertionContext::from_tape_dir(&self.tape_dir)
            .with_warp_source(self.warp_source.clone());
        let report = run_all(&self.assertions, &ctx);
        let summary_lines = report.summary_lines();
        write_stage_log(
            &stages_dir,
            "04-evaluate",
            &render_evaluate_log(&report, &summary_lines),
        )?;

        // 5. bundle
        let artifacts = BundleArtifacts {
            has_master_mov: record_artifacts.mov_path.exists(),
            patches: Vec::new(),
            session_log_bytes: Some(record_artifacts.session_bytes),
            mcp_logs: record_artifacts.mcp_log_names,
        };
        let eval_status = if report.passed() { "pass" } else { "fail" };
        let stage_logs = collect_stage_logs(&stages_dir, 50)?;
        let inputs = BundleInputs {
            scenario: &self.scenario,
            branch: &self.branch,
            head: &self.head,
            recorded_at: Utc::now(),
            eval_status,
            artifacts: artifacts.clone(),
            stage_logs,
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

fn write_stage_log(stages_dir: &Path, name: &str, content: &str) -> Result<PathBuf> {
    let path = stages_dir.join(format!("{name}.log"));
    std::fs::write(&path, content).map_err(Error::Io)?;
    Ok(path)
}

fn render_build_log(out: &BuildOutput, started: DateTime<Utc>) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "build: cargo build started at {}\n",
        started.format("%Y-%m-%dT%H:%M:%SZ")
    ));
    s.push_str(&format!("build: duration {:?}\n", out.duration));
    s.push_str(&format!(
        "build: produced binary {}\n",
        out.binary_path.display()
    ));
    s.push_str("--- cargo stdout ---\n");
    s.push_str(&String::from_utf8_lossy(&out.stdout));
    if !out.stdout.ends_with(b"\n") {
        s.push('\n');
    }
    s.push_str("--- cargo stderr ---\n");
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    if !out.stderr.ends_with(b"\n") {
        s.push('\n');
    }
    s
}

fn render_deploy_log(
    binary_path: &Path,
    pid: u32,
    started: DateTime<Utc>,
    ended: DateTime<Utc>,
) -> String {
    format!(
        "deploy: spawned {} (pid={}) at {}\ndeploy: killed at {}\n",
        binary_path.display(),
        pid,
        started.format("%Y-%m-%dT%H:%M:%SZ"),
        ended.format("%Y-%m-%dT%H:%M:%SZ"),
    )
}

fn render_record_log(artifacts: &RecordedArtifacts, started: DateTime<Utc>) -> String {
    format!(
        "record: started at {}\nrecord: mov {} ({} bytes)\nrecord: session log slice {} bytes\nrecord: mcp logs copied {}\n",
        started.format("%Y-%m-%dT%H:%M:%SZ"),
        artifacts.mov_path.display(),
        artifacts.mov_bytes,
        artifacts.session_bytes,
        artifacts.mcp_log_names.len(),
    )
}

fn render_evaluate_log(report: &EngineReport, summary: &[String]) -> String {
    let mut s = format!(
        "evaluate: {} pass, {} fail, {} info\n",
        report.pass_count, report.fail_count, report.info_count,
    );
    for line in summary {
        s.push_str(line);
        s.push('\n');
    }
    if report.passed() {
        s.push_str("evaluate: pass\n");
    } else {
        s.push_str("evaluate: FAIL\n");
    }
    s
}

fn collect_stage_logs(stages_dir: &Path, tail_lines: usize) -> Result<Vec<StageLog>> {
    let mut entries: Vec<PathBuf> = match std::fs::read_dir(stages_dir) {
        Ok(it) => it
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("log"))
            .collect(),
        Err(_) => return Ok(Vec::new()),
    };
    entries.sort();

    let mut out = Vec::with_capacity(entries.len());
    for path in entries {
        let content = std::fs::read_to_string(&path).map_err(Error::Io)?;
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(tail_lines);
        let tail = lines[start..].join("\n");
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.log")
            .to_string();
        out.push(StageLog { name, tail });
    }
    Ok(out)
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

    use chrono::TimeZone;

    fn fixed_time() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 14, 4, 12, 7).unwrap()
    }

    #[test]
    fn render_build_log_includes_duration_and_streams() {
        let out = BuildOutput {
            binary_path: PathBuf::from("/tmp/warp-oss"),
            stdout: b"compiled\n".to_vec(),
            stderr: b"warning: nothing\n".to_vec(),
            duration: std::time::Duration::from_millis(1234),
        };
        let s = render_build_log(&out, fixed_time());
        assert!(s.contains("cargo build started at 2026-05-14T04:12:07Z"));
        assert!(s.contains("duration 1.234s"));
        assert!(s.contains("/tmp/warp-oss"));
        assert!(s.contains("compiled"));
        assert!(s.contains("warning: nothing"));
    }

    #[test]
    fn render_build_log_handles_empty_streams_without_trailing_newline() {
        let out = BuildOutput {
            binary_path: PathBuf::from("/tmp/x"),
            stdout: Vec::new(),
            stderr: b"err-without-newline".to_vec(),
            duration: std::time::Duration::ZERO,
        };
        let s = render_build_log(&out, fixed_time());
        assert!(s.contains("err-without-newline"));
        assert!(s.ends_with('\n'));
    }

    #[test]
    fn render_deploy_log_format() {
        let ended = Utc.with_ymd_and_hms(2026, 5, 14, 4, 13, 0).unwrap();
        let s = render_deploy_log(Path::new("/tmp/warp-oss"), 4242, fixed_time(), ended);
        assert!(s.contains("/tmp/warp-oss"));
        assert!(s.contains("pid=4242"));
        assert!(s.contains("2026-05-14T04:12:07Z"));
        assert!(s.contains("2026-05-14T04:13:00Z"));
    }

    #[test]
    fn render_record_log_includes_counts() {
        let artifacts = RecordedArtifacts {
            mov_path: PathBuf::from("/tape/master.mov"),
            mov_bytes: 1024,
            session_bytes: 256,
            mcp_log_names: vec!["a.log".into(), "b.log.1".into()],
        };
        let s = render_record_log(&artifacts, fixed_time());
        assert!(s.contains("/tape/master.mov"));
        assert!(s.contains("(1024 bytes)"));
        assert!(s.contains("256 bytes"));
        assert!(s.contains("mcp logs copied 2"));
    }

    #[test]
    fn render_evaluate_log_pass_and_fail_paths() {
        use crate::assertion::{AssertionResult, NamedResult};

        let pass_report = EngineReport {
            results: vec![NamedResult {
                name: "x".into(),
                result: AssertionResult::pass("ok"),
            }],
            pass_count: 1,
            fail_count: 0,
            info_count: 0,
        };
        let s = render_evaluate_log(&pass_report, &["  ✓ ok".to_string()]);
        assert!(s.contains("evaluate: 1 pass"));
        assert!(s.contains("  ✓ ok"));
        assert!(s.contains("evaluate: pass"));

        let fail_report = EngineReport {
            results: vec![NamedResult {
                name: "y".into(),
                result: AssertionResult::fail("nope"),
            }],
            pass_count: 0,
            fail_count: 1,
            info_count: 0,
        };
        let s = render_evaluate_log(&fail_report, &["  ✗ nope".to_string()]);
        assert!(s.contains("evaluate: 0 pass, 1 fail"));
        assert!(s.contains("evaluate: FAIL"));
    }

    #[test]
    fn collect_stage_logs_returns_empty_for_missing_dir() {
        let result = collect_stage_logs(Path::new("/no/such/dir"), 50).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn collect_stage_logs_tails_lines_in_sorted_order() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("02-deploy.log"), "line1\nline2\nline3\nline4\n").unwrap();
        std::fs::write(dir.join("01-build.log"), "build-line\n").unwrap();
        // Non-.log file: should be skipped.
        std::fs::write(dir.join("README.md"), "noise").unwrap();

        let logs = collect_stage_logs(dir, 2).unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].name, "01-build.log");
        assert_eq!(logs[1].name, "02-deploy.log");
        // Tail of last 2 lines.
        assert_eq!(logs[1].tail, "line3\nline4");
    }

    #[test]
    fn pipeline_callbacks_can_be_set_and_invoked() {
        use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
        use std::sync::Arc;

        let s = Scenario::builder("s").title("t").build().unwrap();
        let build_called = Arc::new(AtomicBool::new(false));
        let deploy_pid = Arc::new(AtomicU32::new(0));
        let bc = build_called.clone();
        let dp = deploy_pid.clone();
        let p = Pipeline::new(s, PathBuf::from("/x"), PathBuf::from("/y"))
            .with_build_finished_callback(move |_p: &Path| {
                bc.store(true, Ordering::SeqCst);
            })
            .with_deploy_spawned_callback(move |pid: u32| {
                dp.store(pid, Ordering::SeqCst);
            })
            .with_build_timeout(std::time::Duration::from_secs(60));

        assert!(p.on_build_finished.is_some());
        assert!(p.on_deploy_spawned.is_some());
        assert_eq!(p.build_timeout, Some(std::time::Duration::from_secs(60)));

        // Exercise the closures directly.
        (p.on_build_finished.as_ref().unwrap())(Path::new("/tmp/x"));
        (p.on_deploy_spawned.as_ref().unwrap())(7777);
        assert!(build_called.load(Ordering::SeqCst));
        assert_eq!(deploy_pid.load(Ordering::SeqCst), 7777);
    }
}
