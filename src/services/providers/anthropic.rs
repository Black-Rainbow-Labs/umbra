//! The Anthropic Messages API (`POST /v1/messages`), called over plain HTTP because Rust has no
//! official Anthropic SDK.

use anyhow::Result;
use serde_json::{json, Map, Value};

use super::credentials::{anthropic_credential, Credential};
use super::pricing::{cost_of, price_for};
use super::{apply_extra_body, retry_count, with_retries, Provider, ProviderError, ToolSpec};
use crate::models::config::ProviderConfig;
use crate::models::message::{Message, Role, ToolCall};
use crate::models::model_response::{ModelResponse, Usage};
use crate::models::image::ImageData;
use crate::utils::http::{http_agent, post_json_interruptible};
use crate::utils::tokens::known_context_window;

/// The system text a Claude Code login must send first. Requests made with that login are
/// refused (HTTP 429) without it.
const CLAUDE_CODE_IDENTITY: &str = "You are Claude Code, Anthropic's official CLI for Claude.";

/// The key in `Message.extra` that holds the assistant's raw content blocks. They are sent back
/// exactly as received, because current Claude models reject edited thinking blocks.
pub const RAW_CONTENT_KEY: &str = "anthropic_content";

/// A Claude model.
pub struct AnthropicProvider {
    config: ProviderConfig,
    model: String,
    credential: Credential,
    agent: ureq::Agent,
}

impl AnthropicProvider {
    /// Builds the provider and resolves its credential.
    /// @param config - the provider settings
    /// @param model - the model id, such as claude-haiku-5-5
    pub fn new(config: ProviderConfig, model: &str) -> Result<Self> {
        let credential = anthropic_credential(&config)?;
        Ok(AnthropicProvider { config, model: model.to_string(), credential, agent: http_agent(600) })
    }

    /// Builds the request body.
    /// @param messages - the conversation
    /// @param tools - the tools offered to the model
    fn request_body(&self, messages: &[Message], tools: &[ToolSpec]) -> Value {
        let mut body = json!({
            "model": self.model,
            "max_tokens": self.config.max_tokens.unwrap_or(16000),
            "system": self.system_blocks(messages),
            "messages": convert_messages(messages),
            "cache_control": { "type": "ephemeral" },
        });
        if !tools.is_empty() {
            body["tools"] = Value::Array(tools.iter().map(|t| json!({ "name": t.name, "description": t.description, "input_schema": t.input_schema })).collect());
        }
        if let Some(temperature) = self.config.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(effort) = &self.config.effort {
            body["output_config"] = json!({ "effort": effort });
        }
        apply_extra_body(&mut body, &self.config);
        body
    }

    /// Collects the system messages into system blocks, with the Claude Code identity first when
    /// the Claude Code login is in use.
    /// @param messages - the conversation
    fn system_blocks(&self, messages: &[Message]) -> Value {
        let mut blocks = Vec::new();
        if matches!(self.credential, Credential::OAuthToken(_)) {
            blocks.push(json!({ "type": "text", "text": CLAUDE_CODE_IDENTITY }));
        }
        for message in messages.iter().filter(|m| m.role == Role::System) {
            blocks.push(json!({ "type": "text", "text": message.content }));
        }
        Value::Array(blocks)
    }

    /// Builds the request headers for the credential in use.
    fn headers(&self) -> Vec<(String, String)> {
        let mut headers = vec![("anthropic-version".to_string(), "2023-06-01".to_string())];
        match &self.credential {
            Credential::ApiKey(key) => headers.push(("x-api-key".into(), key.clone())),
            Credential::OAuthToken(token) => {
                headers.push(("authorization".into(), format!("Bearer {token}")));
                headers.push(("anthropic-beta".into(), "oauth-2025-04-20".into()));
            }
        }
        headers.extend(self.config.headers.iter().map(|(k, v)| (k.clone(), v.clone())));
        headers
    }

    /// Sends one request and parses the reply. The request runs on a worker thread so Ctrl+C
    /// stops the wait at once; the server still finishes the abandoned reply.
    /// @param body - the request body
    fn send(&self, body: &Value) -> Result<ModelResponse, ProviderError> {
        let url = format!("{}/v1/messages", self.config.base_url.trim_end_matches('/'));
        let reply = post_json_interruptible(&self.agent, &url, &self.headers(), body).map_err(ProviderError::from_request)?;
        if reply.status != 200 {
            return Err(ProviderError::from_status(reply.status, &reply.body));
        }
        let raw: Value = serde_json::from_str(&reply.body).map_err(|e| ProviderError::fatal(format!("bad JSON from Anthropic: {e}")))?;
        let mut response = parse_response(&raw);
        response.cost = cost_of(&response.usage, price_for(&self.model, &self.config));
        Ok(response)
    }
}

