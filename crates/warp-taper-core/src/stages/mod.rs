//! Pipeline stages.
//!
//! P3 ships [`build`] and [`deploy`]. Subsequent phases add `record`
//! (screen + log capture), `evaluate` (assertion engine wiring), and
//! `bundle` (README + metadata emission). A unified `Stage` trait will
//! land alongside the pipeline orchestrator in P5; until then each stage
//! is a concrete type with its own constructor + `run` method.

pub mod build;
pub mod deploy;

pub use build::{BuildOutput, BuildProfile, BuildStage};
pub use deploy::{DeployHandle, DeployStage};
