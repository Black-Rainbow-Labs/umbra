//! Helpers shared by the integration tests: a throwaway folder, running the real binary, and
//! reading the trajectory it wrote.

#![allow(dead_code)]

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
