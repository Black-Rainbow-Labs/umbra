//! The folder through which `umbra session <command>` reaches a running session. A session keeps
//! `~/.umbra/control/<session id>/` while it runs: `state.json` says what the session is using now
//! (model, mode, MCP servers), and `requests.jsonl` holds the commands sent to it, one JSON object
//! per line. Every command the agent runs gets `UMBRA_CONTROL_DIR`, `UMBRA_SESSION_ID` and
//! `UMBRA_BIN` in its environment, so the agent can change its own session with Umbra's command
//! line. The session reads and empties `requests.jsonl` before every model call and at the
//! prompt.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::utils::paths::umbra_home;

/// A session folder older than this with no process behind it is removed.
const STALE: Duration = Duration::from_secs(7 * 24 * 3600);

/// What a running session is using, written to `state.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SessionState {
    pub id: String,
    pub pid: u32,
    pub cwd: String,
    pub model: String,
    pub mode: String,
    /// Each MCP server and its state: `running`, `starting`, `failed` or `off`.
    pub mcp: Vec<(String, String)>,
    /// Seconds since 1970 when the state was written.
    pub updated_at: u64,
}

/// One command sent to a running session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "command", rename_all = "lowercase")]
pub enum ControlRequest {
    /// Use this model from the next model call.
    Model { name: String },
    /// Switch to this mode.
    Mode { mode: String },
    /// Turn an MCP server on or off in this session only.
    Mcp { name: String, on: bool },
    /// Summarize the history before the next model call.
    Compact { focus: String },
    /// End the session after the current step.
    Quit,
}

/// The control folder of the session running in this process.
static CURRENT: Mutex<Option<PathBuf>> = Mutex::new(None);

/// The folder every session's control folder is in.
pub fn control_root() -> PathBuf {
    umbra_home().join("control")
}

/// Seconds since 1970.
fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Makes the control folder of a session and makes it this process's session. Stale folders of
/// sessions that ended without removing theirs are removed first.
/// @param session_id - the session id
pub fn open(session_id: &str) -> PathBuf {
    remove_stale();
    let dir = control_root().join(session_id);
    let _ = std::fs::create_dir_all(&dir);
    if let Some(previous) = CURRENT.lock().unwrap_or_else(|e| e.into_inner()).replace(dir.clone()) {
        if previous != dir {
            let _ = std::fs::remove_dir_all(previous);
        }
    }
    dir
}

