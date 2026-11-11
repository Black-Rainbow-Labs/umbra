//! The finish check (task-2617, E3). When the model replies to end its turn right after a
//! command that changed files, with no command run since, it gets one message asking it to run
//! the check that proves the change and to compare the result with each requirement. Once per
//! turn, so it costs at most one extra model call.
//!
//! A change is seen by comparing a fingerprint of the working folder (every file's path, size
//! and modification time) taken just before and just after each command, so only what the
//! command itself changed counts, not Umbra's own trajectory writes between commands. A folder
//! too large to fingerprint quickly is never counted as changed.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::UNIX_EPOCH;

/// The most files fingerprinted before the folder is treated as too large to check.
const MAX_ENTRIES: usize = 20_000;

/// Folders that hold tools' own state, not the user's work.
const SKIPPED_DIRS: [&str; 6] = [".git", "node_modules", "target", "__pycache__", ".venv", ".umbra"];

/// The message the model gets instead of ending its turn.
pub const FINISH_CHECK_TEXT: &str = "You changed files and have not run anything since the last change. Before you reply, run the command that shows the change works (the tests, the program, or a look at the changed output) and check the result against each requirement in the request. Fix anything that does not meet a requirement. Then reply.";

/// What the finish check knows about the current turn.
#[derive(Debug, Default)]
pub struct FinishCheck {
    enabled: bool,
    /// The folder's fingerprint just before the running command, or None when it could not be taken.
    before: Option<u64>,
    /// True when the last command changed files.
    changed: bool,
    /// True once the check was asked this turn.
    asked: bool,
}

impl FinishCheck {
    /// Builds the check, turned on or off.
    /// @param enabled - the `finish_check` setting, false for environments other than local
    pub fn new(enabled: bool) -> Self {
        FinishCheck { enabled, ..Default::default() }
    }

    /// Starts a new turn: forgets the last turn's state.
    pub fn start_turn(&mut self) {
        self.asked = false;
        self.changed = false;
    }

    /// Takes the folder's fingerprint just before a command runs.
    /// @param cwd - the working folder
    pub fn before_command(&mut self, cwd: &Path) {
        self.before = if self.enabled { tree_fingerprint(cwd) } else { None };
    }

    /// Records whether the command that just ran changed the folder.
    /// @param cwd - the working folder
    pub fn after_command(&mut self, cwd: &Path) {
        if !self.enabled {
            return;
        }
        self.changed = match (self.before.take(), tree_fingerprint(cwd)) {
            (Some(before), Some(after)) => before != after,
            _ => false,
        };
    }

    /// Returns true when the model's reply should get the check message instead of ending the
    /// turn, and marks the check as asked.
    pub fn should_ask(&mut self) -> bool {
        if !self.enabled || self.asked || !self.changed {
            return false;
        }
        self.asked = true;
        true
    }
}

/// Hashes the path, size and modification time of every file under a folder, in a fixed order.
/// Returns None for a folder with more than `MAX_ENTRIES` files.
/// @param root - the folder
pub fn tree_fingerprint(root: &Path) -> Option<u64> {
    let mut entries: Vec<(String, u64, u128)> = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else { continue };
        for entry in read.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            let path = entry.path();
            if meta.is_dir() {
                if !SKIPPED_DIRS.iter().any(|name| entry.file_name() == *name) {
                    pending.push(path);
                }
                continue;
            }
            let modified = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
            entries.push((path.strip_prefix(root).unwrap_or(&path).to_string_lossy().into_owned(), meta.len(), modified));
            if entries.len() > MAX_ENTRIES {
                return None;
            }
        }
    }
    entries.sort();
    let mut hasher = DefaultHasher::new();
    entries.hash(&mut hasher);
    Some(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Makes an empty folder under the system temp folder for one test.
    /// @param name - the test's name
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("umbra-finish-check-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn asks_once_after_a_command_that_changed_files() {
        let dir = scratch("asks");
        let mut check = FinishCheck::new(true);
        check.start_turn();
        check.before_command(&dir);
        std::fs::write(dir.join("a.txt"), "one").unwrap();
        check.after_command(&dir);
        assert!(check.should_ask(), "a reply right after a change gets the check");
        assert!(!check.should_ask(), "only once per turn");
        check.start_turn();
        check.before_command(&dir);
        std::fs::write(dir.join("a.txt"), "two!").unwrap();
        check.after_command(&dir);
        std::fs::write(dir.join("written-between-commands.json"), "{}").unwrap();
        check.before_command(&dir);
        check.after_command(&dir);
        assert!(!check.should_ask(), "a command after the change counts as the check, and writes between commands do not count");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn does_not_ask_when_nothing_changed_or_when_off() {
        let dir = scratch("quiet");
        let mut check = FinishCheck::new(true);
        check.start_turn();
        check.before_command(&dir);
        check.after_command(&dir);
        assert!(!check.should_ask());
        let mut off = FinishCheck::new(false);
        off.start_turn();
        off.before_command(&dir);
        std::fs::write(dir.join("b.txt"), "x").unwrap();
        off.after_command(&dir);
        assert!(!off.should_ask());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ignores_changes_inside_git_and_node_modules() {
        let dir = scratch("skip");
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let before = tree_fingerprint(&dir);
        std::fs::write(dir.join(".git").join("index"), "x").unwrap();
        assert_eq!(before, tree_fingerprint(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
