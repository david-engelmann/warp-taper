//! L2 integration: end-to-end pipeline run against the tiny_warp fixture
//! with a no-op recorder. Exercises build → deploy → record → evaluate →
//! bundle in one shot and verifies the produced tape directory.

use std::path::PathBuf;
use std::time::Duration;

use warp_taper_core::{
    assertion::{FileExists, McpLogSnapshotCaptured},
    Assertion, NoOpRecorder, Pipeline, RecordTrigger, Scenario,
};
use warp_taper_fixtures::{tiny_warp_with_behavior, WarpBehavior};

#[test]
#[cfg_attr(not(unix), ignore = "deploy launch is unix-only for now")]
fn full_pipeline_runs_against_tiny_warp() {
    let warp_src_tmp = tempfile::tempdir().unwrap();
    let tape_tmp = tempfile::tempdir().unwrap();
    let warp_log_tmp = tempfile::tempdir().unwrap();
    let warp_log_path = warp_log_tmp.path().join("warp-oss.log");
    std::fs::write(&warp_log_path, b"INITIAL\n").unwrap();

    let fixture = tiny_warp_with_behavior(warp_src_tmp.path(), WarpBehavior::LongLived).unwrap();

    let scenario = Scenario::builder("test-scenario")
        .title("Pipeline smoke")
        .ticket("warpdotdev/warp#0")
        .expected("Pipeline runs end-to-end against the tiny_warp fixture.")
        .build()
        .unwrap();

    let assertions: Vec<Box<dyn Assertion>> = vec![
        Box::new(FileExists::new(
            tape_tmp.path().join("master.mov"),
            "master.mov present",
        )),
        Box::new(McpLogSnapshotCaptured),
    ];

    let pipeline = Pipeline::new(
        scenario,
        fixture.root().to_path_buf(),
        tape_tmp.path().to_path_buf(),
    )
    .with_assertions(assertions)
    .with_warp_log_path(&warp_log_path)
    .with_branch("test-branch")
    .with_head("deadbeef")
    .with_package(fixture.package_name())
    .with_binary_name(fixture.binary_name());

    let tape = pipeline
        .run(
            NoOpRecorder::new(),
            RecordTrigger::Duration(Duration::from_millis(50)),
        )
        .unwrap_or_else(|e| panic!("pipeline failed: {e}"));

    // README was written.
    assert!(tape.readme_path.is_file(), "README.md missing");
    let readme = std::fs::read_to_string(&tape.readme_path).unwrap();
    assert!(readme.contains("# Tape: Pipeline smoke"));
    assert!(readme.contains("warpdotdev/warp#0"));
    assert!(readme.contains("`test-branch` @ `deadbeef`"));

    // logs/ + master.mov exist.
    assert!(tape.dir.join("master.mov").is_file());
    assert!(tape.dir.join("logs/warp-oss.session.log").is_file());
    assert!(tape.dir.join("logs/mcp").is_dir());

    // Stage logs were written and embedded in the README.
    let stages_dir = tape.dir.join("stages");
    assert!(stages_dir.is_dir(), "stages/ dir missing");
    assert!(stages_dir.join("01-build.log").is_file());
    assert!(stages_dir.join("02-deploy.log").is_file());
    assert!(stages_dir.join("03-record.log").is_file());
    assert!(stages_dir.join("04-evaluate.log").is_file());
    assert!(readme.contains("## Stages"));
    assert!(readme.contains("01-build.log"));
    assert!(readme.contains("04-evaluate.log"));

    // The mcp snapshot is empty (scenario has no mcp_log_paths) so that
    // assertion fails; the master.mov assertion passes. So overall fails.
    assert_eq!(tape.evaluation.pass_count, 1);
    assert_eq!(tape.evaluation.fail_count, 1);
}

#[test]
fn pipeline_errors_when_warp_source_missing() {
    let scenario = Scenario::builder("test").title("t").build().unwrap();
    let tape_tmp = tempfile::tempdir().unwrap();
    let pipeline = Pipeline::new(
        scenario,
        PathBuf::from("/no/such/warp-source"),
        tape_tmp.path().to_path_buf(),
    );
    let err = pipeline
        .run(NoOpRecorder::new(), RecordTrigger::Duration(Duration::ZERO))
        .unwrap_err();
    let rendered = format!("{err}");
    assert!(rendered.contains("build"), "got: {rendered}");
}
