//! Built-in: #11265 — The "+ Add" (new MCP server) save path bypasses the
//! redaction predicate entirely.
//!
//! Reproduction (drive against a build WITHOUT the DEMO-PATCH-C-NEW
//! raw-JSON scan inside the new-server save branch of
//! `MCPServersEditPageView::handle_action`):
//!   1. With redaction toggle ON, navigate Settings → MCP Servers.
//!   2. Click **+ Add** (NOT edit-existing). Paste a top-level
//!      `url` config that contains a secret (`Bearer sk-…`).
//!   3. Click Save.
//!      → Save succeeds. No "contains secrets" toast.
//!      → Reason: the new-server branch calls
//!        `ParsedTemplatableMCPServerResult::from_user_json(&json)`
//!        directly and skips `parse_templatable_json`, which is where the
//!        predicate lives. The +Add path was never routed through the
//!        predicate that #10839 wired up.
//!   4. (Sanity step) For the same secret-bearing config, save via
//!      **edit-existing** instead.
//!      → Save is BLOCKED. Same JSON, different code path — proves the
//!        bypass is the +Add branch.

use crate::assertion::{Assertion, FileExists};
use crate::error::Result;
use crate::scenario::Scenario;
use crate::scenarios::Builtin;

const TITLE: &str = "MCP '+ Add' new-server save path bypasses redaction predicate";
const TICKET: &str = "warpdotdev/warp#11265";
const EXPECTED: &str = "\
With Settings → Privacy → Secret redaction ON, saving a new MCP server via
the '+ Add' button should be gated by the same predicate that gates
edit-existing saves. Today the +Add branch skips parse_templatable_json
entirely and calls ParsedTemplatableMCPServerResult::from_user_json
directly, so a config with a literal secret in `url` is accepted. Same
JSON saved via edit-existing is correctly rejected — confirming the +Add
branch is the gap.";

pub fn mcp_add_server_bypass() -> Result<Builtin> {
    let scenario = Scenario::builder("11265-mcp-add-server-bypass")
        .title(TITLE)
        .ticket(TICKET)
        .expected(EXPECTED)
        .build()?;

    let master_mov = std::path::PathBuf::from("tapes/11265-mcp-add-server-bypass/master.mov");
    let assertions: Vec<Box<dyn Assertion>> = vec![Box::new(FileExists::new(
        master_mov,
        "master.mov captured for #11265 +Add-bypass repro",
    ))];

    Ok((scenario, assertions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_with_expected_metadata() {
        let (scenario, assertions) = mcp_add_server_bypass().unwrap();
        assert_eq!(scenario.slug, "11265-mcp-add-server-bypass");
        assert_eq!(scenario.metadata.ticket.as_deref(), Some(TICKET));
        assert_eq!(assertions.len(), 1);
    }
}
