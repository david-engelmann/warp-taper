//! Bash-script adapter.
//!
//! Wraps an existing `assertions.sh` (or any executable bash script) so the
//! Rust engine can drive it without losing the back-compat path for the
//! legacy bash scenario format. Stdout of the script is captured in the
//! result message; the per-line `  ✓ ...` / `  ✗ ...` markers that the
//! script emits are extracted by [`crate::assertion::EngineReport::summary_lines`]
//! and folded into the bundle README's assertion summary.

use std::path::PathBuf;
use std::process::Command;

use crate::assertion::{Assertion, AssertionContext, AssertionResult};

#[derive(Debug, Clone)]
pub struct ShellScriptAssertion {
    name: String,
    script_path: PathBuf,
    working_dir: Option<PathBuf>,
}

impl ShellScriptAssertion {
    pub fn new(name: impl Into<String>, script_path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            script_path: script_path.into(),
            working_dir: None,
        }
    }

    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }
}

impl Assertion for ShellScriptAssertion {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self, ctx: &AssertionContext) -> AssertionResult {
        let mut cmd = Command::new("bash");
        cmd.arg(&self.script_path);
        cmd.env("TAPE_DIR", &ctx.tape_dir);
        cmd.env("TAPE_LOGS", &ctx.logs_dir);
        cmd.env("TAPE_SESSION", &ctx.session_log);
        cmd.env("TAPE_MCP_LOGS", &ctx.mcp_logs_dir);
        if let Some(s) = &ctx.scenario_dir {
            cmd.env("SCENARIO_DIR", s);
        }
        if let Some(w) = &ctx.warp_source {
            cmd.env("WARP_SOURCE", w);
        }
        if let Some(d) = &self.working_dir {
            cmd.current_dir(d);
        } else if let Some(d) = &ctx.scenario_dir {
            cmd.current_dir(d);
        }

        let output = match cmd.output() {
            Ok(o) => o,
            Err(e) => {
                return AssertionResult::fail(format!(
                    "failed to invoke bash {}: {e}",
                    self.script_path.display()
                ));
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let message = if stderr.trim().is_empty() {
            stdout
        } else {
            format!("{stdout}--- stderr ---\n{stderr}")
        };

        if output.status.success() {
            AssertionResult::pass(message)
        } else {
            AssertionResult::fail(message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn write_script(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    fn ctx_for(dir: &std::path::Path) -> AssertionContext {
        AssertionContext::from_tape_dir(dir)
    }

    #[test]
    fn passes_on_zero_exit_with_check_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_script(
            tmp.path(),
            "assert.sh",
            "#!/usr/bin/env bash\necho '  ✓ everything is fine'\nexit 0\n",
        );

        let a = ShellScriptAssertion::new("legacy", script);
        let r = a.run(&ctx_for(tmp.path()));
        assert_eq!(r.outcome, crate::assertion::Outcome::Pass);
        assert!(r.message.contains("everything is fine"));
    }

    #[test]
    fn fails_on_non_zero_exit() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_script(
            tmp.path(),
            "assert.sh",
            "#!/usr/bin/env bash\necho '  ✗ something broke'\nexit 1\n",
        );

        let a = ShellScriptAssertion::new("legacy", script);
        let r = a.run(&ctx_for(tmp.path()));
        assert_eq!(r.outcome, crate::assertion::Outcome::Fail);
        assert!(r.message.contains("something broke"));
    }

    #[test]
    fn captures_stderr_in_message() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_script(
            tmp.path(),
            "assert.sh",
            "#!/usr/bin/env bash\necho 'stdout content'\necho 'stderr content' >&2\nexit 0\n",
        );

        let a = ShellScriptAssertion::new("legacy", script);
        let r = a.run(&ctx_for(tmp.path()));
        assert!(r.message.contains("stdout content"));
        assert!(r.message.contains("stderr content"));
        assert!(r.message.contains("--- stderr ---"));
    }

    #[test]
    fn fails_gracefully_when_script_missing() {
        // bash itself runs successfully — but exits non-zero because it
        // cannot open the script. We surface that as a Fail result with
        // bash's own error text in the captured stderr.
        let tmp = tempfile::tempdir().unwrap();
        let a = ShellScriptAssertion::new("legacy", tmp.path().join("nope.sh"));
        let r = a.run(&ctx_for(tmp.path()));
        assert_eq!(r.outcome, crate::assertion::Outcome::Fail);
        assert!(
            r.message.contains("No such file") || r.message.contains("cannot"),
            "expected bash's not-found error in stderr; got: {}",
            r.message
        );
    }

    #[test]
    fn exposes_context_env_vars_to_script() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("logs/mcp")).unwrap();
        let script = write_script(
            tmp.path(),
            "assert.sh",
            "#!/usr/bin/env bash\n\
             echo \"TAPE_DIR=$TAPE_DIR\"\n\
             echo \"TAPE_LOGS=$TAPE_LOGS\"\n\
             echo \"TAPE_SESSION=$TAPE_SESSION\"\n\
             echo \"TAPE_MCP_LOGS=$TAPE_MCP_LOGS\"\n\
             exit 0\n",
        );

        let ctx = AssertionContext::from_tape_dir(tmp.path());
        let r = ShellScriptAssertion::new("env-check", script).run(&ctx);
        assert!(r
            .message
            .contains(&format!("TAPE_DIR={}", tmp.path().display())));
        assert!(r.message.contains("TAPE_LOGS="));
        assert!(r.message.contains("TAPE_SESSION="));
        assert!(r.message.contains("TAPE_MCP_LOGS="));
    }
}