/// The control folder of the session running in this process, if any.
pub fn current() -> Option<PathBuf> {
    CURRENT.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Removes this process's control folder, when the session ends.
pub fn close() {
    if let Some(dir) = CURRENT.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// The environment a command the agent runs gets, so `umbra session` finds this session.
pub fn env_vars() -> Vec<(String, String)> {
    let Some(dir) = current() else { return Vec::new() };
    let id = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut vars = vec![("UMBRA_CONTROL_DIR".to_string(), dir.to_string_lossy().to_string()), ("UMBRA_SESSION_ID".to_string(), id)];
    if let Ok(exe) = std::env::current_exe() {
        vars.push(("UMBRA_BIN".to_string(), exe.to_string_lossy().to_string()));
    }
    vars
}

/// The state last written, so an unchanged state is not written again before every step.
static LAST: Mutex<Option<SessionState>> = Mutex::new(None);

/// Writes `state.json` in this process's control folder, when it changed since the last write.
/// @param state - what the session uses now
pub fn write_state(mut state: SessionState) {
    let Some(dir) = current() else { return };
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if last.as_ref() == Some(&state) && dir.join("state.json").exists() {
            return;
        }
        *last = Some(state.clone());
    }
    state.pid = std::process::id();
    state.updated_at = now_secs();
    let Ok(text) = serde_json::to_string_pretty(&state) else { return };
    let temporary = dir.join("state.json.tmp");
    if std::fs::write(&temporary, text).is_ok() {
        let _ = std::fs::rename(&temporary, dir.join("state.json"));
    }
}

/// Reads a session's `state.json`.
/// @param dir - the session's control folder
pub fn read_state(dir: &Path) -> Result<SessionState> {
    let text = std::fs::read_to_string(dir.join("state.json")).with_context(|| format!("no running session at {}", dir.display()))?;
    serde_json::from_str(&text).context("the session's state.json cannot be read")
}

/// Sends a command to a session by adding a line to its `requests.jsonl`.
/// @param dir - the session's control folder
/// @param request - the command
pub fn send(dir: &Path, request: &ControlRequest) -> Result<()> {
    if !dir.join("state.json").exists() {
        bail!("no running session at {}", dir.display());
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("requests.jsonl"))?;
    writeln!(file, "{}", serde_json::to_string(request)?)?;
    Ok(())
}

/// Takes every command sent to this process's session since the last call. The file is renamed
/// before it is read, so a line added while it is read goes into a new file and is not lost.
pub fn take_requests() -> Vec<ControlRequest> {
    let Some(dir) = current() else { return Vec::new() };
    let file = dir.join("requests.jsonl");
    if !file.exists() {
        return Vec::new();
    }
    let taking = dir.join("requests.taking");
    if std::fs::rename(&file, &taking).is_err() {
        return Vec::new();
    }
    let text = std::fs::read_to_string(&taking).unwrap_or_default();
    let _ = std::fs::remove_file(&taking);
    text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
}

/// Finds the control folder `umbra session` talks to: the session named on the command line (its
/// id or the start of it), else the one in `UMBRA_CONTROL_DIR`, else the only running session.
/// @param session - the `--session` option
pub fn find(session: Option<&str>) -> Result<PathBuf> {
    if let Some(wanted) = session {
        let matches: Vec<PathBuf> = running().into_iter().filter(|(id, _)| id.starts_with(wanted)).map(|(_, dir)| dir).collect();
        return match matches.len() {
            1 => Ok(matches[0].clone()),
            0 => bail!("no running session has an id that starts with {wanted}"),
            _ => bail!("more than one running session has an id that starts with {wanted}"),
        };
    }
    if let Ok(dir) = std::env::var("UMBRA_CONTROL_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let all = running();
    match all.len() {
        1 => Ok(all[0].1.clone()),
        0 => bail!("no Umbra session is running"),
        _ => bail!("{} sessions are running; name one with --session <id> (umbra session list shows them)", all.len()),
    }
}

/// Every running session's id and control folder.
pub fn running() -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(control_root()) else { return Vec::new() };
    entries.flatten().filter(|e| e.path().join("state.json").exists()).filter(|e| read_state(&e.path()).is_ok_and(|s| process_alive(s.pid))).map(|e| (e.file_name().to_string_lossy().to_string(), e.path())).collect()
}

/// Removes control folders whose process has ended, or that were not written for a week.
fn remove_stale() {
    let Ok(entries) = std::fs::read_dir(control_root()) else { return };
    for entry in entries.flatten() {
        let gone = match read_state(&entry.path()) {
            Ok(state) => !process_alive(state.pid) || now_secs().saturating_sub(state.updated_at) > STALE.as_secs(),
            Err(_) => entry.metadata().and_then(|m| m.modified()).is_ok_and(|t| t.elapsed().unwrap_or_default() > STALE),
        };
        if gone && entry.path() != current().unwrap_or_default() {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// True when a process with this id is running.
/// @param pid - the process id
#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    pid != 0 && unsafe { libc::kill(pid as libc::pid_t, 0) == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) }
}

/// True when a process with this id is running. Windows has no cheap check without more
/// bindings, so a folder counts as running until it goes stale.
/// @param pid - the process id
#[cfg(not(unix))]
fn process_alive(pid: u32) -> bool {
    pid != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_sent_and_taken_once() {
        let dir = std::env::temp_dir().join(format!("umbra-control-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        *CURRENT.lock().unwrap() = Some(dir.clone());
        write_state(SessionState { id: "abc".into(), model: "m".into(), ..Default::default() });
        assert_eq!(read_state(&dir).unwrap().model, "m");
        send(&dir, &ControlRequest::Mode { mode: "yolo".into() }).unwrap();
        send(&dir, &ControlRequest::Quit).unwrap();
        assert_eq!(take_requests(), vec![ControlRequest::Mode { mode: "yolo".into() }, ControlRequest::Quit]);
        assert!(take_requests().is_empty());
        assert!(env_vars().iter().any(|(k, v)| k == "UMBRA_CONTROL_DIR" && *v == dir.to_string_lossy()));
        close();
        assert!(!dir.exists());
    }
}
