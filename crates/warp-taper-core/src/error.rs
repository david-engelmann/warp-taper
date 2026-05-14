use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("scenario not found: {0}")]
    ScenarioNotFound(PathBuf),

    #[error("scenario invalid: {0}")]
    ScenarioInvalid(String),

    #[error("stage failed: {stage}: {message}")]
    StageFailed {
        stage: &'static str,
        message: String,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_scenario_not_found_includes_path() {
        let err = Error::ScenarioNotFound(PathBuf::from("/no/such/path"));
        let rendered = format!("{err}");
        assert!(rendered.contains("/no/such/path"));
        assert!(rendered.contains("scenario not found"));
    }

    #[test]
    fn display_scenario_invalid_includes_reason() {
        let err = Error::ScenarioInvalid("missing field: ticket".into());
        let rendered = format!("{err}");
        assert!(rendered.contains("missing field: ticket"));
    }

    #[test]
    fn display_stage_failed_includes_stage_and_message() {
        let err = Error::StageFailed {
            stage: "build",
            message: "cargo exited with status 1".into(),
        };
        let rendered = format!("{err}");
        assert!(rendered.contains("build"));
        assert!(rendered.contains("cargo exited with status 1"));
    }

    #[test]
    fn io_error_converts_via_from() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }
}
