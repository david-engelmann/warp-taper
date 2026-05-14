//! warp-taper core library.
//!
//! Public modules land per the [implementation plan](../../../docs/PLAN.md).
//! P0 ships the error type only; subsequent phases add `scenario`,
//! `log_tail`, `assertion`, `recorder`, `stages`, `pipeline`, and `bundle`.

pub mod error;

pub use error::{Error, Result};
