//! End-to-end exercise of the build + deploy stages against the
//! `tiny_warp` cargo fixture. Compiles a real binary with real cargo and
//! launches it; deliberately scoped to a single test so the cargo-build
//! cost (~1s) hits the test suite once.

use warp_taper_core::{BuildStage, DeployStage};
use warp_taper_fixtures::tiny_warp;

#[test]
#[cfg_attr(not(unix), ignore = "deploy launch path is unix-only for now")]
fn build_then_deploy_tiny_warp_fixture() {
    let tmp = tempfile::tempdir().unwrap();
    let fixture = tiny_warp(tmp.path()).unwrap();

    let build = BuildStage::new(fixture.root())
        .with_package(fixture.package_name())
        .with_binary_name(fixture.binary_name());

    let output = build.run().unwrap_or_else(|e| panic!("build failed: {e}"));

    assert!(
        output.binary_path.exists(),
        "binary should exist at {}",
        output.binary_path.display()
    );
    assert_eq!(output.binary_path, build.expected_binary_path());

    // Launch the just-built binary. It prints once and exits — no need to kill.
    let deploy = DeployStage::new(&output.binary_path).capture_output();
    let mut handle = deploy.run().unwrap();
    assert!(handle.pid() > 0);

    // The fake warp exits immediately after printing. Poll briefly.
    let mut exit_status = None;
    for _ in 0..100 {
        if let Some(s) = handle.try_wait().unwrap() {
            exit_status = Some(s);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let status = exit_status.expect("fake warp should have exited");
    assert!(status.success(), "fake warp exited non-zero: {status}");

    handle.kill().unwrap();
}
