//! Built-in: #11262 — SECRETS_REGEX is silently empty at startup despite a
//! persisted custom_secret_regex_list and a TOML-enabled redaction toggle.
//!
//! Reproduction (drive against a build WITHOUT the DEMO-PATCH-B
//! subscription in `secret_regex_updater.rs`):
//!   1. settings.toml has `privacy.secret_redaction.enabled = true` AND a
//!      populated `privacy.custom_secret_regex_list`.
//!   2. Launch warp-oss. The runtime toggle starts OFF (cloud sync wins
//!      over TOML — see feedback memory). SECRETS_REGEX is empty because
//!      no `PrivacySettingsChangedEvent::CustomSecretRegexListChanged`
//!      ever fires at startup.
//!   3. Settings → MCP Servers → open an existing server, paste a
//!      bearer-token config, click Save.
//!      → Save succeeds even though the user persisted both `enabled=true`
//!        and a regex list capable of detecting the secret.
//!   4. (Confirming step) Flip the toggle ON, save again. Save is now
//!        BLOCKED — proving the regex list compiled only in response to a
//!        SafeModeSettings event, not at startup.

use crate::assertion::{Assertion, FileExists};
use crate::error::Result;
use crate::scenario::Scenario;
use crate::scenarios::Builtin;

const TITLE: &str = "SECRETS_REGEX empty at startup despite enabled=true + populated regex list";
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
}
