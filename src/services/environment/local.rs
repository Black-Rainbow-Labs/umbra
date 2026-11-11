//! Runs each command in a fresh shell on this machine, the way mini-swe-agent's
//! LocalEnvironment does. Nothing carries over between commands: `cd` and `export` are lost.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use wait_timeout::ChildExt;

use super::Environment;
use crate::models::action::Observation;
use crate::models::config::EnvironmentConfig;
use crate::utils::interrupt;
use crate::utils::process::{own_process_group, ProcessJob};

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
        process.args(args).current_dir(&self.cwd).envs(&self.config.env).envs(crate::services::control_service::env_vars()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        own_process_group(&mut process);
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

    /// Values the prompt templates can use: the platform, the working folder, the timeout, and the
    /// path of the running Umbra binary, which the agent can run to change Umbra itself.
    fn template_vars(&self) -> Value {
        let umbra_bin = std::env::current_exe().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        json!({ "cwd": self.cwd.to_string_lossy(), "timeout": self.config.timeout, "shell": self.shell.join(" "), "environment_kind": "local", "umbra_bin": umbra_bin })
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

/// How a command's wait ended.
enum Ending {
    Exited(Option<i32>),
    TimedOut,
    /// Ctrl+C was pressed, after this many seconds.
    Interrupted(f64),
}

/// Waits for the command, collecting both pipes. It is killed when the timeout passes or when
/// the person presses Ctrl+C, and the output so far is kept either way.
/// @param child - the running shell
/// @param timeout - how long to wait
/// @param command - the command text, for the timeout message
fn wait_for_output(mut child: Child, timeout: Duration, command: &str) -> Observation {
    let job = ProcessJob::track(&child);
    let stdout = spawn_reader(child.stdout.take());
    let stderr = spawn_reader(child.stderr.take());
    let ending = wait_for_exit(&mut child, timeout);
    if !matches!(ending, Ending::Exited(_)) {
        if let Some(job) = &job {
            job.kill();
        }
        kill_tree(&mut child);
    }
    let grace = Duration::from_secs(2);
    let mut output = stdout.recv_timeout(grace).unwrap_or_default();
    output.push_str(&stderr.recv_timeout(grace).unwrap_or_default());
    match ending {
        Ending::Exited(code) => Observation { output, returncode: code.unwrap_or(-1), exception_info: String::new(), images: Vec::new(), ..Default::default() },
        Ending::TimedOut => Observation::failed(output, format!("An error occurred while executing the command: Command '{command}' timed out after {} seconds", timeout.as_secs())),
        Ending::Interrupted(seconds) => Observation::failed(output, format!("{INTERRUPTED_COMMAND} after {seconds:.1} seconds")),
    }
}

/// The start of the error a command gets when Ctrl+C stops it.
pub const INTERRUPTED_COMMAND: &str = "The user interrupted this command (Ctrl+C)";

/// Waits for the shell to exit, checking for Ctrl+C every 100 ms.
/// @param child - the running shell
/// @param timeout - how long to wait
fn wait_for_exit(child: &mut Child, timeout: Duration) -> Ending {
    let started = Instant::now();
    loop {
        match child.wait_timeout(interrupt::POLL) {
            Ok(Some(status)) => return Ending::Exited(status.code()),
            Ok(None) => {}
            Err(_) => return Ending::Exited(None),
        }
        if interrupt::is_pending() {
            return Ending::Interrupted(started.elapsed().as_secs_f64());
        }
        if started.elapsed() >= timeout {
            return Ending::TimedOut;
        }
    }
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
