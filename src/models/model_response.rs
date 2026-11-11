//! What a provider returns for one model call, already converted out of its wire format.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::message::ToolCall;

/// Token counts reported by the provider for one call.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    /// The part of `output_tokens` the model spent reasoning, when the server reports it.
    #[serde(default)]
    pub reasoning_tokens: u64,
}

/// One model reply.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: String,
    pub usage: Usage,
    pub cost: f64,
    /// The provider's raw JSON response, kept for the trajectory file.
    pub raw: Value,
    /// How long the call took: `total_ms`, `first_token_ms` (streamed replies only: the first
    /// content, reasoning or tool call text) and the server's own `timings` when it sends them.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub timing: Value,
}
