//! Built-in scenarios authored as Rust code.
//!
//! Each scenario function returns `(Scenario, Vec<Box<dyn Assertion>>)` so
//! callers can plug it directly into a [`Pipeline`](crate::Pipeline).

pub mod mcp_add_server_bypass;
pub mod mcp_env_headers_bypass;
pub mod mcp_integer_coercion;
pub mod mcp_log_rotation;
pub mod mcp_redaction_toggle;
pub mod secrets_regex_startup_empty;

pub use mcp_add_server_bypass::mcp_add_server_bypass;
pub use mcp_env_headers_bypass::mcp_env_headers_bypass;
pub use mcp_integer_coercion::mcp_integer_coercion;
pub use mcp_log_rotation::mcp_log_rotation;
pub use mcp_redaction_toggle::mcp_redaction_toggle;
pub use secrets_regex_startup_empty::secrets_regex_startup_empty;

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
        "mcp-redaction-toggle" | "10839-mcp-redaction-toggle" => Some(mcp_redaction_toggle),
        "secrets-regex-startup-empty" | "11262-secrets-regex-startup-empty" => {
            Some(secrets_regex_startup_empty)
        }
        "mcp-env-headers-bypass" | "11263-mcp-env-headers-bypass" => Some(mcp_env_headers_bypass),
        "mcp-add-server-bypass" | "11265-mcp-add-server-bypass" => Some(mcp_add_server_bypass),
        "mcp-integer-coercion" | "11407-mcp-integer-coercion" => Some(mcp_integer_coercion),
        _ => None,
    }
}

/// Names of all registered built-in scenarios, in stable order.
pub fn names() -> &'static [&'static str] {
    &[
        "mcp-log-rotation",
        "mcp-redaction-toggle",
        "secrets-regex-startup-empty",
        "mcp-env-headers-bypass",
        "mcp-add-server-bypass",
        "mcp-integer-coercion",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn by_name_resolves_short_and_long_slug() {
        assert!(by_name("mcp-log-rotation").is_some());
        assert!(by_name("10874-mcp-log-rotation").is_some());
        assert!(by_name("mcp-redaction-toggle").is_some());
        assert!(by_name("10839-mcp-redaction-toggle").is_some());
        assert!(by_name("secrets-regex-startup-empty").is_some());
        assert!(by_name("11262-secrets-regex-startup-empty").is_some());
        assert!(by_name("mcp-env-headers-bypass").is_some());
        assert!(by_name("11263-mcp-env-headers-bypass").is_some());
        assert!(by_name("mcp-add-server-bypass").is_some());
        assert!(by_name("11265-mcp-add-server-bypass").is_some());
        assert!(by_name("mcp-integer-coercion").is_some());
        assert!(by_name("11407-mcp-integer-coercion").is_some());
    }

    #[test]
    fn by_name_returns_none_for_unknown() {
        assert!(by_name("nope").is_none());
    }

    #[test]
    fn names_includes_registered_scenarios() {
        assert!(names().contains(&"mcp-log-rotation"));
        assert!(names().contains(&"mcp-redaction-toggle"));
        assert!(names().contains(&"secrets-regex-startup-empty"));
        assert!(names().contains(&"mcp-env-headers-bypass"));
        assert!(names().contains(&"mcp-add-server-bypass"));
        assert!(names().contains(&"mcp-integer-coercion"));
    }
}
