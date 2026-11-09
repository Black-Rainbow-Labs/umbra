//! The run record written after every step. The shape follows mini-swe-agent's
//! `mini-swe-agent-1.1` format so its tools can read Umbra trajectories. Umbra adds
//! `info.session`, `info.compactions` and `images` on messages.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::compaction::CompactionRecord;
use super::message::Message;
use super::session::SessionInfo;

/// Running totals for the model calls in one session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelStats {
    pub instance_cost: f64,
    pub api_calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// How many times the history was compacted.
    pub compactions: u32,
}

/// The `info` block of a trajectory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TrajectoryInfo {
    pub model_stats: ModelStats,
    pub config: Value,
    pub umbra_version: String,
    pub exit_status: String,
    pub submission: String,
    /// Every compaction in the session, oldest first.
    pub compactions: Vec<CompactionRecord>,
    /// What the session needs to be found and resumed.
    pub session: SessionInfo,
}

/// A whole trajectory file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    pub info: TrajectoryInfo,
    pub messages: Vec<Message>,
    pub trajectory_format: String,
}
