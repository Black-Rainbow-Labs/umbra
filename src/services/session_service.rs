//! Resuming a saved session: finding it from `--resume` or `--continue`, loading its history,
//! and repairing a history that stopped in the middle of a step.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::models::compaction::CompactionRecord;
use crate::models::message::{Message, Role};
use crate::models::session::{SessionIndexEntry, SessionInfo};
use crate::models::trajectory::ModelStats;
use crate::models::trajectory::Trajectory;
use crate::repositories::journal_repository::{journal_is_newest, journal_path, load_journal};
use crate::repositories::session_repository::{find_session, list_sessions, load_trajectory, new_session_id, sessions_in};
use crate::repositories::trajectory_repository::save_trajectory;
use crate::utils::console::read_line;

/// The result given to a tool call that never got one because the session stopped first.
pub const NOT_RUN_SESSION_ENDED: &str = "This action was not run: the session ended before it finished.";

/// A saved session, loaded and ready to continue.
#[derive(Debug, Clone)]
pub struct ResumedSession {
    pub messages: Vec<Message>,
    pub stats: ModelStats,
    pub compactions: Vec<CompactionRecord>,
    pub session: SessionInfo,
    pub trajectory_path: PathBuf,
    /// How many tool calls with no result were given one.
    pub repaired: usize,
}

/// Finds the trajectory to resume. `--continue` takes this folder's newest session. `--resume`
/// takes an id, the start of an id, or a trajectory path; with no value it lists this folder's
/// sessions to pick from.
/// @param resume - the `--resume` value: None when not given, empty when given with no value
/// @param continue_last - `--continue`
/// @param cwd - the folder the person is in
/// @param interactive - whether Umbra may ask
pub fn find_resume_target(resume: Option<&str>, continue_last: bool, cwd: &Path, interactive: bool) -> Result<Option<PathBuf>> {
    if continue_last {
        let Some(newest) = sessions_in(cwd).into_iter().next() else { bail!("no saved session in {}. `umbra sessions --all` lists every folder's sessions", cwd.display()) };
        return Ok(Some(PathBuf::from(newest.trajectory)));
    }
    match resume {
        None => Ok(None),
        Some("") => pick_session(cwd, interactive).map(Some),
        Some(key) if Path::new(key).is_file() || journal_path(Path::new(key)).is_file() => Ok(Some(trajectory_for(Path::new(key)))),
        Some(key) => Ok(Some(PathBuf::from(find_session(key)?.trajectory))),
    }
}

/// Writes a saved session as a trajectory JSON file, for `umbra export`, and prints where.
/// @param args - the session to export and where to write it
pub fn export_command(args: &crate::cli::ExportArgs) -> Result<()> {
    let key = args.session.as_str();
    let path = if Path::new(key).is_file() || journal_path(Path::new(key)).is_file() { trajectory_for(Path::new(key)) } else { PathBuf::from(find_session(key)?.trajectory) };
    let trajectory = load_session_record(&path)?;
    let out = args.output.clone().unwrap_or_else(|| path.clone());
    save_trajectory(&out, &trajectory)?;
    println!("{}", out.display());
    Ok(())
}

/// The trajectory path a session file names. A journal path (`x.traj.journal.jsonl`) names the
/// session of `x.traj.json`, since a session saved without `-o` has only its journal.
/// @param path - a trajectory or journal path
pub fn trajectory_for(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_suffix(".journal.jsonl") {
        Some(stem) => PathBuf::from(format!("{stem}.json")),
        None => path.to_path_buf(),
    }
}

/// Lists this folder's newest sessions and asks which one to resume.
/// @param cwd - the folder the person is in
/// @param interactive - whether Umbra may ask
fn pick_session(cwd: &Path, interactive: bool) -> Result<PathBuf> {
    if !interactive {
        bail!("--resume needs a session id or path when stdin is not a terminal");
    }
    let sessions: Vec<SessionIndexEntry> = sessions_in(cwd).into_iter().take(10).collect();
    if sessions.is_empty() {
        bail!("no saved session in {}", cwd.display());
    }
    for (number, session) in sessions.iter().enumerate() {
        println!("{:>3}. {}", number + 1, describe_session(session));
    }
    let answer = read_line("Session number to resume: ").unwrap_or_default();
    match answer.trim().parse::<usize>() {
        Ok(n) if (1..=sessions.len()).contains(&n) => Ok(PathBuf::from(&sessions[n - 1].trajectory)),
        _ => bail!("no session chosen"),
    }
}

/// One line about a session, for the picker and `umbra sessions`.
/// @param session - the index entry
pub fn describe_session(session: &SessionIndexEntry) -> String {
    let status = if session.exit_status.is_empty() { "unfinished" } else { session.exit_status.as_str() };
    let task: String = session.task.chars().take(70).collect();
    format!("{}  {:<10} {:<12} {:>3} calls  {}  {}", session.id, time_ago(session.updated_at), status, session.api_calls, session.model, task)
}

/// How long ago a time was, in the largest whole unit.
/// @param seconds - seconds since the Unix epoch
pub fn time_ago(seconds: f64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    let age = (now - seconds).max(0.0) as u64;
    match age {
        0..60 => format!("{age}s ago"),
        60..3600 => format!("{}m ago", age / 60),
        3600..86_400 => format!("{}h ago", age / 3600),
        _ => format!("{}d ago", age / 86_400),
    }
}

