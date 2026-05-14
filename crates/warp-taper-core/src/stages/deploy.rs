//! Deploy stage — launches a previously-built binary and tracks its lifetime.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct DeployStage {
    binary_path: PathBuf,
    args: Vec<String>,
    env: HashMap<String, String>,
    capture_output: bool,
    working_dir: Option<PathBuf>,
}

impl DeployStage {
    pub fn new(binary_path: impl Into<PathBuf>) -> Self {
        Self {
            binary_path: binary_path.into(),
            args: Vec::new(),
            env: HashMap::new(),
            capture_output: false,
            working_dir: None,
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Capture stdout/stderr instead of inheriting from the parent.
    pub fn capture_output(mut self) -> Self {
        self.capture_output = true;
        self
    }

    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }

    /// Build the [`Command`] that would be spawned. Exposed for unit tests.
    pub fn command(&self) -> Command {
        let mut cmd = Command::new(&self.binary_path);
        cmd.args(&self.args);
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        if let Some(d) = &self.working_dir {
            cmd.current_dir(d);
        }
        if self.capture_output {
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        } else {
            cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        }
        cmd
    }

    /// Spawn the binary and return a handle. The caller is responsible for
    /// killing the process via [`DeployHandle::kill`] when the scenario ends.
    pub fn run(&self) -> Result<DeployHandle> {
        if !self.binary_path.exists() {
            return Err(Error::StageFailed {
                stage: "deploy",
                message: format!("binary not found: {}", self.binary_path.display()),
            });
        }
        let mut cmd = self.command();
        let child = cmd.spawn().map_err(Error::Io)?;
        Ok(DeployHandle {
            binary_path: self.binary_path.clone(),
            child,
        })
    }
}

#[derive(Debug)]
pub struct DeployHandle {
    binary_path: PathBuf,
    child: Child,
}

impl DeployHandle {
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }

    /// Non-blocking: returns `Some(status)` if the child has exited.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        self.child.try_wait().map_err(Error::Io)
    }

    /// Kill the child and reap it. Returns `Ok(())` if the process was alive
    /// (and is now killed) or already exited.
    pub fn kill(mut self) -> Result<()> {
        match self.child.kill() {
            Ok(()) => {
                self.child.wait().map_err(Error::Io)?;
                Ok(())
            }
            // InvalidInput means the child already exited before kill().
            Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => Ok(()),
            Err(e) => Err(Error::Io(e)),
        }
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
    fn command_routes_binary_args_and_env() {
        let stage = DeployStage::new("/bin/sleep")
            .with_args(vec!["1".into()])
            .with_env("FOO", "bar");
        let cmd = stage.command();
        assert_eq!(cmd.get_program(), "/bin/sleep");
        assert_eq!(args_of(&cmd), vec!["1"]);
        let envs: Vec<(String, Option<String>)> = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|s| s.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert!(envs
            .iter()
            .any(|(k, v)| k == "FOO" && v.as_deref() == Some("bar")));
    }

    #[test]
    fn run_errors_when_binary_missing() {
        let err = DeployStage::new("/no/such/binary").run().unwrap_err();
        match err {
            Error::StageFailed { stage, message } => {
                assert_eq!(stage, "deploy");
                assert!(message.contains("binary not found"));
            }
            other => panic!("expected StageFailed, got {other:?}"),
        }
    }

    #[test]
    #[cfg(unix)]
    fn spawn_and_kill_real_process() {
        // /bin/sleep exists on macOS + Linux runners.
        let stage = DeployStage::new("/bin/sleep").with_args(vec!["60".into()]);
        let mut handle = stage.run().unwrap();
        assert!(handle.pid() > 0);
        assert_eq!(handle.binary_path(), Path::new("/bin/sleep"));

        // The child is still alive.
        assert!(handle.try_wait().unwrap().is_none());

        handle.kill().unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn kill_on_already_exited_child_is_ok() {
        // /bin/echo exits immediately on its own.
        let stage = DeployStage::new("/bin/echo")
            .with_args(vec!["hi".into()])
            .capture_output();
        let mut handle = stage.run().unwrap();
        // Wait for it to exit on its own. Poll briefly — should be fast.
        for _ in 0..50 {
            if handle.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        // Killing a process that already exited is a no-op.
        handle.kill().unwrap();
    }
}
