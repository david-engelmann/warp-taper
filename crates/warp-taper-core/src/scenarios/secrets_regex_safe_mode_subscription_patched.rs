//! Built-in: AFTER recording for warpdotdev/warp#11262 / PR #11457.
//!
//! Sibling of [`secrets_regex_startup_empty`] (the BEFORE recording). Runs
//! against a warp-oss binary built from `david/11262-secrets-regex-safe-mode-subscription`,
//! i.e. WITH the new `SafeModeSettings` subscription wired into
//! `CustomSecretRegexUpdater`. The recipe walks the same UI path:
//!
//!   1. settings.toml has `privacy.secret_redaction.enabled = true` and a
//!      populated `privacy.custom_secret_regex_list`.
//!   2. Launch warp-oss. Runtime toggle starts OFF.
//!   3. Settings -> Privacy -> click the toggle ON. The new
//!      `SafeModeSettingsChangedEvent::SafeModeEnabled` subscription fires
//!      and `update_custom_secret_regex_list` recompiles SECRETS_REGEX from
//!      the TOML-loaded list.
//!   4. Settings -> MCP Servers -> + Add -> paste a config containing
//!      `sk-FAKE0000000000FAKEdemoWARP11262` -> click Save.
//!      -> Save is BLOCKED. The toast "This MCP server contains secrets…"
//!         appears and `demo-regex-empty-11262` does NOT land in MY MCPS.
//!
//! Captions match the BEFORE scenario through step 4 so the two recordings
//! are visually parallel; only the final outcome caption diverges.

use crate::assertion::{Assertion, FileExists};
use crate::error::Result;
use crate::scenario::Scenario;
use crate::scenarios::Builtin;

const TITLE: &str =
    "AFTER (PR #11457) - SafeModeEnabled subscription compiles SECRETS_REGEX, MCP save blocked";
const TICKET: &str = "warpdotdev/warp#11262";
const EXPECTED: &str = "\
With PR #11457 applied, toggling Safe Mode ON fires
SafeModeSettingsChangedEvent::SafeModeEnabled, which the
CustomSecretRegexUpdater now subscribes to. The subscription calls
update_custom_secret_regex_list, compiling SECRETS_REGEX from the
TOML-loaded custom_secret_regex_list. The follow-up MCP +Add save with
an embedded secret is then correctly blocked by #10839's predicate, and
the user sees the 'This MCP server contains secrets…' toast.";

pub fn secrets_regex_safe_mode_subscription_patched() -> Result<Builtin> {
    let scenario = Scenario::builder("11262-secrets-regex-startup-empty-patched")
        .title(TITLE)
        .ticket(TICKET)
        .expected(EXPECTED)
        // Steps 1-4 mirror the BEFORE scenario one-for-one (same UI path,
        // same wait_ms budget in the recipe) so the two recordings are
        // visually parallel up to the Save click. The outcome caption
        // diverges: BEFORE says "BUG ... SUCCEEDED", AFTER says
        // "FIX ... BLOCKED".
        .caption(
            0.0,
            8.0,
            "AFTER (PR #11457) - SafeModeEnabled subscription added to CustomSecretRegexUpdater",
        )
        .caption(
            8.0,
            28.0,
            "Step 1: Settings -> Privacy -> 'Secret redaction' (toggle starts OFF, same as BEFORE)",
        )
        .caption(
            28.0,
            48.0,
            "Step 2: Click toggle ON - new SafeModeEnabled subscription fires, SECRETS_REGEX compiles from TOML list",
        )
        .caption(
            48.0,
            98.0,
            "Step 3: MCP Servers -> + Add -> paste config with sk-FAKE...11262 secret",
        )
        .caption(
            98.0,
            110.0,
            "Step 4: Click Save",
        )
        .caption(
            110.0,
            145.0,
            "FIX: Save BLOCKED - toast 'This MCP server contains secrets…'. Predicate (#10839) now sees the populated DFA.",
        )
        .build()?;

    let master_mov =
        std::path::PathBuf::from("tapes/11262-secrets-regex-startup-empty-patched/master.mov");
    let assertions: Vec<Box<dyn Assertion>> = vec![Box::new(FileExists::new(
        master_mov,
        "master.mov captured for #11262 patched (PR #11457) AFTER recording",
    ))];

    Ok((scenario, assertions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_with_expected_metadata() {
        let (scenario, assertions) = secrets_regex_safe_mode_subscription_patched().unwrap();
        assert_eq!(scenario.slug, "11262-secrets-regex-startup-empty-patched");
        assert_eq!(scenario.metadata.ticket.as_deref(), Some(TICKET));
        assert_eq!(assertions.len(), 1);
    }

    #[test]
    fn captions_parallel_the_before_scenario_through_step_four() {
        // Steps 1-4 must end at the same timestamps as the BEFORE scenario
        // so when the two videos are reviewed side-by-side every UI moment
        // lines up except the final outcome.
        let (scenario, _) = secrets_regex_safe_mode_subscription_patched().unwrap();
        assert_eq!(scenario.captions.len(), 6);
        let boundaries: Vec<u64> = scenario.captions.iter().map(|c| c.end.as_secs()).collect();
        assert_eq!(boundaries, vec![8, 28, 48, 98, 110, 145]);
    }

    #[test]
    fn outcome_caption_calls_out_the_fix() {
        let (scenario, _) = secrets_regex_safe_mode_subscription_patched().unwrap();
        let outcome = scenario.captions.last().unwrap();
        assert!(
            outcome.text.contains("FIX") && outcome.text.contains("BLOCKED"),
            "AFTER outcome caption must say FIX and BLOCKED, got: {}",
            outcome.text
        );
        assert!(
            outcome.text.contains("This MCP server contains secrets"),
            "AFTER outcome caption must quote the actual toast text (lets reviewers match it to the screen), got: {}",
            outcome.text
        );
    }
}
