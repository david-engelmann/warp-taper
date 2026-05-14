//! L3 end-to-end: drive the real `warp-taper` CLI against the real Warp
//! checkout at `$WARP_SOURCE`. `#[ignore]` by default — runs locally with:
//!
//! ```sh
//! WARP_SOURCE=$HOME/personal/warp cargo nextest run --run-ignored --package warp-taper-cli
//! ```
//!
//! CI does not run this test; the build cost is too high for default runs.

use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;
use predicates::str::contains;

const METADATA_YAML: &str = "\
title: \"E2E against real warp\"
ticket: \"warpdotdev/warp#0\"
expected: |
  Real warp-oss builds, deploys, and produces a tape.
";

#[test]
#[ignore = "L3 e2e: requires WARP_SOURCE checkout. Run with --run-ignored."]
fn end_to_end_against_real_warp() {
    let warp_source = match std::env::var("WARP_SOURCE") {
        Ok(s) => PathBuf::from(s),
        Err(_) => panic!("WARP_SOURCE not set"),
    };
    assert!(
        warp_source.is_dir(),
        "WARP_SOURCE does not exist: {}",
        warp_source.display()
    );

    let scenario_tmp = tempfile::tempdir().unwrap();
    let tape_tmp = tempfile::tempdir().unwrap();
    fs::write(scenario_tmp.path().join("metadata.yaml"), METADATA_YAML).unwrap();

    Command::cargo_bin("warp-taper")
        .unwrap()
        .arg("run")
        .arg(scenario_tmp.path())
        .arg("--warp-source")
        .arg(&warp_source)
        .arg("--tape-dir")
        .arg(tape_tmp.path())
        .arg("--no-screencapture")
        .arg("--duration-ms")
        .arg("100")
        .timeout(std::time::Duration::from_secs(600))
        .assert()
        .stderr(contains("tape at"));

    assert!(tape_tmp.path().join("README.md").is_file());
}
