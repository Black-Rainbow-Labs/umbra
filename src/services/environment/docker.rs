//! Runs commands inside a Docker container started for the run, like mini-swe-agent's
//! DockerEnvironment: `docker run -d ... sleep 2h` once, then `docker exec` per command.

use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::Environment;
use super::local::LocalEnvironment;
use crate::models::action::Observation;
use crate::models::config::EnvironmentConfig;

/// A container that lives for one run.
pub struct DockerEnvironment {
    config: EnvironmentConfig,
    container_id: String,
    runner: LocalEnvironment,
}

impl DockerEnvironment {
    /// Starts the container.
    /// @param config - the environment settings; `image` is required
    pub fn start(config: EnvironmentConfig) -> Result<Self> {
        if config.image.is_empty() {
            bail!("environment.image must be set for the docker environment");
        }
        let cwd = if config.cwd.is_empty() { "/".to_string() } else { config.cwd.clone() };
        let name = format!("umbra-{:08x}", std::process::id() ^ (now_nanos() as u32));
        let mut args = vec!["run".to_string(), "-d".into(), "--name".into(), name, "-w".into(), cwd];
        args.extend(config.run_args.iter().cloned());
        args.extend([config.image.clone(), "sleep".into(), "2h".into()]);
        let out = Command::new(&config.docker_executable).args(&args).output().context("could not run docker")?;
        if !out.status.success() {
            bail!("docker run failed: {}", String::from_utf8_lossy(&out.stderr));
        }
        let container_id = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let runner_config = EnvironmentConfig { shell: vec![config.docker_executable.clone()], env: Default::default(), ..config.clone() };
        let runner = LocalEnvironment::new(runner_config, &std::env::current_dir()?);
        Ok(DockerEnvironment { config, container_id, runner })
    }

    /// Builds the `docker exec` arguments for one command.
    /// @param command - the shell command text
    fn exec_args(&self, command: &str) -> Vec<String> {
        let cwd = if self.config.cwd.is_empty() { "/".to_string() } else { self.config.cwd.clone() };
        let mut args = vec!["exec".to_string(), "-w".into(), cwd];
        for key in &self.config.forward_env {
            if let Ok(value) = std::env::var(key) {
                args.extend(["-e".into(), format!("{key}={value}")]);
            }
        }
        for (key, value) in &self.config.env {
            args.extend(["-e".into(), format!("{key}={value}")]);
        }
        let shell = if self.config.shell.is_empty() { vec!["bash".to_string(), "-lc".into()] } else { self.config.shell.clone() };
        args.push(self.container_id.clone());
        args.extend(shell);
        args.push(format!("exec 2>&1\n{command}"));
        args
    }
}

impl Environment for DockerEnvironment {
    /// Runs one command in the container.
    /// @param command - the shell command text
    fn execute(&mut self, command: &str) -> Observation {
        let args = self.exec_args(command);
        self.runner.execute_args(&args, command)
    }

    /// Values the prompt templates can use.
    fn template_vars(&self) -> Value {
        json!({ "cwd": if self.config.cwd.is_empty() { "/" } else { &self.config.cwd }, "timeout": self.config.timeout, "image": self.config.image, "environment_kind": "docker" })
    }

    /// The settings recorded in the trajectory.
    fn describe(&self) -> Value {
        json!({ "environment": self.config, "environment_type": "umbra.docker", "container_id": self.container_id })
    }

    /// Removes the container.
    fn close(&mut self) {
        let _ = Command::new(&self.config.docker_executable).args(["rm", "-f", &self.container_id]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
}

/// Nanoseconds since the Unix epoch, used to make container names unique.
fn now_nanos() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}
