//! Any server that speaks the OpenAI Chat Completions API (`POST /chat/completions`): OpenAI,
//! OpenRouter, llama.cpp's `llama-server`, Ollama, vLLM and others.

use serde_json::{json, Value};

use super::credentials::api_key;
use super::pricing::{cost_of, price_for};
use super::{apply_extra_body, is_overflow_text, retry_count, with_retries, Provider, ProviderError, ToolSpec};
use crate::models::config::ProviderConfig;
use crate::models::message::{Message, Role, ToolCall};
use crate::models::model_response::{ModelResponse, Usage};
use crate::utils::http::{get_text, http_agent, post_json};
use crate::utils::tokens::known_context_window;

/// A model behind an OpenAI compatible endpoint.
pub struct OpenAiProvider {
    config: ProviderConfig,
    model: String,
    agent: ureq::Agent,
}

impl OpenAiProvider {
    /// Builds the provider.
    /// @param config - the provider settings, including `base_url`
    /// @param model - the model id the server expects
    pub fn new(config: ProviderConfig, model: &str) -> Self {
        OpenAiProvider { config, model: model.to_string(), agent: http_agent(1800) }
    }

    /// Builds the request body.
    /// @param messages - the conversation
    /// @param tools - the tools offered to the model
    fn request_body(&self, messages: &[Message], tools: &[ToolSpec]) -> Value {
        let mut body = json!({ "model": self.model, "messages": convert_messages(messages) });
        if !tools.is_empty() {
            body["tools"] = Value::Array(tools.iter().map(|t| json!({ "type": "function", "function": { "name": t.name, "description": t.description, "parameters": t.input_schema } })).collect());
        }
        if let Some(max_tokens) = self.config.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(temperature) = self.config.temperature {
            body["temperature"] = json!(temperature);
        }
        apply_extra_body(&mut body, &self.config);
        body
    }

    /// Builds the request headers.
    fn headers(&self) -> Vec<(String, String)> {
        let mut headers: Vec<(String, String)> = Vec::new();
        if let Some(key) = api_key(&self.config) {
            headers.push(("authorization".into(), format!("Bearer {key}")));
        }
        headers.extend(self.config.headers.iter().map(|(k, v)| (k.clone(), v.clone())));
        headers
    }

    /// Sends one request and parses the reply.
    /// @param body - the request body
    fn send(&self, body: &Value) -> Result<ModelResponse, ProviderError> {
        let url = format!("{}/chat/completions", self.config.base_url.trim_end_matches('/'));
        let reply = post_json(&self.agent, &url, &self.headers(), body).map_err(ProviderError::network)?;
        if reply.status != 200 {
            return Err(ProviderError::from_status(reply.status, &reply.body));
        }
        let raw: Value = serde_json::from_str(&reply.body).map_err(|e| ProviderError::fatal(format!("bad JSON from {url}: {e}")))?;
        if let Some(error) = raw.get("error") {
            if is_overflow_text(&error.to_string()) {
                return Err(ProviderError { message: format!("server error: {error}"), retryable: false, context_overflow: true });
            }
            return Err(ProviderError::network(format!("server error: {error}")));
        }
        let mut response = parse_response(&raw);
        response.cost = cost_of(&response.usage, price_for(&self.model, &self.config));
        Ok(response)
    }
}

impl Provider for OpenAiProvider {
    /// Sends the conversation, retrying rate limits, server errors and network failures.
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
        json!({ "provider": "openai", "model_name": self.model, "base_url": self.config.base_url })
    }

    /// The window from the built in table for known models. Otherwise the server is asked: a
    /// llama.cpp or NInfer server on this machine reports `n_ctx` at `/props`, and vLLM,
    /// llama.cpp and OpenRouter list a size for each model at `/models`.
    fn context_window(&mut self) -> Option<u64> {
        if let Some(window) = known_context_window(&self.model) {
            return Some(window);
        }
        let base = self.config.base_url.trim_end_matches('/');
        let agent = http_agent(10);
        let headers = self.headers();
        if is_local_url(base) {
            let props_url = format!("{}/props", base.trim_end_matches("/v1"));
            if let Some(window) = fetch_json(&agent, &props_url, &headers).as_ref().and_then(window_from_props) {
                return Some(window);
            }
        }
        fetch_json(&agent, &format!("{base}/models"), &headers).and_then(|models| window_from_models(&models, &self.model))
    }
}

/// Returns true for a server on this machine.
/// @param url - the base URL
fn is_local_url(url: &str) -> bool {
    ["://localhost", "://127.0.0.1", "://[::1]", "://0.0.0.0"].iter().any(|host| url.contains(host))
}

/// GETs a URL and parses the reply as JSON. Any failure gives None.
/// @param agent - the HTTP agent
/// @param url - the endpoint
/// @param headers - request headers
fn fetch_json(agent: &ureq::Agent, url: &str, headers: &[(String, String)]) -> Option<Value> {
    let reply = get_text(agent, url, headers).ok()?;
    if reply.status != 200 {
        return None;
    }
    serde_json::from_str(&reply.body).ok()
}

/// Reads the context size from a llama.cpp `/props` reply.
/// @param props - the reply JSON
fn window_from_props(props: &Value) -> Option<u64> {
    props["n_ctx"].as_u64().or_else(|| props["default_generation_settings"]["n_ctx"].as_u64()).filter(|n| *n > 0)
}