/// Reads a saved session: from its journal when it is at least as new as the trajectory file,
/// since the journal has every step and the trajectory JSON only the end of each turn, otherwise
/// from the trajectory (a trajectory changed after the journal, by a person or a tool, wins). A
/// journal that cannot be read falls back to the trajectory.
/// @param path - the trajectory file
pub fn load_session_record(path: &Path) -> Result<Trajectory> {
    let journal = journal_path(path);
    if journal_is_newest(path) {
        match load_journal(&journal) {
            Ok(trajectory) => return Ok(trajectory),
            Err(error) if !path.is_file() => return Err(error),
            Err(_) => {}
        }
    }
    load_trajectory(path)
}

/// Loads a trajectory to continue it: removes the exit marker, gives unanswered tool calls a
/// result, and fills in the session block for trajectories written before Umbra had one.
/// @param path - the trajectory file
pub fn load_resumed(path: &Path) -> Result<ResumedSession> {
    let trajectory = load_session_record(path)?;
    let mut messages = trajectory.messages;
    while messages.last().is_some_and(|m| m.role == Role::Exit) {
        messages.pop();
    }
    if !messages.iter().any(|m| m.role == Role::System) {
        bail!("{} has no system message, so it cannot be resumed", path.display());
    }
    let repaired = repair_history(&mut messages);
    let mut session = trajectory.info.session;
    if session.id.is_empty() {
        session.id = session_id_from_path(path).unwrap_or_else(new_session_id);
    }
    Ok(ResumedSession { messages, stats: trajectory.info.model_stats, compactions: trajectory.info.compactions, session, trajectory_path: path.to_path_buf(), repaired })
}

/// Reads the session id out of a default trajectory name such as `1794183406123.traj.json`.
/// @param path - the trajectory file
fn session_id_from_path(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let stem = name.strip_suffix(".traj.json")?;
    (!stem.is_empty() && stem.chars().all(|c| c.is_ascii_digit())).then(|| stem.to_string())
}

/// Gives every tool call that has no result a "not run" result, placed after the results its
/// step does have. The Anthropic API refuses a `tool_use` without a `tool_result`, so a session
/// that stopped during a step could not be sent again without this. Returns how many were added.
/// @param messages - the history, changed in place
pub fn repair_history(messages: &mut Vec<Message>) -> usize {
    let mut added = 0;
    let mut index = 0;
    while index < messages.len() {
        if messages[index].role != Role::Assistant || messages[index].tool_calls.is_empty() {
            index += 1;
            continue;
        }
        let mut end = index + 1;
        while end < messages.len() && messages[end].role == Role::Tool {
            end += 1;
        }
        let answered: Vec<String> = messages[index + 1..end].iter().filter_map(|m| m.tool_call_id.clone()).collect();
        let missing: Vec<String> = messages[index].tool_calls.iter().map(|c| c.id.clone()).filter(|id| !answered.contains(id)).collect();
        for (offset, id) in missing.iter().enumerate() {
            let result = Message::tool(id, NOT_RUN_SESSION_ENDED).with_extra(serde_json::json!({ "repaired": true }));
            messages.insert(end + offset, result);
        }
        added += missing.len();
        index = end + missing.len();
    }
    added
}

/// Cuts a resumed history back to its first `keep` messages, for `--at` with `--fork`, and
/// gives tool calls left without a result one. The system message is always kept.
/// @param resumed - the loaded session, changed in place
/// @param keep - how many messages to keep
pub fn branch_at(resumed: &mut ResumedSession, keep: usize) {
    resumed.messages.truncate(keep.max(1));
    resumed.repaired += repair_history(&mut resumed.messages);
}

/// Lists sessions for `umbra sessions`: this folder's, or every folder's.
/// @param cwd - the folder the person is in
/// @param all - every folder
pub fn listed_sessions(cwd: &Path, all: bool) -> Vec<SessionIndexEntry> {
    if all { list_sessions() } else { sessions_in(cwd) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::message::ToolCall;

    /// An assistant message that asks for the given tool calls.
    fn calls(ids: &[&str]) -> Message {
        let mut message = Message::new(Role::Assistant, "");
        message.tool_calls = ids.iter().map(|id| ToolCall { id: id.to_string(), name: "bash".into(), arguments: serde_json::json!({ "command": "ls" }) }).collect();
        message
    }

    #[test]
    fn gives_unanswered_tool_calls_a_result_after_the_step() {
        let mut messages = vec![Message::system("s"), Message::user("u"), calls(&["a", "b"]), Message::tool("a", "ran"), Message::user("Interrupted by user: stop"), calls(&["c"])];
        assert_eq!(repair_history(&mut messages), 2);
        let shape: Vec<(Role, Option<String>)> = messages.iter().map(|m| (m.role, m.tool_call_id.clone())).collect();
        assert_eq!(shape[3], (Role::Tool, Some("a".into())));
        assert_eq!(shape[4], (Role::Tool, Some("b".into())));
        assert_eq!(shape[5].0, Role::User);
        assert_eq!(shape[7], (Role::Tool, Some("c".into())));
        assert_eq!(messages[4].content, NOT_RUN_SESSION_ENDED);
        assert_eq!(repair_history(&mut messages), 0);
    }

    #[test]
    fn reads_session_ids_from_default_trajectory_names() {
        assert_eq!(session_id_from_path(Path::new("x/1794183406123.traj.json")).as_deref(), Some("1794183406123"));
        assert_eq!(session_id_from_path(Path::new("x/run.traj.json")), None);
    }
}
