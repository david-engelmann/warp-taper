//! Built-in: PR #10874 MCP log rotation kicks in at the size cap.
//!
//! Ported from the bash `scenarios/10874-mcp-log-rotation/` directory.
//! Declares the metadata and the same checks the legacy `assertions.sh`
//! performed, expressed as typed [`Assertion`]s.

use std::path::PathBuf;

use crate::assertion::{Assertion, LogLacks, McpLogSnapshotCaptured, McpRotationOccurred};
use crate::error::Result;
use crate::scenario::Scenario;
use crate::scenarios::Builtin;

const TITLE: &str = "MCP log rotation kicks in at the size cap";
const TICKET: &str = "warpdotdev/warp#10874";
const EXPECTED: &str = "\
An MCP server's log file rotates after writing past the configured size
threshold (10 MiB by default, 5 rotated copies = 60 MiB cap per server).
The MCP server continues to operate normally during rotation: no error
toasts, no dropped connections, no crashed processes. Before PR #10874
the active log grew without bound.";

/// Default macOS path for the Warp-Stable MCP log directory.
pub fn default_mcp_log_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(
            "Library/Group Containers/2BBY89MBSN.dev.warp/Library/Application Support/dev.warp.Warp-Stable/mcp",
        );
    }
    PathBuf::from("./mcp")
}

/// Build the scenario + assertion bundle.
pub fn mcp_log_rotation() -> Result<Builtin> {
    let scenario = Scenario::builder("10874-mcp-log-rotation")
        .title(TITLE)
        .ticket(TICKET)
        .expected(EXPECTED)
        .mcp_log_path(default_mcp_log_dir())
        .build()?;

    let assertions: Vec<Box<dyn Assertion>> = vec![
        Box::new(McpLogSnapshotCaptured),
        Box::new(McpRotationOccurred::new()),
        Box::new(LogLacks::in_session_log(
            r"SimpleLogger: rotation failed",
            "no rotation-failure WARNs in session log",
        )?),
    ];

    Ok((scenario, assertions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_scenario_with_expected_metadata() {
        let (scenario, _) = mcp_log_rotation().unwrap();
        assert_eq!(scenario.slug, "10874-mcp-log-rotation");
        assert_eq!(scenario.metadata.title, TITLE);
        assert_eq!(scenario.metadata.ticket.as_deref(), Some(TICKET));
        let expected = scenario.metadata.expected.as_deref().unwrap_or("");
        assert!(expected.contains("threshold"), "expected: {expected}");
        assert!(expected.contains("rotation"), "expected: {expected}");
        assert_eq!(scenario.mcp_log_paths.len(), 1);
    }

    #[test]
    fn assertions_cover_snapshot_rotation_and_no_failures() {
        let (_, assertions) = mcp_log_rotation().unwrap();
        let names: Vec<&str> = assertions.iter().map(|a| a.name()).collect();
        assert!(names.contains(&"mcp_log_snapshot_captured"));
        assert!(names.contains(&"mcp_rotation_occurred"));
        assert!(names.contains(&"log_lacks"));
        assert_eq!(assertions.len(), 3);
    }

    #[test]
    fn default_mcp_log_dir_includes_warp_stable_subpath() {
        let path = default_mcp_log_dir();
        let s = path.to_string_lossy();
        assert!(s.contains("dev.warp.Warp-Stable"));
        assert!(s.ends_with("/mcp"));
    }
}
