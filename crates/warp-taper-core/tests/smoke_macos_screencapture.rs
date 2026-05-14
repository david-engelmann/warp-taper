//! L5 smoke test: drives macOS `screencapture` for real.
//!
//! Gated behind the `smoke` feature and ignored by default — runs locally
//! only on a Mac with screen-recording permission granted. CI never enables
//! this feature, so the file compiles to nothing in the default build.

#![cfg(all(feature = "smoke", target_os = "macos"))]

use std::time::Duration;

use warp_taper_core::MacOsScreencapture;

#[test]
#[ignore = "smoke; requires screen-recording permission. Run: cargo test --features smoke -- --ignored"]
fn one_second_screencapture_produces_non_empty_mov() {
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("smoke.mov");

    let recorder = MacOsScreencapture::new().with_max_duration_seconds(2);
    let handle = recorder.start(&dest).expect("spawn screencapture");

    // Let the recording run for a moment before signaling stop.
    std::thread::sleep(Duration::from_secs(1));

    let artifact = handle.stop().expect("finalize recording");
    assert_eq!(artifact.path, dest);
    assert!(
        artifact.bytes > 0,
        "expected a non-empty .mov, got {} bytes",
        artifact.bytes
    );
}
