//! Built-in: #11263 — Secrets pasted into MCP `env` or `headers` bypass the
//! redaction predicate because `from_user_json` templatizes those values to
//! `{{key}}` placeholders before `find_secrets_in_text` runs.
//!
//! Reproduction (drive against a build WITHOUT the DEMO-PATCH-C raw-JSON
//! scan in `parse_templatable_json`):
//!   1. With redaction toggle ON (so SECRETS_REGEX is populated), open an
//!      existing MCP server's JSON editor.
//!   2. Paste a config that puts the secret in `headers.Authorization`
//!      (e.g. `"Authorization": "Bearer sk-…"`), NOT in the top-level
//!      `url`.
//!   3. Click Save.
//!      → Save succeeds. No "contains secrets" toast.
//!      → Reason: by the time the predicate scans `template.json`, the
//!        bearer value has been replaced with `{{ServerName_API_KEY}}`,
//!        so `find_secrets_in_text` sees no secret.
//!   4. (Sanity step) Move the same secret into the top-level `url`
//!      string. Save again.
//!      → Save is now BLOCKED — proving the predicate works for non-
//!        templatized fields and the bug is the pre-scan templatization.

use crate::assertion::{Assertion, FileExists};
use crate::error::Result;
use crate::scenario::Scenario;
use crate::scenarios::Builtin;

const TITLE: &str =
    "Secrets in MCP env/headers bypass redaction (templatized to placeholders before scan)";
const TICKET: &str = "warpdotdev/warp#11263";
const EXPECTED: &str = "\
With Settings → Privacy → Secret redaction ON, an MCP server save that
embeds a literal secret in `env` or `headers` should be blocked by the
existing predicate. Today it succeeds: ParsedTemplatableMCPServerResult
::from_user_json runs templatize_field on those subtrees before the
secret scan, so find_secrets_in_text only ever sees `{{placeholder}}`
values. The fix is to scan the raw user input before templatization.";

pub fn mcp_env_headers_bypass() -> Result<Builtin> {
    let scenario = Scenario::builder("11263-mcp-env-headers-bypass")
        .title(TITLE)
        .ticket(TICKET)
        .expected(EXPECTED)
        .build()?;

    let master_mov = std::path::PathBuf::from("tapes/11263-mcp-env-headers-bypass/master.mov");
    let assertions: Vec<Box<dyn Assertion>> = vec![Box::new(FileExists::new(
        master_mov,
        "master.mov captured for #11263 env/headers-bypass repro",
    ))];

    Ok((scenario, assertions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_with_expected_metadata() {
        let (scenario, assertions) = mcp_env_headers_bypass().unwrap();
        assert_eq!(scenario.slug, "11263-mcp-env-headers-bypass");
        assert_eq!(scenario.metadata.ticket.as_deref(), Some(TICKET));
        assert_eq!(assertions.len(), 1);
    }
}
