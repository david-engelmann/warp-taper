//! L2 integration: drive the compiled `warp-taper` binary against a
//! scenario directory + the tiny_warp fixture cargo project.

use std::fs;

use assert_cmd::Command;
use predicates::str::contains;
use warp_taper_fixtures::{tiny_warp_with_behavior, WarpBehavior};

const METADATA_YAML: &str = "\
title: \"CLI smoke\"
ticket: \"warpdotdev/warp#0\"
expected: |
  CLI run subcommand produces a tape with a README.
";

#[test]
#[cfg_attr(not(unix), ignore = "deploy launch is unix-only for now")]
fn run_subcommand_against_tiny_warp_writes_tape_readme() {
    let warp_src_tmp = tempfile::tempdir().unwrap();
    let scenario_tmp = tempfile::tempdir().unwrap();
    let tape_tmp = tempfile::tempdir().unwrap();
    let warp_log_tmp = tempfile::tempdir().unwrap();
    let warp_log_path = warp_log_tmp.path().join("warp-oss.log");
    fs::write(&warp_log_path, b"").unwrap();

    let fixture = tiny_warp_with_behavior(warp_src_tmp.path(), WarpBehavior::LongLived).unwrap();
    fs::write(scenario_tmp.path().join("metadata.yaml"), METADATA_YAML).unwrap();

    Command::cargo_bin("warp-taper")
        .unwrap()
        .arg("run")
        .arg(scenario_tmp.path())
        .arg("--warp-source")
        .arg(fixture.root())
        .arg("--tape-dir")
        .arg(tape_tmp.path())
        .arg("--package")
        .arg(fixture.package_name())
        .arg("--binary-name")
        .arg(fixture.binary_name())
        .arg("--warp-log")
        .arg(&warp_log_path)
        .arg("--no-screencapture")
        .arg("--duration-ms")
        .arg("50")
        .assert()
        .stderr(contains("tape at"));

    assert!(tape_tmp.path().join("README.md").is_file());
    assert!(tape_tmp.path().join("master.mov").is_file());
    let readme = fs::read_to_string(tape_tmp.path().join("README.md")).unwrap();
    assert!(readme.contains("CLI smoke"));
    assert!(readme.contains("warpdotdev/warp#0"));
}

#[test]
fn version_subcommand_prints_version() {
    Command::cargo_bin("warp-taper")
        .unwrap()
        .arg("version")
        .assert()
        .success()
        .stdout(contains("warp-taper"));
}
