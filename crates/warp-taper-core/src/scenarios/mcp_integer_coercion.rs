//! Built-in: #11407 — MCP tool args now coerce nested whole-number floats
//! to i64 before they hit the wire, so strict MCP servers (e.g. PANW-style
//! OpenAPI auditors) stop rejecting `5.0` where the schema declares
//! `"type": "integer"`.
//!
//! Visual evidence is code-level rather than UI-level: the fix lives in
//! `coerce_integer_args` (a private helper) and is exercised by a unit
//! test that pins the PANW repro schema (object → object → array →
//! oneOf → integer). The scenario drives a terminal session inside
//! warp-oss that:
//!
//!   1. `cat`s the PANW test schema so the recording shows the exact
//!      JSON Schema shape the fix has to walk (object nesting + tuple
//!      `oneOf` + integer leaves at three different positions).
//!   2. Runs `cargo test panw_audit_management_repro -- --nocapture` so
//!      the agent-side output proves the unit test passes — i.e. the
//!      walker successfully coerced all three integer fields.
//!
//! Captions overlay the recording with the agent-perspective story (what
//! the schema declares, what the agent would have sent before the fix,
//! what it sends now).

use std::time::Duration;

use crate::assertion::{Assertion, FileExists};
use crate::error::Result;
use crate::scenario::Scenario;
use crate::scenarios::Builtin;

const TITLE: &str = "MCP tool call: nested whole-number floats coerced to integer before wire send";
const TICKET: &str = "warpdotdev/warp#11407";
const EXPECTED: &str = "\
The PANW-style schema (object → object → array → oneOf → integer)
exercises every recursive path the walker now handles. The unit test
prints PASS, which confirms the wire-format guarantee: an agent supplies
`{ search_from: 0.0, search_to: 100.0, filters: [{ value: 1730419200000.0 }] }`
and the MCP server receives `{ search_from: 0, search_to: 100,
filters: [{ value: 1730419200000 }] }` (integers, not floats).";

pub fn mcp_integer_coercion() -> Result<Builtin> {
    let scenario = Scenario::builder("11407-mcp-integer-coercion")
        .title(TITLE)
        .ticket(TICKET)
        .expected(EXPECTED)
        // The captions narrate the agent-side story. Times line up with the
        // recipe steps in `scripts/recipes/11407-mcp-integer-coercion.json`:
        // the recipe cats the schema (≈ 0-10s), then runs the test (≈ 10-25s),
        // then leaves the result on screen (≈ 25-32s).
        .caption(
            0.0,
            4.0,
            "warpdotdev/warp PR #11407 - nested integer coercion",
        )
        .caption(
            4.0,
            10.0,
            "Schema declares search_from / search_to / filters[].value as integer",
        )
        .caption(
            10.0,
            16.0,
            "Without the fix the agent would send 0.0 / 100.0 / 1730419200000.0",
        )
        .caption(
            16.0,
            22.0,
            "Walker recurses through object - array - oneOf - integer leaves",
        )
        .caption(
            22.0,
            32.0,
            "Test passes - all three nested integers reach the wire as integers",
        )
        .build()?;

    let assertions: Vec<Box<dyn Assertion>> = vec![Box::new(FileExists::new(
        std::path::PathBuf::from("tapes/11407-mcp-integer-coercion/master.mov"),
        "master.mov captured for #11407 integer-coercion evidence",
    ))];

    Ok((scenario, assertions))
}

/// How long the recipe runs end-to-end; matches the last caption's `end`
/// so the captioned output covers the full recording.
#[allow(dead_code)]
pub const SCENARIO_DURATION: Duration = Duration::from_secs(32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_with_expected_metadata() {
        let (scenario, assertions) = mcp_integer_coercion().unwrap();
        assert_eq!(scenario.slug, "11407-mcp-integer-coercion");
        assert_eq!(scenario.metadata.ticket.as_deref(), Some(TICKET));
        assert_eq!(assertions.len(), 1);
    }

    #[test]
    fn five_captions_with_increasing_time_windows() {
        let (scenario, _) = mcp_integer_coercion().unwrap();
        assert_eq!(scenario.captions.len(), 5);
        // Captions must be ordered + non-overlapping.
        let mut last_end = std::time::Duration::ZERO;
        for caption in &scenario.captions {
            assert!(caption.start >= last_end, "captions out of order");
            assert!(caption.end > caption.start);
            last_end = caption.end;
        }
        // Last caption's end matches the documented scenario duration.
        assert_eq!(scenario.captions.last().unwrap().end, SCENARIO_DURATION);
    }
}
