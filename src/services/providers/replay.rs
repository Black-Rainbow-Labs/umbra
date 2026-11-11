//! A scripted provider that returns replies from a JSON file in order. It makes runs repeatable
//! without a network, which is how Umbra's own tests drive the agent loop.
//!
//! The file is a JSON array. Each entry is either a string (the reply text) or an object
//! `{"content": "...", "tool_calls": [{"name": "bash", "arguments": {"command": "ls"}}]}`, or
//! `{"error": {"status": 400, "body": "prompt is too long"}}` to make that call fail the way a
//! server would. An object may also have `"sleep_ms"`: the call waits that long first, the way a
//! slow model does, and Ctrl+C ends the wait.
//!
//! When `UMBRA_REPLAY_RECORD` names a file, every request is appended to it as one JSON line
//! holding the messages and tool names, so a test can read exactly what the model was sent.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::{Provider, ProviderError, ToolSpec};
use crate::models::config::ProviderConfig;
use crate::models::message::{Message, ToolCall};
use crate::models::model_response::ModelResponse;
use crate::utils::interrupt;

/// Replays scripted replies.
pub struct ReplayProvider {
    replies: Vec<Value>,
    next: usize,
    model: String,
    file: String,
}

impl ReplayProvider {
    /// Loads the script. The file comes from `replay_file` in the config, or from the model name
    /// itself, so `-m replay/path/to/script.json` works with no config.
    /// @param config - the provider settings
    /// @param model - the model name, used as the file path when `replay_file` is empty
    pub fn new(config: &ProviderConfig, model: &str) -> Result<Self> {
        let file = if config.replay_file.is_empty() { model.to_string() } else { config.replay_file.clone() };
        let text = std::fs::read_to_string(&file).with_context(|| format!("reading replay file {file}"))?;
        let replies: Value = serde_json::from_str(&text).with_context(|| format!("parsing replay file {file}"))?;
        let Value::Array(replies) = replies else { bail!("replay file {file} must hold a JSON array") };
        Ok(ReplayProvider { replies, next: 0, model: model.to_string(), file })
    }
}

impl Provider for ReplayProvider {
    /// Returns the next scripted reply, or an error when the script has run out.
    /// @param _messages - ignored
    /// @param _tools - ignored
    fn complete(&mut self, messages: &[Message], tools: &[ToolSpec]) -> Result<ModelResponse, ProviderError> {
        if is_title_request(messages) {
            return self.title_reply();
        }
        record_request(messages, tools);
        while self.replies.get(self.next).is_some_and(|e| e.get("title_reply").is_some()) {
            self.next += 1;
        }
        let Some(entry) = self.replies.get(self.next).cloned() else {
            return Err(ProviderError::fatal(format!("replay file {} has no reply number {}", self.file, self.next + 1)));
        };
        self.next += 1;
        let sleep_ms = entry.get("sleep_ms").and_then(Value::as_u64).unwrap_or(0);
        if sleep_ms > 0 && !interrupt::sleep(Duration::from_millis(sleep_ms)) {
            return Err(ProviderError::interrupted());
        }
        if let Some(error) = entry.get("error") {
            let status = error["status"].as_u64().unwrap_or(500) as u16;
            return Err(ProviderError::from_status(status, error["body"].as_str().unwrap_or("")));
        }
        Ok(scripted_response(&entry, self.next))
    }

    /// The model name.
    fn model_name(&self) -> &str {
        &self.model
    }

    /// The settings recorded in the trajectory.
    fn describe(&self) -> Value {
        json!({ "provider": "replay", "model_name": self.model, "replay_file": self.file })
    }
}

/// True for the request that asks for a session's title, which a script answers with an entry
/// of the form `{"title_reply": "..."}` wherever it is in the file, and which is not recorded.
/// @param messages - the request's messages
fn is_title_request(messages: &[Message]) -> bool {
    messages.first().is_some_and(|m| m.content.starts_with("You name coding sessions"))
}

impl ReplayProvider {
    /// Answers a title request with the script's `title_reply` entry, or fails when it has none.
    fn title_reply(&self) -> Result<ModelResponse, ProviderError> {
        let found = self.replies.iter().find_map(|e| e.get("title_reply").and_then(Value::as_str).map(String::from));
        match found {
            Some(text) => Ok(ModelResponse { content: text, finish_reason: "stop".into(), ..Default::default() }),
            None => Err(ProviderError::fatal("the replay file has no title_reply")),
        }
    }
}

/// Appends one request to the file named by `UMBRA_REPLAY_RECORD`, when it is set. A failure to
/// write is printed and otherwise ignored, since recording only serves tests.
/// @param messages - the messages the agent sent
/// @param tools - the tools it offered
fn record_request(messages: &[Message], tools: &[ToolSpec]) {
    let Ok(path) = std::env::var("UMBRA_REPLAY_RECORD") else { return };
    let sent: Vec<Value> = messages.iter().map(|m| json!({ "role": m.role, "content": m.content, "tool_call_id": m.tool_call_id, "tool_calls": m.tool_calls, "images": m.images.iter().map(|i| json!({ "media_type": i.media_type, "width": i.width, "height": i.height, "bytes": i.data.len() * 3 / 4, "source": i.source })).collect::<Vec<_>>() })).collect();
    let line = json!({ "messages": sent, "tools": tools.iter().map(|t| t.name.clone()).collect::<Vec<_>>() }).to_string();
    let written = std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut file| std::io::Write::write_all(&mut file, format!("{line}
").as_bytes()));
    if let Err(error) = written {
        eprintln!("umbra: could not record the replay request to {path}: {error}");
    }
}

/// Converts one script entry into a response.
/// @param entry - a string or an object with content and tool_calls
/// @param number - the reply number, used to build tool call ids
fn scripted_response(entry: &Value, number: usize) -> ModelResponse {
    if let Value::String(text) = entry {
        return ModelResponse { content: text.clone(), finish_reason: "stop".into(), raw: entry.clone(), ..Default::default() };
    }
    let calls = entry["tool_calls"].as_array().cloned().unwrap_or_default();
    let tool_calls: Vec<ToolCall> = calls
        .iter()
        .enumerate()
        .map(|(i, c)| ToolCall { id: format!("replay_{number}_{i}"), name: c["name"].as_str().unwrap_or("bash").to_string(), arguments: c["arguments"].clone() })
        .collect();
    let finish_reason = if tool_calls.is_empty() { "stop" } else { "tool_calls" };
    ModelResponse { content: entry["content"].as_str().unwrap_or("").to_string(), tool_calls, finish_reason: finish_reason.into(), raw: entry.clone(), ..Default::default() }
}
