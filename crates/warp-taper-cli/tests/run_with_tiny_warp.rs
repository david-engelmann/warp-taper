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

#[test]
fn list_builtins_includes_mcp_log_rotation() {
    Command::cargo_bin("warp-taper")
        .unwrap()
        .arg("list-builtins")
        .assert()
        .success()
        .stdout(contains("mcp-log-rotation"));
}

#[test]
fn run_builtin_unknown_name_errors_with_hint() {
    let tmp = tempfile::tempdir().unwrap();
    Command::cargo_bin("warp-taper")
        .unwrap()
        .arg("run-builtin")
        .arg("not-a-real-scenario")
        .arg("--warp-source")
        .arg(tmp.path())
        .arg("--tape-dir")
        .arg(tmp.path().join("tape"))
        .arg("--no-screencapture")
        .arg("--duration-ms")
        .arg("10")
        .assert()
        .failure()
        .stderr(contains("unknown built-in scenario"));
}

#[test]
fn describe_built_in_prints_scenario_metadata() {
    Command::cargo_bin("warp-taper")
        .unwrap()
        .arg("describe")
        .arg("mcp-log-rotation")
        .assert()
        .success()
        .stdout(contains("slug:"))
        .stdout(contains("mcp-log-rotation"))
        .stdout(contains("warpdotdev/warp#10874"))
        .stdout(contains("mcp_rotation_occurred"));
}

#[test]
fn describe_unknown_name_errors() {
    Command::cargo_bin("warp-taper")
        .unwrap()
        .arg("describe")
        .arg("not-real")
        .assert()
        .failure()
        .stderr(contains("unknown built-in scenario"));
}

#[test]
fn init_emits_compilable_starter_module() {
    Command::cargo_bin("warp-taper")
        .unwrap()
        .args([
            "init",
            "12345-fancy-fix",
            "--title",
            "Fancy fix",
            "--ticket",
            "owner/repo#1",
        ])
        .assert()
        .success()
        .stdout(contains("pub fn _12345_fancy_fix()"))
        .stdout(contains("\"12345-fancy-fix\""))
        .stdout(contains("Fancy fix"))
        .stdout(contains(".ticket(\"owner/repo#1\")"));
}

#[test]
#[cfg_attr(not(unix), ignore = "deploy launch is unix-only for now")]
fn run_subcommand_with_branch_and_head_overrides() {
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
        .arg("--branch")
        .arg("override-branch")
        .arg("--head")
        .arg("deadbeef99")
        .assert()
        .success();

    let readme = fs::read_to_string(tape_tmp.path().join("README.md")).unwrap();
    assert!(
        readme.contains("`override-branch` @ `deadbeef99`"),
        "got: {readme}"
    );
}
