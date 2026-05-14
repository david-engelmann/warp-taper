//! Integration test: synthesize a fake tape directory and run a realistic
//! bundle of builtin assertions against it. Validates the engine's
//! cross-module wiring and the summary-line aggregation.

use std::fs;

use warp_taper_core::assertion::{
    run_all, Assertion, AssertionContext, DirNotEmpty, FileExists, LogContains, LogLacks,
    McpLogSnapshotCaptured, McpRotationOccurred,
};

fn build_fake_tape(dir: &std::path::Path) {
    fs::create_dir_all(dir.join("logs/mcp")).unwrap();
    fs::write(dir.join("master.mov"), b"fake-mov").unwrap();
    fs::write(
        dir.join("logs/warp-oss.session.log"),
        "INFO starting up\nINFO SimpleLogger: rotation completed for foo\nINFO clean exit\n",
    )
    .unwrap();
    fs::write(
        dir.join("logs/mcp/server-uuid.log"),
        b"active server log content",
    )
    .unwrap();
    fs::write(
        dir.join("logs/mcp/server-uuid.log.1"),
        b"rotated server log content",
    )
    .unwrap();
}

#[test]
fn full_bundle_against_synthetic_tape() {
    let tmp = tempfile::tempdir().unwrap();
    let tape = tmp.path();
    build_fake_tape(tape);

    let ctx = AssertionContext::from_tape_dir(tape);

    let assertions: Vec<Box<dyn Assertion>> = vec![
        Box::new(FileExists::new(
            tape.join("master.mov"),
            "screen recording present",
        )),
        Box::new(DirNotEmpty::new(
            ctx.mcp_logs_dir.clone(),
            "MCP logs dir has entries",
        )),
        Box::new(McpLogSnapshotCaptured),
        Box::new(McpRotationOccurred::new()),
        Box::new(
            LogContains::in_session_log(
                r"SimpleLogger: rotation completed",
                "rotation completed line in session log",
            )
            .unwrap(),
        ),
        Box::new(
            LogLacks::in_session_log(
                r"SimpleLogger: rotation failed",
                "no rotation failures in session log",
            )
            .unwrap(),
        ),
    ];

    let report = run_all(&assertions, &ctx);
    assert!(
        report.passed(),
        "report should pass; failures: {:?}",
        report.results
    );
    assert_eq!(report.pass_count, 6);
    assert_eq!(report.fail_count, 0);

    let lines = report.summary_lines();
    assert_eq!(lines.len(), 6);
    assert!(lines.iter().all(|l| l.starts_with("  ✓ ")));
}

#[test]
fn report_fails_when_a_check_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let tape = tmp.path();
    build_fake_tape(tape);

    let ctx = AssertionContext::from_tape_dir(tape);
    let assertions: Vec<Box<dyn Assertion>> = vec![
        Box::new(FileExists::new(tape.join("master.mov"), "mov present")),
        Box::new(FileExists::new(
            tape.join("does-not-exist"),
            "this should fail",
        )),
    ];

    let report = run_all(&assertions, &ctx);
    assert!(!report.passed());
    assert_eq!(report.pass_count, 1);
    assert_eq!(report.fail_count, 1);

    let lines = report.summary_lines();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("  ✓ "));
    assert!(lines[1].starts_with("  ✗ "));
}
