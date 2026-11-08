//! The stdio transport: Umbra starts the server and exchanges one JSON message per line.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;

use super::transport::{is_server_request, notification_message, reply_to_server_request, request_message, response_result, Transport};

/// A running stdio server.
pub struct StdioTransport {
    child: Child,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    responses: Receiver<Value>,
    next_id: u64,
}

impl StdioTransport {
    /// Starts the server process.
    /// @param command - the program
    /// @param args - its arguments
    /// @param env - extra environment variables
    /// @param cwd - the working folder, if the config set one
    /// @param log_path - where the server's stderr is written
    pub fn start(command: &str, args: &[String], env: &[(String, String)], cwd: Option<&str>, log_path: &Path) -> Result<Self> {
        let (program, program_args) = windows_command(command, args);
        let mut process = Command::new(&program);
        process.args(&program_args).envs(env.iter().cloned()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(dir) = cwd.filter(|d| !d.is_empty()) {
            process.current_dir(dir);
        }
        let mut child = process.spawn().with_context(|| format!("could not start MCP server '{command}'"))?;
        let stdin = Arc::new(Mutex::new(Some(child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?)));
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
        let stderr = child.stderr.take().ok_or_else(|| anyhow!("no stderr"))?;
        let responses = spawn_stdout_reader(stdout, stdin.clone());
        spawn_stderr_logger(stderr, log_path.to_path_buf());
        Ok(StdioTransport { child, stdin, responses, next_id: 1 })
    }

    /// Writes one message as a line.
    /// @param message - the JSON-RPC message
    fn write(&self, message: &Value) -> Result<()> {
        let mut guard = self.stdin.lock().map_err(|_| anyhow!("MCP stdin lock poisoned"))?;
        let stdin = guard.as_mut().ok_or_else(|| anyhow!("MCP connection is closed"))?;
        writeln!(stdin, "{message}").context("MCP server closed its stdin")?;
        stdin.flush()?;
        Ok(())
    }
}

impl Transport for StdioTransport {
    /// Sends a request and waits for the response with the same id.
    /// @param method - the method name
    /// @param params - the params object
    /// @param timeout - how long to wait
    fn request(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.write(&request_message(id, method, params))?;
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.responses.recv_timeout(left) {
                Ok(message) if message["id"].as_u64() == Some(id) => return response_result(&message),
                Ok(_) => continue,
                Err(RecvTimeoutError::Timeout) => {
                    let _ = self.write(&notification_message("notifications/cancelled", serde_json::json!({ "requestId": id, "reason": "timeout" })));
                    bail!("MCP server did not answer {method} within {} seconds", timeout.as_secs());
                }
                Err(RecvTimeoutError::Disconnected) => bail!("MCP server exited while Umbra waited for {method}"),
            }
        }
    }

    /// Sends a notification.
    /// @param method - the method name
    /// @param params - the params object
    fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        self.write(&notification_message(method, params))
    }

    /// Stdio needs no version header.
    fn set_protocol_version(&mut self, _version: &str) {}

    /// Closes stdin, which tells the server to exit, waits half a second for it, then kills it.
    fn close(&mut self) {
        if let Ok(mut stdin) = self.stdin.lock() {
            stdin.take();
        }
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// On Windows, `npx`, `npm` and other `.cmd` and `.bat` launchers cannot be started directly, so
/// they run through `cmd /c`. Everything else, and every other platform, is unchanged.
/// @param command - the configured program
/// @param args - its arguments
fn windows_command(command: &str, args: &[String]) -> (String, Vec<String>) {
    if !cfg!(windows) {
        return (command.to_string(), args.to_vec());
    }
    let lower = command.to_lowercase();
    let is_script = lower.ends_with(".cmd") || lower.ends_with(".bat") || (!lower.contains('.') && resolves_to_script(command));
    if !is_script {
        return (command.to_string(), args.to_vec());
    }
    let mut cmd_args = vec!["/c".to_string(), command.to_string()];
    cmd_args.extend(args.iter().cloned());
    ("cmd".to_string(), cmd_args)
}

/// Returns true when a bare program name is found on PATH only as a `.cmd` or `.bat` file.
/// @param command - a program name with no extension
fn resolves_to_script(command: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    for dir in std::env::split_paths(&path) {
        if dir.join(format!("{command}.exe")).exists() {
            return false;
        }
        if dir.join(format!("{command}.cmd")).exists() || dir.join(format!("{command}.bat")).exists() {
            return true;
        }
    }
    false
}

/// Reads the server's stdout line by line. Responses go to the returned channel; requests from
/// the server are answered at once; anything that is not JSON is ignored.
/// @param stdout - the server's stdout
/// @param stdin - the server's stdin, for answering its requests
fn spawn_stdout_reader(stdout: std::process::ChildStdout, stdin: Arc<Mutex<Option<ChildStdin>>>) -> Receiver<Value> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let Some(message) = parse_message_line(&line) else { continue };
            if is_server_request(&message) {
                if let Ok(mut guard) = stdin.lock() {
                    if let Some(pipe) = guard.as_mut() {
                        let _ = writeln!(pipe, "{}", reply_to_server_request(&message));
                        let _ = pipe.flush();
                    }
                }
            } else if message.get("id").is_some() && sender.send(message).is_err() {
                break;
            }
        }
    });
    receiver
}

/// Parses one stdout line as a JSON-RPC message. A server that prints text in front of a
/// message on the same line breaks the protocol, but the message after the text is still used.
/// @param line - one line of the server's stdout
fn parse_message_line(line: &str) -> Option<Value> {
    let trimmed = line.trim();
    if let Ok(message) = serde_json::from_str::<Value>(trimmed) {
        return Some(message);
    }
    let start = trimmed.find('{')?;
    serde_json::from_str::<Value>(&trimmed[start..]).ok().filter(|m| m.get("jsonrpc").is_some())
}

/// Copies the server's stderr into a log file so its logging never fills the pipe.
/// @param stderr - the server's stderr
/// @param log_path - the log file
fn spawn_stderr_logger(stderr: std::process::ChildStderr, log_path: std::path::PathBuf) {
    std::thread::spawn(move || {
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut file = std::fs::File::create(&log_path).ok();
        for line in BufReader::new(stderr).lines().map_while(|l| l.ok()) {
            if let Some(f) = file.as_mut() {
                let _ = writeln!(f, "{line}");
            }
        }
    });
}
