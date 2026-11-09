//! What the model asked Umbra to do, and what came back.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::image::ImageData;

/// One thing to run. A model response can hold several in tool mode and exactly one in text mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Action {
    /// A shell command run in the environment.
    Bash { command: String, #[serde(default, skip_serializing_if = "Option::is_none")] tool_call_id: Option<String> },
    /// A call to a tool on an MCP server.
    Mcp { server: String, tool: String, arguments: Value, #[serde(default, skip_serializing_if = "Option::is_none")] tool_call_id: Option<String> },
}

impl Action {
    /// Returns the id of the tool call that produced this action, if it came from a tool call.
    pub fn tool_call_id(&self) -> Option<&str> {
        match self {
            Action::Bash { tool_call_id, .. } | Action::Mcp { tool_call_id, .. } => tool_call_id.as_deref(),
        }
    }

    /// Returns a one line description used for confirmation prompts and the whitelist.
    pub fn describe(&self) -> String {
        match self {
            Action::Bash { command, .. } => command.clone(),
            Action::Mcp { server, tool, arguments, .. } => format!("mcp {server}.{tool} {arguments}"),
        }
    }
}

/// The result of running one action. Mirrors mini-swe-agent's `{output, returncode, exception_info}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub output: String,
    pub returncode: i32,
    pub exception_info: String,
    /// Images the action returned. Only MCP tools return them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageData>,
}

impl Observation {
    /// Builds an observation for an action that failed before or while running.
    /// @param output - any partial output that was captured
    /// @param error - the error description shown to the model
    pub fn failed(output: impl Into<String>, error: impl Into<String>) -> Self {
        Observation { output: output.into(), returncode: -1, exception_info: error.into(), images: Vec::new() }
    }
}
