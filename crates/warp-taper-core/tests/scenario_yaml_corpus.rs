//! Integration tests for the YAML scenario loader against a small corpus
//! of representative shapes.

use std::path::PathBuf;

use rstest::rstest;
use warp_taper_core::Scenario;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scenario-yaml")
}

#[rstest]
#[case::minimal("minimal.yaml", "minimal", "minimal scenario", false, false, 0)]
#[case::with_ticket("with-ticket.yaml", "with-ticket", "with ticket", true, false, 0)]
#[case::with_expected("with-expected.yaml", "with-expected", "with expected", false, true, 0)]
#[case::with_mcp_paths(
    "with-mcp-paths.yaml",
    "with-mcp-paths",
    "with mcp paths",
    false,
    false,
    2
)]
#[case::full("full.yaml", "full", "full kitchen sink", true, true, 1)]
fn parses_corpus(
    #[case] file: &str,
    #[case] slug: &str,
    #[case] expected_title: &str,
    #[case] has_ticket: bool,
    #[case] has_expected: bool,
    #[case] mcp_count: usize,
) {
    let yaml = std::fs::read_to_string(fixtures_dir().join(file))
        .unwrap_or_else(|e| panic!("read {file}: {e}"));
    let s = Scenario::from_yaml_str(slug, &yaml).unwrap_or_else(|e| panic!("parse {file}: {e}"));
    assert_eq!(s.slug, slug);
    assert_eq!(s.metadata.title, expected_title);
    assert_eq!(s.metadata.ticket.is_some(), has_ticket);
    assert_eq!(s.metadata.expected.is_some(), has_expected);
    assert_eq!(s.mcp_log_paths.len(), mcp_count);
}
