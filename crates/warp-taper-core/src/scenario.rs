//! Scenario definition and loaders.
//!
//! A `Scenario` is the input to a recording session: title, ticket pointer,
//! expected-behavior prose, and any external log directories that need to be
//! snapshotted at end-of-recording. Scenarios can be constructed via the
//! typed [`ScenarioBuilder`] API or loaded from the legacy YAML format used
//! by the bash pipeline.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scenario {
    pub slug: String,
    pub metadata: Metadata,
    pub mcp_log_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Metadata {
    pub title: String,
    pub ticket: Option<String>,
    pub expected: Option<String>,
}

impl Scenario {
    pub fn builder(slug: impl Into<String>) -> ScenarioBuilder {
        ScenarioBuilder::new(slug.into())
    }

    /// Load a scenario from a `metadata.yaml` file. The slug is derived from
    /// the parent directory name (e.g. `scenarios/10874-mcp-log-rotation/metadata.yaml`
    /// → slug `"10874-mcp-log-rotation"`).
    pub fn from_yaml_file(path: &Path) -> Result<Self> {
        let yaml = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(Error::ScenarioNotFound(path.to_path_buf()));
            }
            Err(e) => return Err(Error::Io(e)),
        };
        let slug = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .ok_or_else(|| {
                Error::ScenarioInvalid(format!("cannot derive slug from path {}", path.display()))
            })?;
        Self::from_yaml_str(slug, &yaml)
    }

    pub fn from_yaml_str(slug: &str, yaml: &str) -> Result<Self> {
        #[derive(serde::Deserialize)]
        struct Raw {
            #[serde(default)]
            title: String,
            #[serde(default)]
            ticket: Option<String>,
            #[serde(default)]
            expected: Option<String>,
            #[serde(default)]
            mcp_log_paths: Vec<String>,
        }

        let raw: Raw = serde_yml::from_str(yaml)
            .map_err(|e| Error::ScenarioInvalid(format!("yaml parse: {e}")))?;

        if raw.title.trim().is_empty() {
            return Err(Error::ScenarioInvalid("missing field: title".into()));
        }
        if slug.is_empty() {
            return Err(Error::ScenarioInvalid("slug is required".into()));
        }

        let mcp_log_paths = raw.mcp_log_paths.into_iter().map(expand_tilde).collect();

        Ok(Self {
            slug: slug.to_string(),
            metadata: Metadata {
                title: raw.title,
                ticket: raw.ticket.filter(|s| !s.is_empty()),
                expected: raw
                    .expected
                    .map(|s| s.trim_end_matches('\n').to_string())
                    .filter(|s| !s.is_empty()),
            },
            mcp_log_paths,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ScenarioBuilder {
    slug: String,
    metadata: Metadata,
    mcp_log_paths: Vec<PathBuf>,
}

impl ScenarioBuilder {
    fn new(slug: String) -> Self {
        Self {
            slug,
            metadata: Metadata::default(),
            mcp_log_paths: Vec::new(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.metadata.title = title.into();
        self
    }

    pub fn ticket(mut self, ticket: impl Into<String>) -> Self {
        self.metadata.ticket = Some(ticket.into());
        self
    }

    pub fn expected(mut self, expected: impl Into<String>) -> Self {
        self.metadata.expected = Some(expected.into());
        self
    }

    pub fn mcp_log_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.mcp_log_paths.push(path.into());
        self
    }

    pub fn build(self) -> Result<Scenario> {
        if self.slug.is_empty() {
            return Err(Error::ScenarioInvalid("slug is required".into()));
        }
        if self.metadata.title.trim().is_empty() {
            return Err(Error::ScenarioInvalid("title is required".into()));
        }
        Ok(Scenario {
            slug: self.slug,
            metadata: self.metadata,
            mcp_log_paths: self.mcp_log_paths,
        })
    }
}

fn expand_tilde(s: String) -> PathBuf {
    if let Some(stripped) = s.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_YAML: &str = "title: \"hello world\"\n";

    const FULL_YAML: &str = "\
title: \"MCP log rotation kicks in at the size cap\"
ticket: \"warpdotdev/warp#10874\"
expected: |
  An MCP server's log rotates after the size threshold is crossed.
  No error toasts during rotation.
mcp_log_paths:
  - ~/Library/Group Containers/2BBY89MBSN.dev.warp/Library/Application Support/dev.warp.Warp-Stable/mcp
  - /tmp/some/other/path
";

    #[test]
    fn from_yaml_str_minimal() {
        let s = Scenario::from_yaml_str("slug-x", MINIMAL_YAML).unwrap();
        assert_eq!(s.slug, "slug-x");
        assert_eq!(s.metadata.title, "hello world");
        assert!(s.metadata.ticket.is_none());
        assert!(s.metadata.expected.is_none());
        assert!(s.mcp_log_paths.is_empty());
    }

    #[test]
    fn from_yaml_str_full() {
        let s = Scenario::from_yaml_str("10874-mcp-log-rotation", FULL_YAML).unwrap();
        assert_eq!(
            s.metadata.title,
            "MCP log rotation kicks in at the size cap"
        );
        assert_eq!(s.metadata.ticket.as_deref(), Some("warpdotdev/warp#10874"));
        let expected = s.metadata.expected.unwrap();
        assert!(expected.contains("size threshold"));
        assert!(expected.contains("error toasts"));
        // Block scalar trailing newline is trimmed.
        assert!(!expected.ends_with('\n'));
        assert_eq!(s.mcp_log_paths.len(), 2);
    }

    #[test]
    fn from_yaml_str_missing_title_is_invalid() {
        let yaml = "ticket: \"x/y#1\"\n";
        let err = Scenario::from_yaml_str("slug", yaml).unwrap_err();
        match err {
            Error::ScenarioInvalid(msg) => assert!(msg.contains("title"), "msg: {msg}"),
            other => panic!("expected ScenarioInvalid, got {other:?}"),
        }
    }

    #[test]
    fn from_yaml_str_whitespace_only_title_is_invalid() {
        let yaml = "title: \"   \"\n";
        let err = Scenario::from_yaml_str("slug", yaml).unwrap_err();
        assert!(matches!(err, Error::ScenarioInvalid(_)));
    }

    #[test]
    fn from_yaml_str_malformed_yaml_is_invalid() {
        let yaml = "title: \"unterminated";
        let err = Scenario::from_yaml_str("slug", yaml).unwrap_err();
        assert!(matches!(err, Error::ScenarioInvalid(_)));
    }

    #[test]
    fn from_yaml_str_empty_slug_is_invalid() {
        let err = Scenario::from_yaml_str("", MINIMAL_YAML).unwrap_err();
        assert!(matches!(err, Error::ScenarioInvalid(_)));
    }

    #[test]
    fn from_yaml_str_empty_mcp_list() {
        let yaml = "title: \"x\"\nmcp_log_paths: []\n";
        let s = Scenario::from_yaml_str("slug", yaml).unwrap();
        assert!(s.mcp_log_paths.is_empty());
    }

    struct HomeGuard(Option<std::ffi::OsString>);
    impl Drop for HomeGuard {
        fn drop(&mut self) {
            // SAFETY: only used in single-threaded test scope.
            match self.0.take() {
                Some(h) => unsafe { std::env::set_var("HOME", h) },
                None => unsafe { std::env::remove_var("HOME") },
            }
        }
    }

    #[test]
    fn from_yaml_str_expands_tilde_in_paths() {
        let _guard = HomeGuard(std::env::var_os("HOME"));
        // SAFETY: only used in single-threaded test scope.
        unsafe { std::env::set_var("HOME", "/tmp/fake-home") };

        let yaml = "title: x\nmcp_log_paths:\n  - ~/foo/bar\n";
        let s = Scenario::from_yaml_str("slug", yaml).unwrap();
        assert_eq!(s.mcp_log_paths[0], PathBuf::from("/tmp/fake-home/foo/bar"));
    }

    #[test]
    fn from_yaml_file_missing_yields_not_found() {
        let err =
            Scenario::from_yaml_file(Path::new("/no/such/scenario/metadata.yaml")).unwrap_err();
        assert!(matches!(err, Error::ScenarioNotFound(_)));
    }

    #[test]
    fn from_yaml_file_derives_slug_from_parent_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let scenario_dir = tmp.path().join("12345-fancy-fix");
        std::fs::create_dir(&scenario_dir).unwrap();
        let yaml_path = scenario_dir.join("metadata.yaml");
        std::fs::write(&yaml_path, MINIMAL_YAML).unwrap();

        let s = Scenario::from_yaml_file(&yaml_path).unwrap();
        assert_eq!(s.slug, "12345-fancy-fix");
    }

    #[test]
    fn builder_happy_path() {
        let s = Scenario::builder("slug")
            .title("My scenario")
            .ticket("owner/repo#42")
            .expected("It should work.")
            .mcp_log_path("/tmp/mcp")
            .build()
            .unwrap();

        assert_eq!(s.slug, "slug");
        assert_eq!(s.metadata.title, "My scenario");
        assert_eq!(s.metadata.ticket.as_deref(), Some("owner/repo#42"));
        assert_eq!(s.metadata.expected.as_deref(), Some("It should work."));
        assert_eq!(s.mcp_log_paths, vec![PathBuf::from("/tmp/mcp")]);
    }

    #[test]
    fn builder_rejects_empty_title() {
        let err = Scenario::builder("slug").build().unwrap_err();
        assert!(matches!(err, Error::ScenarioInvalid(_)));
    }

    #[test]
    fn builder_rejects_empty_slug() {
        let err = Scenario::builder("").title("x").build().unwrap_err();
        assert!(matches!(err, Error::ScenarioInvalid(_)));
    }
}
