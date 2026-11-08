//! Runs each command in a fresh shell on this machine, the way mini-swe-agent's
//! LocalEnvironment does. Nothing carries over between commands: `cd` and `export` are lost.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Value};
use wait_timeout::ChildExt;

use super::Environment;
use crate::models::action::Observation;
use crate::models::config::EnvironmentConfig;

/// A shell on the host.
pub struct LocalEnvironment {
    config: EnvironmentConfig,
    cwd: PathBuf,
    shell: Vec<String>,
}

impl LocalEnvironment {
    /// Builds the environment and picks the shell.
    /// @param config - the environment settings
    /// @param default_cwd - the folder used when the config gives none
    pub fn new(config: EnvironmentConfig, default_cwd: &Path) -> Self {
        let cwd = if config.cwd.is_empty() { default_cwd.to_path_buf() } else { PathBuf::from(&config.cwd) };
        let shell = if config.shell.is_empty() { find_default_shell() } else { config.shell.clone() };
        LocalEnvironment { config, cwd, shell }
    }

    /// Runs the first shell entry as a program with the given arguments, with the timeout and
    /// output capture of `execute`. The Docker environment uses this to run `docker exec`.
    /// @param args - the program's arguments
    /// @param label - the command text, for the timeout message
    pub fn execute_args(&self, args: &[String], label: &str) -> Observation {
        let mut process = Command::new(&self.shell[0]);
        process.args(args).current_dir(&self.cwd).envs(&self.config.env).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        set_own_process_group(&mut process);
        match process.spawn() {
            Ok(child) => wait_for_output(child, Duration::from_secs(self.config.timeout.max(1)), label),
            Err(e) => Observation::failed("", format!("An error occurred while executing the command: could not start {}: {e}", self.shell[0])),
        }
    }
}

impl Environment for LocalEnvironment {
    /// Runs one command with stdout and stderr merged, killing it after the timeout.
    /// @param command - the shell command text
    fn execute(&mut self, command: &str) -> Observation {
        let mut args: Vec<String> = self.shell[1..].to_vec();
        args.push(wrap_command(&self.shell, command));
        self.execute_args(&args, command)
    }

    /// Values the prompt templates can use: the platform, the working folder and the timeout.
    fn template_vars(&self) -> Value {
        json!({ "cwd": self.cwd.to_string_lossy(), "timeout": self.config.timeout, "shell": self.shell.join(" "), "environment_kind": "local" })
    }

    /// The settings recorded in the trajectory.
    fn describe(&self) -> Value {
        json!({ "environment": self.config, "environment_type": "umbra.local", "shell": self.shell })
    }
}

/// Makes the shell send stderr to stdout so the output keeps its original order. Only shells
/// that understand `exec 2>&1` get it; anything else has its stderr appended after stdout.
/// @param shell - the shell program and flags
/// @param command - the command text
fn wrap_command(shell: &[String], command: &str) -> String {
    let program = Path::new(&shell[0]).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    if ["bash", "sh", "zsh", "dash"].contains(&program.as_str()) { format!("exec 2>&1\n{command}") } else { command.to_string() }
}

/// Picks bash. On Windows that is Git for Windows' bash, never `System32\bash.exe`, which runs
/// commands inside WSL instead of on Windows. Falls back to `cmd /C` when no bash exists.
pub fn find_default_shell() -> Vec<String> {
    if !cfg!(windows) {
        return vec!["bash".into(), "-c".into()];
    }
    let candidates = [r"C:\Program Files\Git\bin\bash.exe", r"C:\Program Files (x86)\Git\bin\bash.exe"];
    if let Some(found) = candidates.iter().find(|p| Path::new(p).exists()) {
        return vec![found.to_string(), "-c".into()];
    }
    if let Some(found) = bash_on_path() {
        return vec![found.to_string_lossy().into_owned(), "-c".into()];
    }
    vec!["cmd".into(), "/C".into()]
}

/// Searches PATH for bash.exe, skipping the WSL launcher in the Windows folder.
fn bash_on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join("bash.exe")).find(|p| p.exists() && !p.to_string_lossy().to_lowercase().contains(r"\windows\"))
}

/// Puts the shell in its own process group on Unix so a timeout can kill everything it started.
/// @param process - the command being built
#[cfg(unix)]
fn set_own_process_group(process: &mut Command) {
    use std::os::unix::process::CommandExt;
    process.process_group(0);
}

/// Windows needs nothing here; `taskkill /T` kills the process tree instead.
#[cfg(not(unix))]
fn set_own_process_group(_process: &mut Command) {}

/// Waits for the command, collecting both pipes, and kills it when the timeout passes.
/// @param child - the running shell
/// @param timeout - how long to wait
/// @param command - the command text, for the timeout message
fn wait_for_output(mut child: Child, timeout: Duration, command: &str) -> Observation {
    let stdout = spawn_reader(child.stdout.take());
    let stderr = spawn_reader(child.stderr.take());
    let status = child.wait_timeout(timeout).ok().flatten();
    let timed_out = status.is_none();
    if timed_out {
        kill_tree(&mut child);
    }
    let grace = Duration::from_secs(2);
    let mut output = stdout.recv_timeout(grace).unwrap_or_default();
    output.push_str(&stderr.recv_timeout(grace).unwrap_or_default());
    if timed_out {
        let message = format!("An error occurred while executing the command: Command '{command}' timed out after {} seconds", timeout.as_secs());
        return Observation::failed(output, message);
    }
    let returncode = status.and_then(|s| s.code()).unwrap_or(-1);
    Observation { output, returncode, exception_info: String::new() }
}

/// Reads a pipe to the end on its own thread and sends the text back. Reading on a thread means
/// a full pipe can never block the command, and a pipe held open by a leftover grandchild
/// cannot block Umbra.
/// @param pipe - the child's stdout or stderr
fn spawn_reader<R: Read + Send + 'static>(pipe: Option<R>) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        let _ = sender.send(String::from_utf8_lossy(&bytes).into_owned());
    });
    receiver
}

/// Kills the shell and everything it started.
/// @param child - the shell process Umbra started
fn kill_tree(child: &mut Child) {
    let pid = child.id().to_string();
    if cfg!(windows) {
        let _ = Command::new("taskkill").args(["/F", "/T", "/PID", &pid]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    } else {
        let _ = Command::new("kill").args(["-9", &format!("-{pid}")]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
    let _ = child.kill();
    let _ = child.wait();
}