impl Provider for AnthropicProvider {
    /// Sends the conversation, retrying rate limits and server errors.
    /// @param messages - the conversation
    /// @param tools - the tools offered to the model
    fn complete(&mut self, messages: &[Message], tools: &[ToolSpec]) -> Result<ModelResponse, ProviderError> {
        let body = self.request_body(messages, tools);
        with_retries(retry_count(&self.config), || self.send(&body))
    }

    /// The model id.
    fn model_name(&self) -> &str {
        &self.model
    }

    /// The settings recorded in the trajectory.
    fn describe(&self) -> Value {
        json!({ "provider": "anthropic", "model_name": self.model, "base_url": self.config.base_url, "auth": self.credential.kind(), "max_tokens": self.config.max_tokens.unwrap_or(16000) })
    }

    /// The window from the built in table: one million tokens for a `[1m]` model id, 200,000
    /// for other Claude models.
    fn context_window(&mut self) -> Option<u64> {
        known_context_window(&self.model)
    }
}

/// Converts the history into Anthropic messages. Tool results become `tool_result` blocks in a
/// user message, and messages with the same role in a row are joined into one.
/// @param messages - the conversation
pub fn convert_messages(messages: &[Message]) -> Value {
    let mut out: Vec<Value> = Vec::new();
    for message in messages {
        let Some((role, blocks)) = convert_message(message) else { continue };
        match out.last_mut() {
            Some(last) if last["role"] == role => last["content"].as_array_mut().expect("content is an array").extend(blocks),
            _ => out.push(json!({ "role": role, "content": blocks })),
        }
    }
    Value::Array(out)
}

/// Converts one message into a role and its content blocks. System and exit messages return None.
/// @param message - one history entry
fn convert_message(message: &Message) -> Option<(&'static str, Vec<Value>)> {
    match message.role {
        Role::System | Role::Exit => None,
        Role::User => Some(("user", user_blocks(message))),
        Role::Tool => Some(("user", vec![tool_result_block(message)])),
        Role::Assistant => Some(("assistant", assistant_blocks(message))),
    }
}

/// Returns a user message's blocks: its images first, as Anthropic recommends, then its text.
/// @param message - a user message
fn user_blocks(message: &Message) -> Vec<Value> {
    let mut blocks: Vec<Value> = message.images.iter().map(image_block).collect();
    blocks.push(text_block(&message.content));
    blocks
}

/// Builds a tool result. With images its content is a list of the text and image blocks.
/// @param message - a tool message
fn tool_result_block(message: &Message) -> Value {
    let id = message.tool_call_id.clone().unwrap_or_default();
    if message.images.is_empty() {
        return json!({ "type": "tool_result", "tool_use_id": id, "content": message.content });
    }
    let mut content = vec![text_block(&message.content)];
    content.extend(message.images.iter().map(image_block));
    json!({ "type": "tool_result", "tool_use_id": id, "content": content })
}

/// Builds a base64 image block.
/// @param image - the image
fn image_block(image: &ImageData) -> Value {
    json!({ "type": "image", "source": { "type": "base64", "media_type": image.media_type, "data": image.data } })
}

/// Returns the assistant's content blocks: the raw blocks from the API when present, otherwise
/// text and tool use blocks built from the message.
/// @param message - an assistant message
fn assistant_blocks(message: &Message) -> Vec<Value> {
    if let Some(Value::Array(raw)) = message.extra.get(RAW_CONTENT_KEY) {
        if !raw.is_empty() {
            return raw.clone();
        }
    }
    let mut blocks = Vec::new();
    if !message.content.trim().is_empty() {
        blocks.push(text_block(&message.content));
    }
    for call in &message.tool_calls {
        blocks.push(json!({ "type": "tool_use", "id": call.id, "name": call.name, "input": call.arguments }));
    }
    if blocks.is_empty() {
        blocks.push(text_block("(no output)"));
    }
    blocks
}

/// Builds a text block, replacing empty text, which the API rejects.
/// @param text - the block text
fn text_block(text: &str) -> Value {
    let text = if text.is_empty() { "(empty)" } else { text };
    json!({ "type": "text", "text": text })
}

