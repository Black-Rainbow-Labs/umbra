//! Where commands run. An environment takes a command string and returns its output.

pub mod docker;
pub mod local;

use std::path::Path;

use anyhow::{bail, Result};
use serde_json::Value;

use crate::models::action::Observation;
use crate::models::config::EnvironmentConfig;

/// The one thing every environment does: run a command and report what happened.
pub trait Environment {
    /// Runs one command and returns its merged output and exit code. Never panics on a failed
    /// command; a failure is an observation with `returncode` -1 and `exception_info` set.
    /// @param command - the shell command text
    fn execute(&mut self, command: &str) -> Observation;

    /// Values the prompt templates can use, such as the working folder.
    fn template_vars(&self) -> Value;

    /// The settings recorded in the trajectory.
    fn describe(&self) -> Value;

    /// Releases anything the environment holds, such as a container.
    fn close(&mut self) {}
}

/// Builds the environment named by `config.kind`.
/// @param config - the environment settings
/// @param default_cwd - the folder used when the config gives none
pub fn create_environment(config: &EnvironmentConfig, default_cwd: &Path) -> Result<Box<dyn Environment>> {
    match config.kind.as_str() {
        "" | "local" => Ok(Box::new(local::LocalEnvironment::new(config.clone(), default_cwd))),
        "docker" => Ok(Box::new(docker::DockerEnvironment::start(config.clone())?)),
        other => bail!("unknown environment kind '{other}'. Use local or docker"),
    }
}

/// Checks whether a command's output asks to end the run, the mini-swe-agent way: the first
/// non blank line is exactly the sentinel and the command exited 0. Returns the submission,
/// which is everything after that line.
/// @param observation - the result of a command
pub fn submission_from(observation: &Observation) -> Option<String> {
    if observation.returncode != 0 {
        return None;
    }
    let trimmed = observation.output.trim_start();
    let mut lines = trimmed.splitn(2, '\n');
    let first = lines.next()?.trim();
    if first != SUBMIT_SENTINEL {
        return None;
    }
    Some(lines.next().unwrap_or("").to_string())
}

/// The text a command prints to end the run. Kept identical to mini-swe-agent so its prompts
/// and benchmark configs work unchanged.
pub const SUBMIT_SENTINEL: &str = "COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_sentinel_only_on_the_first_line_and_exit_zero() {
        let ok = Observation { output: "\nCOMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT\npatch".into(), returncode: 0, exception_info: String::new() };
        assert_eq!(submission_from(&ok).as_deref(), Some("patch"));
        let failed = Observation { returncode: 1, ..ok.clone() };
        assert_eq!(submission_from(&failed), None);
        let later = Observation { output: "hello\nCOMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT".into(), returncode: 0, exception_info: String::new() };
        assert_eq!(submission_from(&later), None);
    }
}
