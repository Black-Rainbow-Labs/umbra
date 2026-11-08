//! Path helpers: the home folder, `~` expansion, and finding the project root.

use std::path::{Path, PathBuf};

/// Returns the user's home folder. `UMBRA_HOME_OVERRIDE` replaces it, which tests use to run
/// against a fake home folder.
pub fn home_dir() -> PathBuf {
    if let Ok(over) = std::env::var("UMBRA_HOME_OVERRIDE") {
        if !over.is_empty() {
            return PathBuf::from(over);
        }
    }
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Returns Umbra's own folder, `~/.umbra`.
pub fn umbra_home() -> PathBuf {
    home_dir().join(".umbra")
}

/// Expands a leading `~` to the home folder.
/// @param raw - a path that may start with `~/`
pub fn expand_tilde(raw: &str) -> PathBuf {
    if raw == "~" {
        return home_dir();
    }
    if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        return home_dir().join(rest);
    }
    PathBuf::from(raw)
}

/// Finds the project root: the nearest folder at or above `start` that holds `.git`.
/// When there is none, `start` itself is the project root.
/// @param start - the folder Umbra was started in
pub fn project_root(start: &Path) -> PathBuf {
    let mut current = Some(start);
    while let Some(dir) = current {
        if dir.join(".git").exists() {
            return dir.to_path_buf();
        }
        current = dir.parent();
    }
    start.to_path_buf()
}

/// Lists the folders from the project root down to `cwd`, root first. These are the folders
/// whose instruction files apply, the way Claude Code and Codex walk them.
/// @param root - the project root
/// @param cwd - the working folder, at or below the root
pub fn folders_from_root(root: &Path, cwd: &Path) -> Vec<PathBuf> {
    let mut folders = Vec::new();
    let mut current = Some(cwd);
    while let Some(dir) = current {
        folders.push(dir.to_path_buf());
        if dir == root {
            break;
        }
        current = dir.parent();
    }
    if !folders.iter().any(|f| f == root) {
        folders = vec![cwd.to_path_buf()];
    }
    folders.reverse();
    folders
}

/// Makes a path absolute and removes the `\\?\` prefix Windows adds to canonical paths, so the
/// path matches the plain form other tools write in their config files.
/// @param path - a path that exists
pub fn absolute_path(path: &Path) -> PathBuf {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = resolved.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => resolved,
    }
}

/// Returns a stable key for a path so the same file reached two ways is loaded once.
/// @param path - any existing path
pub fn canonical_key(path: &Path) -> String {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    resolved.to_string_lossy().to_lowercase()
}

/// Returns the platform's application data folder for a program's files, such as Claude Desktop's.
pub fn app_config_dir() -> PathBuf {
    if std::env::var("UMBRA_HOME_OVERRIDE").map(|v| !v.is_empty()).unwrap_or(false) {
        return home_dir().join("AppData").join("Roaming");
    }
    dirs::config_dir().unwrap_or_else(|| home_dir().join(".config"))
}
