//! Built-in: PR #10839 — MCP server save respects the secret-redaction toggle.
//!
//! Demonstrates the fix landed in PR #10839: when Settings → Privacy →
//! Secret redaction is OFF, defining an MCP server config that contains a
//! literal secret (e.g. `Bearer <token>` in the top-level `url`) no longer
//! fails. When the toggle is ON the save is still blocked.
//!
//! Recording sequence (drive manually or via warp-driver.swift):
//!   1. Settings → Privacy. Confirm "Secret redaction" toggle starts OFF.
//!   2. Settings → MCP Servers. Open existing server's JSON editor, paste a
//!      config whose `url` contains a `sk-…`-style token; click Save.
//!      → No error toast; save succeeds. (This is the #8761 user case.)
//!   3. Back to Settings → Privacy. Flip the toggle ON. Wait for the
//!      "Secret visual redaction mode" dropdown to appear (proof the
//!      runtime regex compiled).
//!   4. Settings → MCP Servers. Edit the server, click Save again with the
//!      same secret-bearing config.
//!      → Error toast appears: "This MCP server contains secrets. Visit
//!        Settings > Privacy to modify your secret redaction settings."

use crate::assertion::{Assertion, FileExists};
use crate::error::Result;
use crate::scenario::Scenario;
use crate::scenarios::Builtin;

const TITLE: &str = "MCP server save respects the secret-redaction toggle";
const TICKET: &str = "warpdotdev/warp#10839";
const EXPECTED: &str = "\
With Settings → Privacy → Secret redaction OFF, saving an MCP server config
containing a literal secret succeeds (no error toast). Flipping the toggle ON
and saving the same config produces the existing 'contains secrets' error
toast. Enterprise-enforced redaction still blocks the save regardless of the
personal toggle. Before PR #10839 the save was rejected even with the toggle
OFF.";

pub fn mcp_redaction_toggle() -> Result<Builtin> {
    let scenario = Scenario::builder("10839-mcp-redaction-toggle")
        .title(TITLE)
        .ticket(TICKET)
        .expected(EXPECTED)
        .build()?;

    let master_mov = scenario_master_mov_path("10839-mcp-redaction-toggle");
    let assertions: Vec<Box<dyn Assertion>> = vec![Box::new(FileExists::new(
        master_mov,
        "master.mov captured for #10839 redaction-toggle demo",
    ))];

    Ok((scenario, assertions))
}

fn scenario_master_mov_path(slug: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("tapes/{slug}/master.mov"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_with_expected_metadata() {
        let (scenario, assertions) = mcp_redaction_toggle().unwrap();
        assert_eq!(scenario.slug, "10839-mcp-redaction-toggle");
        assert_eq!(scenario.metadata.title, TITLE);
        assert_eq!(scenario.metadata.ticket.as_deref(), Some(TICKET));
        assert_eq!(assertions.len(), 1);
        assert_eq!(assertions[0].name(), "file_exists");
    }

    #[test]
    fn expected_mentions_both_toggle_states() {
        let (scenario, _) = mcp_redaction_toggle().unwrap();
        let expected = scenario.metadata.expected.unwrap();
        assert!(expected.contains("OFF"));
        assert!(expected.contains("ON"));
    }
}
