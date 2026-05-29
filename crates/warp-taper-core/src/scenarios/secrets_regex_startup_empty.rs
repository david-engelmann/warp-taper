//! Built-in: #11262 — SECRETS_REGEX is silently empty at startup despite a
//! persisted custom_secret_regex_list and a TOML-enabled redaction toggle.
//!
//! This is the **BEFORE** recording — drive against a build WITHOUT the
//! `SafeModeSettings` subscription in `secret_regex_updater.rs`:
//!   1. settings.toml has `privacy.secret_redaction.enabled = true` AND a
//!      populated `privacy.custom_secret_regex_list`.
//!   2. Launch warp-oss. The runtime toggle starts OFF (cloud sync wins
//!      over TOML — see feedback memory). SECRETS_REGEX is empty because
//!      no `PrivacySettingsChangedEvent::CustomSecretRegexList` ever fires
//!      at startup and the updater is **not** subscribed to safe-mode
//!      events.
//!   3. Settings → MCP Servers → + Add → paste a bearer-token config,
//!      click Save.
//!      → Save succeeds even though the user persisted both `enabled=true`
//!        and a regex list capable of detecting the secret. The fix (PR
//!        #11457) is the sibling AFTER scenario:
//!        [`secrets_regex_safe_mode_subscription_patched`].
//!
//! Captions align with the existing master.mov duration (~145 s) and the
//! phase markers in [`scripts/recipes/11262-secrets-regex-startup-empty.json`].

use crate::assertion::{Assertion, FileExists};
use crate::error::Result;
use crate::scenario::Scenario;
use crate::scenarios::Builtin;

const TITLE: &str =
    "BEFORE — SECRETS_REGEX empty at startup despite enabled=true + populated regex list";
const TICKET: &str = "warpdotdev/warp#11262";
const EXPECTED: &str = "\
With TOML privacy.secret_redaction.enabled=true and a populated
custom_secret_regex_list, a fresh warp-oss launch should arrive with
SECRETS_REGEX compiled. Today it does not: the regex set stays empty
until the user touches the toggle or the regex list, so secret-bearing
MCP saves slip through the predicate on first boot.";

pub fn secrets_regex_startup_empty() -> Result<Builtin> {
    let scenario = Scenario::builder("11262-secrets-regex-startup-empty")
        .title(TITLE)
        .ticket(TICKET)
        .expected(EXPECTED)
        // Caption timings line up with the recipe phases in
        // scripts/recipes/11262-secrets-regex-startup-empty.json against
        // the existing ~145 s master recording.
        .caption(
            0.0,
            8.0,
            "BEFORE - Bug #11262: SECRETS_REGEX silently empty at startup",
        )
        .caption(
            8.0,
            28.0,
            "Step 1: Settings -> Privacy -> 'Secret redaction' (toggle starts OFF)",
        )
        .caption(
            28.0,
            48.0,
            "Step 2: Click toggle ON - 'Secret visual redaction mode' appears (UI says enabled)",
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
            "BUG: Save SUCCEEDED - server added to MY MCPS. SECRETS_REGEX never compiled (no safe-mode subscription).",
        )
        .build()?;

    let master_mov = std::path::PathBuf::from("tapes/11262-secrets-regex-startup-empty/master.mov");
    let assertions: Vec<Box<dyn Assertion>> = vec![Box::new(FileExists::new(
        master_mov,
        "master.mov captured for #11262 startup-empty repro",
    ))];

    Ok((scenario, assertions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_with_expected_metadata() {
        let (scenario, assertions) = secrets_regex_startup_empty().unwrap();
        assert_eq!(scenario.slug, "11262-secrets-regex-startup-empty");
        assert_eq!(scenario.metadata.ticket.as_deref(), Some(TICKET));
        assert_eq!(assertions.len(), 1);
    }

    #[test]
    fn captions_are_ordered_and_cover_recipe_duration() {
        let (scenario, _) = secrets_regex_startup_empty().unwrap();
        assert_eq!(scenario.captions.len(), 6);
        for window in scenario.captions.windows(2) {
            assert!(
                window[0].end <= window[1].start,
                "captions must not overlap: {:?} then {:?}",
                window[0],
                window[1]
            );
        }
        assert!(
            scenario.captions.last().unwrap().end >= std::time::Duration::from_secs(140),
            "last caption must span through the save-outcome phase (~145 s)"
        );
    }

    #[test]
    fn before_caption_calls_out_the_bug() {
        let (scenario, _) = secrets_regex_startup_empty().unwrap();
        let outcome = scenario.captions.last().unwrap();
        assert!(
            outcome.text.contains("BUG") && outcome.text.contains("SUCCEEDED"),
            "BEFORE outcome caption must name it as a BUG and say the save SUCCEEDED, got: {}",
            outcome.text
        );
    }
}
