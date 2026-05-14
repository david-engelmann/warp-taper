//! Test fixtures and helpers used by warp-taper-core and warp-taper-cli tests.
//!
//! Public from P3 onward as later phases add stubs (deployer, recorder,
//! pipeline). Fixture functions write deterministic on-disk artifacts under
//! a caller-provided temp dir and return either typed handles or paths to
//! the produced files.

pub mod cargo_project;

pub use cargo_project::{tiny_warp, tiny_warp_with_behavior, TinyWarp, WarpBehavior};
