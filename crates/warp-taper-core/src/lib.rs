//! warp-taper core library.
//!
//! Public modules land per the [implementation plan](../../../docs/PLAN.md).
//! P1 ships pure-logic modules: `scenario`, `log_tail`, `bundle`.
//! Stages (`pipeline`, `stages::*`, `recorder`, `assertion`) land in P2+.

// rust-1.95 added `doc_overindented_list_items`, which trips on the
// long-form module-doc bullet lists we use to explain each scenario's
// reproduction shape. Suppressing here rather than rewriting every
// scenario module's docstring; the indentation is intentional and
// renders correctly in `cargo doc`.
#![allow(clippy::doc_overindented_list_items)]

pub mod assertion;
pub mod bundle;
pub mod captions;
pub mod error;
pub mod log_tail;
pub mod pipeline;
pub mod recorder;
pub mod scenario;
pub mod scenarios;
pub mod stages;

pub use assertion::{
    Assertion, AssertionContext, AssertionResult, EngineReport, NamedResult, Outcome,
};
pub use captions::{apply_captions, CaptionConfig, CaptionedArtifacts};
pub use error::{Error, Result};
pub use log_tail::LogTail;
pub use pipeline::{Pipeline, RecordTrigger, Tape};
#[cfg(unix)]
pub use recorder::{MacOsScreencapture, MacOsScreencaptureHandle};
pub use recorder::{
    NoOpRecorder, NoOpRecordingHandle, Recorder, RecordingArtifact, RecordingHandle,
};
pub use scenario::{Caption, Metadata, Scenario, ScenarioBuilder};
pub use stages::{BuildOutput, BuildProfile, BuildStage, DeployHandle, DeployStage};
