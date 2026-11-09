//! Sessions on disk: the index files under `~/.umbra/sessions`, one per session, and reading a
//! trajectory back.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::models::session::SessionIndexEntry;
use crate::models::trajectory::Trajectory;
use crate::utils::paths::umbra_home;

/// The folder that holds one index file per session.
pub fn sessions_dir() -> PathBuf {
    umbra_home().join("sessions")
}

/// A new session id: the time in milliseconds, which sorts by start time.
pub fn new_session_id() -> String {
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    millis.to_string()
}

/// Writes a session's index entry, through a temporary file so a reader never sees half of it.
/// @param entry - the entry
pub fn save_index(entry: &SessionIndexEntry) -> Result<()> {
    let dir = sessions_dir();
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(format!("{}.json", entry.id));
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_string_pretty(entry)?)?;
    std::fs::rename(&temp, &path).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Reads every index entry, newest first. Unreadable files are skipped.
pub fn list_sessions() -> Vec<SessionIndexEntry> {
    let Ok(entries) = std::fs::read_dir(sessions_dir()) else { return Vec::new() };
    let mut sessions: Vec<SessionIndexEntry> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .filter_map(|text| serde_json::from_str(&text).ok())
        .collect();
    sessions.sort_by(|a, b| b.updated_at.total_cmp(&a.updated_at));
    sessions
}

/// The sessions whose working folder is `cwd`, newest first.
/// @param cwd - the folder
pub fn sessions_in(cwd: &Path) -> Vec<SessionIndexEntry> {
    list_sessions().into_iter().filter(|s| same_folder(Path::new(&s.cwd), cwd)).collect()
}

/// Finds a session by its id, or by the start of its id when only one session matches.
/// @param key - the id or its start
pub fn find_session(key: &str) -> Result<SessionIndexEntry> {
    let sessions = list_sessions();
    if let Some(found) = sessions.iter().find(|s| s.id == key) {
        return Ok(found.clone());
    }
    let matches: Vec<&SessionIndexEntry> = sessions.iter().filter(|s| s.id.starts_with(key)).collect();
    match matches.as_slice() {
        [one] => Ok((*one).clone()),
        [] => bail!("no session '{key}'. `umbra sessions --all` lists them, or pass the path of a trajectory file"),
        many => bail!("'{key}' matches {} sessions: {}", many.len(), many.iter().take(5).map(|s| s.id.as_str()).collect::<Vec<_>>().join(", ")),
    }
}

/// Reads a trajectory file.
/// @param path - the trajectory
pub fn load_trajectory(path: &Path) -> Result<Trajectory> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("{} is not an Umbra trajectory", path.display()))
}

/// Returns true when two paths name the same folder, ignoring slash direction, a trailing slash
/// and, on Windows, letter case.
/// @param a - one folder
/// @param b - the other
pub fn same_folder(a: &Path, b: &Path) -> bool {
    normalize(a) == normalize(b)
}

/// The comparable form of a folder path.
/// @param path - a folder
fn normalize(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let text = text.trim_end_matches('/');
    let text = text.strip_prefix("//?/").unwrap_or(text);
    if cfg!(windows) { text.to_lowercase() } else { text.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_folders_without_caring_about_slashes() {
        assert!(same_folder(Path::new("C:\\work\\umbra\\"), Path::new("C:/work/umbra")));
        assert!(!same_folder(Path::new("/work/a"), Path::new("/work/b")));
    }
}
