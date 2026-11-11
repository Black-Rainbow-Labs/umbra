//! Sessions: what a trajectory records about itself so it can be found and resumed, and the small
//! index entry kept for each one under `~/.umbra/sessions`.

use serde::{Deserialize, Serialize};

/// The `info.session` block of a trajectory.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionInfo {
    /// The start time in milliseconds, also the default trajectory file name.
    pub id: String,
    /// The working folder.
    pub cwd: String,
    /// The `provider/model` string the session last ran with.
    pub model: String,
    /// The first task, shortened, for listings.
    pub task: String,
    /// Seconds since the Unix epoch.
    pub started_at: f64,
    /// Each time the session was resumed, oldest first.
    pub resumes: Vec<ResumeRecord>,
    /// The session's name, kept here too so a trajectory alone can be listed.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub summary: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mcp_off: Vec<String>,
    pub work_seconds: f64,
}

/// One resume of a session.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResumeRecord {
    /// Seconds since the Unix epoch.
    pub at: f64,
    /// The `provider/model` string used from this resume on.
    pub model: String,
    /// The history length when it was resumed.
    pub messages_before: usize,
    /// With `--fork`: the session the history was copied from.
    pub forked_from: String,
}

/// The index entry `~/.umbra/sessions/<id>.json`, rewritten after every step. It lets
/// `umbra sessions` list sessions without reading trajectories, which can be large.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionIndexEntry {
    pub id: String,
    /// The trajectory file.
    pub trajectory: String,
    pub cwd: String,
    pub model: String,
    pub task: String,
    pub started_at: f64,
    pub updated_at: f64,
    /// Empty while the session is running or after a crash.
    pub exit_status: String,
    pub api_calls: u32,
    pub cost: f64,
    pub messages: usize,
    /// A short name: the first line of the first prompt, then a title the model wrote.
    pub title: String,
    /// One sentence about the session, from the model or from the last compaction summary.
    pub summary: String,
    /// The MCP servers turned off for this session, which a resume turns off again.
    pub mcp_off: Vec<String>,
    /// The agent time so far, in seconds.
    pub work_seconds: f64,
}
