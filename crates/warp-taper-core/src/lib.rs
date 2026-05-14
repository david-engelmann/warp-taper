//! warp-taper core library.
//!
//! Public modules land per the [implementation plan](../../../docs/PLAN.md).
//! P1 ships pure-logic modules: `scenario`, `log_tail`, `bundle`.
//! Stages (`pipeline`, `stages::*`, `recorder`, `assertion`) land in P2+.

pub mod assertion;
pub mod bundle;
pub mod error;
pub mod log_tail;
pub mod pipeline;
pub mod recorder;
pub mod scenario;
pub mod stages;

pub use assertion::{
    Assertion, AssertionContext, AssertionResult, EngineReport, NamedResult, Outcome,
};
pub use error::{Error, Result};
pub use log_tail::LogTail;
pub use pipeline::{Pipeline, RecordTrigger, Tape};
#[cfg(unix)]
pub use recorder::{MacOsScreencapture, MacOsScreencaptureHandle};
pub use recorder::{
    NoOpRecorder, NoOpRecordingHandle, Recorder, RecordingArtifact, RecordingHandle,
};
pub use scenario::{Metadata, Scenario, ScenarioBuilder};
pub use stages::{BuildOutput, BuildProfile, BuildStage, DeployHandle, DeployStage};
