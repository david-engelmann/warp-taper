//! Build stage — wraps `cargo build -p <package>` against a Warp checkout.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    fn dir_name(&self) -> &'static str {
        match self {
            BuildProfile::Debug => "debug",
            BuildProfile::Release => "release",
        }
    }

    fn flag(&self) -> Option<&'static str> {
        match self {
            BuildProfile::Debug => None,
            BuildProfile::Release => Some("--release"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuildStage {
    warp_source: PathBuf,
    package: String,
    binary_name: String,
    profile: BuildProfile,
    cargo_path: PathBuf,
    extra_args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BuildOutput {
    pub binary_path: PathBuf,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub duration: Duration,
}

impl BuildStage {
    /// New stage targeting `cargo build -p warp` against `warp_source`.
    /// Defaults: package `warp`, binary `warp-oss`, profile `Debug`,
    /// cargo binary resolved from `$PATH`.
    pub fn new(warp_source: impl Into<PathBuf>) -> Self {
        Self {
            warp_source: warp_source.into(),
            package: "warp".to_string(),
            binary_name: "warp-oss".to_string(),
            profile: BuildProfile::Debug,
            cargo_path: PathBuf::from("cargo"),
            extra_args: Vec::new(),
        }
    }

    pub fn with_package(mut self, package: impl Into<String>) -> Self {
        self.package = package.into();
        self
    }

    pub fn with_binary_name(mut self, binary_name: impl Into<String>) -> Self {
        self.binary_name = binary_name.into();
        self
    }

    pub fn with_profile(mut self, profile: BuildProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn with_cargo_path(mut self, cargo_path: impl Into<PathBuf>) -> Self {
        self.cargo_path = cargo_path.into();
        self
    }

    pub fn with_extra_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    pub fn warp_source(&self) -> &Path {
        &self.warp_source
    }

    pub fn package(&self) -> &str {
        &self.package
    }

    pub fn binary_name(&self) -> &str {
        &self.binary_name
    }

    pub fn profile(&self) -> BuildProfile {
        self.profile
    }

    /// Build the [`Command`] that would be invoked by [`run`](Self::run).
    /// Exposed so tests can assert the command without executing it.
    pub fn command(&self) -> Command {
        let mut cmd = Command::new(&self.cargo_path);
        cmd.current_dir(&self.warp_source);
        cmd.arg("build");
        cmd.args(["-p", &self.package]);
        if let Some(flag) = self.profile.flag() {
            cmd.arg(flag);
        }
        for arg in &self.extra_args {
            cmd.arg(arg);
        }
        cmd
    }

    /// Where [`run`](Self::run) expects to find the compiled binary on success.
    pub fn expected_binary_path(&self) -> PathBuf {
        self.warp_source
            .join("target")
            .join(self.profile.dir_name())
            .join(&self.binary_name)
    }

    /// Run cargo and resolve the produced binary's path.
    pub fn run(&self) -> Result<BuildOutput> {
        if !self.warp_source.is_dir() {
            return Err(Error::StageFailed {
                stage: "build",
                message: format!(
                    "warp source directory not found: {}",
                    self.warp_source.display()
                ),
            });
        }

        let started = Instant::now();
        let output = self.command().output().map_err(Error::Io)?;
        let duration = started.elapsed();

        if !output.status.success() {
            return Err(Error::StageFailed {
                stage: "build",
                message: format!(
                    "cargo exited with status {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            });
        }

        let binary_path = self.expected_binary_path();
        if !binary_path.exists() {
            return Err(Error::StageFailed {
                stage: "build",
                message: format!(
                    "cargo succeeded but expected binary not found at {}",
                    binary_path.display()
                ),
            });
        }

        Ok(BuildOutput {
            binary_path,
            stdout: output.stdout,
            stderr: output.stderr,
            duration,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_of(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn defaults_target_warp_package_and_warp_oss_binary() {
        let stage = BuildStage::new("/x/warp");
        assert_eq!(stage.package(), "warp");
        assert_eq!(stage.binary_name(), "warp-oss");
        assert_eq!(stage.profile(), BuildProfile::Debug);
    }

    #[test]
    fn command_is_cargo_build_minus_p_package() {
        let stage = BuildStage::new("/x/warp");
        let cmd = stage.command();
        assert_eq!(cmd.get_program(), "cargo");
        assert_eq!(cmd.get_current_dir(), Some(Path::new("/x/warp")));
        assert_eq!(args_of(&cmd), vec!["build", "-p", "warp"]);
    }

    #[test]
    fn release_profile_adds_release_flag() {
        let stage = BuildStage::new("/x/warp").with_profile(BuildProfile::Release);
        assert!(args_of(&stage.command()).contains(&"--release".to_string()));
    }

    #[test]
    fn extra_args_are_appended() {
        let stage = BuildStage::new("/x/warp").with_extra_args(vec!["--locked".into()]);
        assert!(args_of(&stage.command()).contains(&"--locked".to_string()));
    }

    #[test]
    fn expected_binary_path_uses_debug_dir_by_default() {
        let stage = BuildStage::new("/x/warp");
        assert_eq!(
            stage.expected_binary_path(),
            PathBuf::from("/x/warp/target/debug/warp-oss")
        );
    }

    #[test]
    fn expected_binary_path_uses_release_dir_for_release_profile() {
        let stage = BuildStage::new("/x/warp")
            .with_binary_name("custom")
            .with_profile(BuildProfile::Release);
        assert_eq!(
            stage.expected_binary_path(),
            PathBuf::from("/x/warp/target/release/custom")
        );
    }

    #[test]
    fn run_errors_when_warp_source_missing() {
        let err = BuildStage::new("/no/such/dir").run().unwrap_err();
        match err {
            Error::StageFailed { stage, message } => {
                assert_eq!(stage, "build");
                assert!(message.contains("warp source directory not found"));
            }
            other => panic!("expected StageFailed, got {other:?}"),
        }
    }

    #[test]
    fn with_cargo_path_routes_invocation_to_chosen_binary() {
        let stage = BuildStage::new("/x/warp").with_cargo_path("/usr/local/bin/cargo");
        assert_eq!(stage.command().get_program(), "/usr/local/bin/cargo");
    }
}
