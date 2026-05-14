//! Built-in scenarios authored as Rust code.
//!
//! Each scenario function returns `(Scenario, Vec<Box<dyn Assertion>>)` so
//! callers can plug it directly into a [`Pipeline`](crate::Pipeline).

pub mod mcp_log_rotation;

pub use mcp_log_rotation::mcp_log_rotation;

use crate::assertion::Assertion;
use crate::error::Result;
use crate::scenario::Scenario;

/// Type alias for a scenario and its assertions.
pub type Builtin = (Scenario, Vec<Box<dyn Assertion>>);

/// Resolve a built-in scenario by slug. Returns `None` if no scenario by
/// that name is registered.
pub fn by_name(name: &str) -> Option<fn() -> Result<Builtin>> {
    match name {
        "mcp-log-rotation" | "10874-mcp-log-rotation" => Some(mcp_log_rotation),
        _ => None,
    }
}

/// Names of all registered built-in scenarios, in stable order.
pub fn names() -> &'static [&'static str] {
    &["mcp-log-rotation"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn by_name_resolves_short_and_long_slug() {
        assert!(by_name("mcp-log-rotation").is_some());
        assert!(by_name("10874-mcp-log-rotation").is_some());
    }

    #[test]
    fn by_name_returns_none_for_unknown() {
        assert!(by_name("nope").is_none());
    }

    #[test]
    fn names_includes_registered_scenarios() {
        assert!(names().contains(&"mcp-log-rotation"));
    }
}