/// Reads text, tool calls, the stop reason and token counts out of a Messages API reply.
/// @param raw - the response JSON
pub fn parse_response(raw: &Value) -> ModelResponse {
    let blocks = raw["content"].as_array().cloned().unwrap_or_default();
    let mut text = Vec::new();
    let mut tool_calls = Vec::new();
    for block in &blocks {
        match block["type"].as_str() {
            Some("text") => text.push(block["text"].as_str().unwrap_or("").to_string()),
            Some("tool_use") => tool_calls.push(ToolCall { id: block["id"].as_str().unwrap_or("").into(), name: block["name"].as_str().unwrap_or("").into(), arguments: block["input"].clone() }),
            _ => {}
        }
    }
    let usage = &raw["usage"];
    let usage = Usage {
        input_tokens: usage["input_tokens"].as_u64().unwrap_or(0),
        output_tokens: usage["output_tokens"].as_u64().unwrap_or(0),
        cache_read_tokens: usage["cache_read_input_tokens"].as_u64().unwrap_or(0),
        cache_write_tokens: usage["cache_creation_input_tokens"].as_u64().unwrap_or(0),
    };
    let finish_reason = match raw["stop_reason"].as_str().unwrap_or("") {
        "max_tokens" => "length".to_string(),
        "tool_use" => "tool_calls".to_string(),
        other => other.to_string(),
    };
    let mut response = ModelResponse { content: text.join("\n"), tool_calls, finish_reason, usage, cost: 0.0, raw: raw.clone() };
    response.raw = attach_raw_content(raw, Value::Array(blocks));
    response
}

/// Keeps the whole reply for the trajectory and marks the content blocks for replay.
/// @param raw - the response JSON
/// @param blocks - its content blocks
fn attach_raw_content(raw: &Value, blocks: Value) -> Value {
    let mut map = raw.as_object().cloned().unwrap_or_else(Map::new);
    map.insert(RAW_CONTENT_KEY.into(), blocks);
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_tool_results_into_one_user_message() {
        let mut assistant = Message::new(Role::Assistant, "thinking");
        assistant.tool_calls = vec![ToolCall { id: "a".into(), name: "bash".into(), arguments: json!({"command": "ls"}) }, ToolCall { id: "b".into(), name: "bash".into(), arguments: json!({"command": "pwd"}) }];
        let messages = vec![Message::system("sys"), Message::user("task"), assistant, Message::tool("a", "out1"), Message::tool("b", "out2")];
        let converted = convert_messages(&messages);
        assert_eq!(converted.as_array().unwrap().len(), 3);
        assert_eq!(converted[2]["content"].as_array().unwrap().len(), 2);
        assert_eq!(converted[1]["content"][1]["type"], "tool_use");
    }

    #[test]
    fn sends_images_in_user_messages_and_tool_results() {
        let image = ImageData { media_type: "image/jpeg".into(), data: "QUJD".into(), width: 2, height: 2, source: "a.jpg".into() };
        let mut assistant = Message::new(Role::Assistant, "");
        assistant.tool_calls = vec![ToolCall { id: "t".into(), name: "shot".into(), arguments: json!({}) }];
        let messages = vec![Message::user("what is this").with_images(vec![image.clone()]), assistant, Message::tool("t", "screenshot").with_images(vec![image])];
        let sent = convert_messages(&messages);
        assert_eq!(sent[0]["content"][0]["type"], "image");
        assert_eq!(sent[0]["content"][0]["source"]["media_type"], "image/jpeg");
        assert_eq!(sent[0]["content"][1]["text"], "what is this");
        let result = &sent[2]["content"][0];
        assert_eq!(result["type"], "tool_result");
        assert_eq!(result["content"][0]["text"], "screenshot");
        assert_eq!(result["content"][1]["source"]["data"], "QUJD");
    }

    #[test]
    fn parses_tool_use_and_maps_stop_reasons() {
        let raw = json!({"content": [{"type": "text", "text": "ok"}, {"type": "tool_use", "id": "t1", "name": "bash", "input": {"command": "ls"}}], "stop_reason": "tool_use", "usage": {"input_tokens": 10, "output_tokens": 5}});
        let parsed = parse_response(&raw);
        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.finish_reason, "tool_calls");
        assert_eq!(parsed.usage.input_tokens, 10);
    }
}
