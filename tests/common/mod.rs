//! Helpers shared by the integration tests: a throwaway folder, running the real binary, and
//! reading the trajectory it wrote.

#![allow(dead_code)]

pub mod screen;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

/// A folder that is deleted when the test ends.
pub struct Scratch {
    pub root: PathBuf,
}

impl Scratch {
    /// Creates an empty folder under the system temp folder, unique to this test.
    /// @param name - the test's name
    pub fn new(name: &str) -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("umbra-test-{name}-{stamp}"));
        std::fs::create_dir_all(&root).unwrap();
        Scratch { root }
    }

    /// Writes a file, creating its folders.
    /// @param relative - the path inside the scratch folder
    /// @param text - the file content
    pub fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    /// Returns a path inside the scratch folder.
    /// @param relative - the path inside the scratch folder
    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
}

impl Drop for Scratch {
    /// Removes the folder.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Runs the umbra binary with a fake home folder, so the tests never read the real one.
/// @param home - the fake home folder
/// @param args - the command line
pub fn run_umbra(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_umbra"))
        .args(args)
        .env("UMBRA_HOME_OVERRIDE", home)
        .env("UMBRA_RETRY_BASE_MS", "10")
        .env("MSYS_NO_PATHCONV", "1")
        .env("NO_COLOR", "1")
        .env("UMBRA_UPDATE_CHECK", "0")
        .env_remove("CODEX_HOME")
        .env_remove("UMBRA_MODEL")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("umbra binary runs")
}

/// Reads a trajectory file.
/// @param path - the trajectory path
pub fn read_trajectory(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("trajectory exists")).expect("trajectory is JSON")
}

/// Returns stdout and stderr as one string, for assertion messages.
/// @param output - the process output
pub fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

/// Turns a path into the forward slash form used on command lines and in config files.
/// @param path - any path
pub fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Builds the umbra command with a fake home folder, the same environment as `run_umbra`.
/// @param home - the fake home folder
/// @param args - the command line
pub fn umbra_command(home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_umbra"));
    command
        .args(args)
        .env("UMBRA_HOME_OVERRIDE", home)
        .env("UMBRA_RETRY_BASE_MS", "10")
        .env("MSYS_NO_PATHCONV", "1")
        .env("NO_COLOR", "1")
        .env_remove("CODEX_HOME")
        .env_remove("UMBRA_MODEL")
        .env_remove("UMBRA_FORCE_INTERACTIVE")
        .env("UMBRA_UPDATE_CHECK", "0");
    command
}

/// Runs umbra with extra environment variables and the given text on stdin.
/// @param home - the fake home folder
/// @param args - the command line
/// @param envs - extra environment variables
/// @param input - what to type on stdin
pub fn run_umbra_with(home: &Path, args: &[&str], envs: &[(&str, &str)], input: &str) -> Output {
    let mut command = umbra_command(home, args);
    command.envs(envs.iter().copied()).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = command.spawn().expect("umbra binary runs");
    let mut stdin = child.stdin.take().unwrap();
    std::io::Write::write_all(&mut stdin, input.as_bytes()).unwrap();
    drop(stdin);
    child.wait_with_output().expect("umbra finishes")
}

/// A running umbra that a test can type into and send Ctrl+C to.
pub struct Running {
    pub child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
}

impl Running {
    /// Starts umbra in its own process group with piped stdio, so a console event can be sent
    /// to it alone.
    /// @param home - the fake home folder
    /// @param args - the command line
    /// @param envs - extra environment variables
    pub fn start(home: &Path, args: &[&str], envs: &[(&str, &str)]) -> Self {
        let mut command = umbra_command(home, args);
        command.envs(envs.iter().copied()).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        own_group(&mut command);
        let mut child = command.spawn().expect("umbra binary runs");
        let stdin = child.stdin.take();
        Running { child, stdin }
    }

    /// Types a line on umbra's stdin.
    /// @param line - the text, without the line ending
    pub fn type_line(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        std::io::Write::write_all(stdin, format!("{line}\n").as_bytes()).unwrap();
        std::io::Write::flush(stdin).unwrap();
    }

    /// Closes stdin, which is the end of input for umbra.
    pub fn close_stdin(&mut self) {
        self.stdin.take();
    }

    /// Sends the console event Ctrl+C sends. Returns false when this machine cannot send one,
    /// for example a Windows test run with no console.
    pub fn ctrl_c(&self) -> bool {
        send_interrupt(self.child.id())
    }

    /// Waits for umbra to exit, at most `limit`, and returns its output. Kills it and panics
    /// when it does not exit in time.
    /// @param limit - the longest wait
    pub fn finish(mut self, limit: std::time::Duration) -> Output {
        self.close_stdin();
        let started = std::time::Instant::now();
        while started.elapsed() < limit {
            if self.child.try_wait().unwrap().is_some() {
                return self.child.wait_with_output().unwrap();
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let output = self.child.wait_with_output().unwrap();
        panic!("umbra did not exit within {limit:?}:\n{}", text(&output));
    }
}

/// Puts the child in a new process group.
/// @param command - the command being built
#[cfg(windows)]
fn own_group(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0000_0200);
}

/// Puts the child in a new process group.
/// @param command - the command being built
#[cfg(unix)]
fn own_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

/// Sends Ctrl+Break to the process group, which Umbra's handler takes the same as Ctrl+C.
/// Windows cannot send Ctrl+C itself to one group.
/// @param pid - the group leader
#[cfg(windows)]
fn send_interrupt(pid: u32) -> bool {
    use windows_sys::Win32::System::Console::{GenerateConsoleCtrlEvent, CTRL_BREAK_EVENT};
    // SAFETY: plain Win32 call with a process group id.
    unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid) != 0 }
}

/// Sends SIGINT, which a terminal sends for Ctrl+C.
/// @param pid - the process
#[cfg(unix)]
fn send_interrupt(pid: u32) -> bool {
    Command::new("kill").args(["-INT", &pid.to_string()]).status().is_ok_and(|s| s.success())
}

/// Writes a replay script and returns the `-m` value for it.
/// @param scratch - the test folder
/// @param name - the file name
/// @param replies - the scripted replies
pub fn replay_model(scratch: &Scratch, name: &str, replies: &Value) -> String {
    let path = scratch.write(name, &replies.to_string());
    format!("replay/{}", slash(&path))
}

/// Reads the requests the replay provider recorded, one JSON value per line.
/// @param path - the record file
pub fn recorded_requests(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path).unwrap_or_default().lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect()
}

/// Writes a PNG of the given size and returns its path.
/// @param scratch - the test folder
/// @param name - the file name
/// @param width - the width in pixels
/// @param height - the height in pixels
pub fn write_png(scratch: &Scratch, name: &str, width: u32, height: u32) -> PathBuf {
    let path = scratch.path(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::DynamicImage::new_rgb8(width, height).save_with_format(&path, image::ImageFormat::Png).unwrap();
    path
}
