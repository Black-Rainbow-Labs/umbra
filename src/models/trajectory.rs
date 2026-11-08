//! The run record written after every step. The shape follows mini-swe-agent's
//! `mini-swe-agent-1.1` format so its tools can read Umbra trajectories.

use serde::Serialize;
use serde_json::Value;

use super::message::Message;

/// Running totals for the model calls in one run.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ModelStats {
    pub instance_cost: f64,
    pub api_calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// The `info` block of a trajectory.
#[derive(Debug, Clone, Serialize)]
pub struct TrajectoryInfo {
    pub model_stats: ModelStats,
    pub config: Value,
    pub umbra_version: String,
    pub exit_status: String,
    pub submission: String,
}

/// A whole trajectory file.
#[derive(Debug, Clone, Serialize)]
pub struct Trajectory {
    pub info: TrajectoryInfo,
    pub messages: Vec<Message>,
    pub trajectory_format: String,
}