/// Finds the model in a `/models` reply and reads its context size. A server that lists one
/// model is taken to be serving it, whatever name the run used.
/// @param models - the reply JSON
/// @param model - the model id the run sends
fn window_from_models(models: &Value, model: &str) -> Option<u64> {
    let list = models["data"].as_array()?;
    let entry = list.iter().find(|m| m["id"] == model).or(if list.len() == 1 { list.first() } else { None })?;
    ["max_model_len", "context_length", "context_window"].iter().find_map(|key| entry[*key].as_u64()).or_else(|| entry["meta"]["n_ctx_train"].as_u64()).filter(|n| *n > 0)
}

/// Converts the history into Chat Completions messages.
/// @param messages - the conversation
pub fn convert_messages(messages: &[Message]) -> Value {
    let converted = messages.iter().filter(|m| m.role != Role::Exit).map(convert_message).collect();
    Value::Array(converted)
}

/// Converts one message. Tool call arguments are sent as JSON text, as the API requires.
/// @param message - one history entry
fn convert_message(message: &Message) -> Value {
    match message.role {
        Role::System => json!({ "role": "system", "content": message.content }),
        Role::User | Role::Exit => json!({ "role": "user", "content": message.content }),
        Role::Tool => json!({ "role": "tool", "tool_call_id": message.tool_call_id.clone().unwrap_or_default(), "content": message.content }),
        Role::Assistant => {
            let mut out = json!({ "role": "assistant", "content": message.content });
            if !message.tool_calls.is_empty() {
                out["tool_calls"] = Value::Array(message.tool_calls.iter().map(tool_call_json).collect());
            }
            out
        }
    }
}

/// Encodes one tool call in the Chat Completions shape.
/// @param call - the tool call
fn tool_call_json(call: &ToolCall) -> Value {
    let arguments = match &call.arguments {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    json!({ "id": call.id, "type": "function", "function": { "name": call.name, "arguments": arguments } })
}

/// Reads the reply. Tool call arguments that are not valid JSON are kept as a string, so the
/// action parser can tell the model what was wrong.
/// @param raw - the response JSON
pub fn parse_response(raw: &Value) -> ModelResponse {
    let choice = &raw["choices"][0];
    let message = &choice["message"];
    let tool_calls = message["tool_calls"].as_array().map(|calls| calls.iter().enumerate().map(|(i, c)| parse_tool_call(i, c)).collect()).unwrap_or_default();
    let usage = &raw["usage"];
    let cached = usage["prompt_tokens_details"]["cached_tokens"].as_u64().unwrap_or(0);
    let usage = Usage {
        input_tokens: usage["prompt_tokens"].as_u64().unwrap_or(0).saturating_sub(cached),
        output_tokens: usage["completion_tokens"].as_u64().unwrap_or(0),
        cache_read_tokens: cached,
        cache_write_tokens: 0,
    };
    ModelResponse { content: message["content"].as_str().unwrap_or("").to_string(), tool_calls, finish_reason: choice["finish_reason"].as_str().unwrap_or("").to_string(), usage, cost: 0.0, raw: raw.clone() }
}

/// Reads one tool call, giving it an id when the server sent none.
/// @param index - the call's position, used for a missing id
/// @param call - the tool call JSON
fn parse_tool_call(index: usize, call: &Value) -> ToolCall {
    let text = call["function"]["arguments"].as_str().unwrap_or("{}");
    let arguments = serde_json::from_str::<Value>(text).unwrap_or_else(|_| Value::String(text.to_string()));
    let id = call["id"].as_str().filter(|s| !s.is_empty()).map(String::from).unwrap_or_else(|| format!("call_{index}"));
    ToolCall { id, name: call["function"]["name"].as_str().unwrap_or("").to_string(), arguments }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_context_sizes_from_server_replies() {
        assert_eq!(window_from_props(&json!({"default_generation_settings": {"n_ctx": 32768}})), Some(32768));
        let models = json!({"data": [{"id": "a", "max_model_len": 1000}, {"id": "b", "context_length": 2000}]});
        assert_eq!(window_from_models(&models, "b"), Some(2000));
        assert_eq!(window_from_models(&models, "c"), None);
        assert_eq!(window_from_models(&json!({"data": [{"id": "x", "meta": {"n_ctx_train": 4096}}]}), "qwen"), Some(4096));
    }

    #[test]
    fn round_trips_tool_calls() {
        let raw = json!({"choices": [{"message": {"content": null, "tool_calls": [{"id": "x", "type": "function", "function": {"name": "bash", "arguments": "{\"command\":\"ls\"}"}}]}, "finish_reason": "tool_calls"}], "usage": {"prompt_tokens": 7, "completion_tokens": 3}});
        let parsed = parse_response(&raw);
        assert_eq!(parsed.tool_calls[0].arguments["command"], "ls");
        let mut assistant = Message::new(Role::Assistant, "");
        assistant.tool_calls = parsed.tool_calls.clone();
        let sent = convert_messages(&[assistant]);
        assert_eq!(sent[0]["tool_calls"][0]["function"]["arguments"], "{\"command\":\"ls\"}");
    }
}
