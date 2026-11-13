//! Any server that speaks the OpenAI Chat Completions API (`POST /chat/completions`): OpenAI,
//! OpenRouter, llama.cpp's `llama-server`, Ollama, vLLM and others.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::credentials::api_key;
use super::pricing::{cost_of, price_for};
use super::{apply_extra_body, is_overflow_text, retry_count, with_retries, Provider, ProviderError, ToolSpec};
use crate::models::config::ProviderConfig;
use crate::models::message::{Message, Role, ToolCall};
use crate::models::model_response::{ModelResponse, Usage};
use crate::models::image::ImageData;
use crate::utils::http::{get_text, http_agent, post_json_interruptible, post_stream, StreamEvent};
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
        let mut body = json!({ "model": self.model, "messages": convert_messages(messages, self.config.send_reasoning.unwrap_or(true)) });
        if self.streaming() {
            body["stream"] = json!(true);
            body["stream_options"] = json!({ "include_usage": true });
        }
        if !tools.is_empty() {
            body["tools"] = Value::Array(tools.iter().map(|t| json!({ "type": "function", "function": { "name": t.name, "description": t.description, "parameters": t.input_schema } })).collect());
        }
        if let Some(max_tokens) = self.config.max_tokens.or_else(|| is_local_url(&self.config.base_url).then_some(LOCAL_MAX_TOKENS)) {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(temperature) = self.config.temperature {
            body["temperature"] = json!(temperature);
        }
        if self.config.preserve_thinking.unwrap_or_else(|| is_local_url(&self.config.base_url)) {
            body["chat_template_kwargs"] = json!({ "preserve_thinking": true });
        }
        if let Some(enable) = thinking_for(self.config.thinking.as_deref(), messages) {
            if !body["chat_template_kwargs"].is_object() {
                body["chat_template_kwargs"] = json!({});
            }
            body["chat_template_kwargs"]["enable_thinking"] = json!(enable);
        }
        apply_extra_body(&mut body, &self.config);
        body
    }

    /// Returns true when replies are streamed, which is the default.
    fn streaming(&self) -> bool {
        self.config.stream.unwrap_or(true)
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

    /// Sends one request and parses the reply. Both ways of sending give up as soon as Ctrl+C
    /// is pressed.
    /// @param body - the request body
    fn send(&self, body: &Value) -> Result<ModelResponse, ProviderError> {
        let url = format!("{}/chat/completions", self.config.base_url.trim_end_matches('/'));
        let started = Instant::now();
        let (raw, first_token) = if self.streaming() { self.send_streaming(&url, body, started)? } else { (self.send_whole(&url, body)?, None) };
        if let Some(error) = raw.get("error").filter(|e| !e.is_null()) {
            if is_overflow_text(&error.to_string()) {
                return Err(ProviderError { message: format!("server error: {error}"), retryable: false, context_overflow: true, interrupted: false });
            }
            return Err(ProviderError::network(format!("server error: {error}")));
        }
        let mut response = parse_response(&raw);
        response.cost = cost_of(&response.usage, price_for(&self.model, &self.config));
        response.timing = call_timing(started.elapsed(), first_token);
        Ok(response)
    }
}

impl OpenAiProvider {
    /// Sends a request without streaming and returns the reply JSON.
    /// @param url - the endpoint
    /// @param body - the request body
    fn send_whole(&self, url: &str, body: &Value) -> Result<Value, ProviderError> {
        let reply = post_json_interruptible(&self.agent, url, &self.headers(), body).map_err(ProviderError::from_request)?;
        if reply.status != 200 {
            return Err(ProviderError::from_status(reply.status, &reply.body));
        }
        serde_json::from_str(&reply.body).map_err(|e| ProviderError::fatal(format!("bad JSON from {url}: {e}")))
    }

    /// Sends a streaming request and joins the chunks into the JSON a whole reply would have had.
    /// Also returns how long after `started` the first content, reasoning or tool call text came.
    /// On Ctrl+C the stream is dropped, which closes the connection at the next chunk.
    /// @param url - the endpoint
    /// @param body - the request body
    /// @param started - when the call began
    fn send_streaming(&self, url: &str, body: &Value, started: Instant) -> Result<(Value, Option<Duration>), ProviderError> {
        let stream = post_stream(&self.agent, url, &self.headers(), body);
        let mut status = 0;
        let mut assembler = StreamAssembler { started: Some(started), ..Default::default() };
        loop {
            match stream.next().map_err(ProviderError::from_request)? {
                StreamEvent::Start(code) => status = code,
                StreamEvent::ErrorBody(text) => return Err(ProviderError::from_status(status, &text)),
                StreamEvent::Line(line) => assembler.push_line(&line),
                StreamEvent::End => break,
                StreamEvent::Failed(message) => return Err(ProviderError::network(format!("the stream from {url} broke: {message}"))),
            }
        }
        let first_token = assembler.first_token;
        let raw = assembler.finish().map_err(|e| ProviderError::fatal(format!("bad reply from {url}: {e}")))?;
        Ok((raw, first_token))
    }
}

/// Builds the timing record of one call, kept in the trajectory so a run's speed can be read
/// without a proxy. The server's own `timings` object, which shows its prompt cache, stays in
/// the raw reply (`extra.response.timings`) and is not copied here, because the copy added
/// about 300 bytes to every step of the session journal and the trajectory.
/// @param total - how long the whole call took
/// @param first_token - how long until the first content, reasoning or tool call text, if streamed
fn call_timing(total: Duration, first_token: Option<Duration>) -> Value {
    let mut timing = json!({ "total_ms": total.as_millis() as u64 });
    if let Some(first) = first_token {
        timing["first_token_ms"] = json!(first.as_millis() as u64);
    }
    timing
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
        if let Some(window) = self.config.context_window.filter(|w| *w > 0) {
            return Some(window);
        }
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

/// Decides `enable_thinking` for one request from the `thinking` setting: `on`, `off`, or
/// `first`, which thinks only when the newest message is the person's (the first request of a
/// turn). None sends nothing.
/// @param setting - the provider's `thinking` setting
/// @param messages - the conversation
fn thinking_for(setting: Option<&str>, messages: &[Message]) -> Option<bool> {
    match setting? {
        "on" => Some(true),
        "off" => Some(false),
        "first" => Some(messages.iter().rev().find(|m| m.role != Role::Exit).is_some_and(|m| m.role == Role::User && m.tool_call_id.is_none())),
        _ => None,
    }
}

/// The reply length limit sent to a server on this machine when `max_tokens` is not set. Without
/// it the server's own default applies (8192 on llama.cpp), and a thinking model can spend all of
/// it on reasoning, so the reply is cut off with no text or tool call in it.
const LOCAL_MAX_TOKENS: u32 = 16000;

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

/// The text that introduces images returned by tools. A `tool` message can only hold text, so
/// the images go in a user message after the step's last tool result.
pub const TOOL_IMAGES_TEXT: &str = "Images returned by the tools above:";

/// Converts the history into Chat Completions messages.
/// @param messages - the conversation
/// @param send_reasoning - send each assistant reply's reasoning text back to the server
pub fn convert_messages(messages: &[Message], send_reasoning: bool) -> Value {
    let mut converted = Vec::new();
    let mut tool_images: Vec<&ImageData> = Vec::new();
    for message in messages.iter().filter(|m| m.role != Role::Exit) {
        if message.role != Role::Tool && !tool_images.is_empty() {
            converted.push(user_with_images(TOOL_IMAGES_TEXT, &std::mem::take(&mut tool_images)));
        }
        if message.role == Role::Tool {
            tool_images.extend(message.images.iter());
        }
        converted.push(convert_message(message, send_reasoning));
    }
    if !tool_images.is_empty() {
        converted.push(user_with_images(TOOL_IMAGES_TEXT, &tool_images));
    }
    Value::Array(converted)
}

/// Builds a user message whose content is the images, then the text.
/// @param text - the text part
/// @param images - the images
fn user_with_images(text: &str, images: &[&ImageData]) -> Value {
    let mut parts: Vec<Value> = images.iter().map(|image| json!({ "type": "image_url", "image_url": { "url": image.data_url() } })).collect();
    parts.push(json!({ "type": "text", "text": text }));
    json!({ "role": "user", "content": parts })
}

/// Converts one message. Tool call arguments are sent as JSON text, as the API requires. An
/// assistant reply gets back the reasoning text the server streamed for it, byte for byte and
/// under the field name the server used, so the server renders the turn exactly as it generated
/// it and its prompt cache matches past that turn.
/// @param message - one history entry
/// @param send_reasoning - send the reply's reasoning text back
fn convert_message(message: &Message, send_reasoning: bool) -> Value {
    match message.role {
        Role::System => json!({ "role": "system", "content": message.content }),
        Role::User | Role::Exit if !message.images.is_empty() => user_with_images(&message.content, &message.images.iter().collect::<Vec<_>>()),
        Role::User | Role::Exit => json!({ "role": "user", "content": message.content }),
        Role::Tool => json!({ "role": "tool", "tool_call_id": message.tool_call_id.clone().unwrap_or_default(), "content": message.content }),
        Role::Assistant => {
            let mut out = json!({ "role": "assistant", "content": message.content });
            if let Some((key, text)) = message.reasoning().filter(|_| send_reasoning) {
                out[key] = json!(text);
            }
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
        reasoning_tokens: usage["completion_tokens_details"]["reasoning_tokens"].as_u64().unwrap_or(0),
    };
    ModelResponse { content: message["content"].as_str().unwrap_or("").to_string(), tool_calls, finish_reason: choice["finish_reason"].as_str().unwrap_or("").to_string(), usage, cost: 0.0, raw: raw.clone(), timing: Value::Null }
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

/// One tool call being put together from stream chunks.
#[derive(Default)]
struct PartialCall {
    id: String,
    name: String,
    arguments: String,
}

/// Joins the chunks of a streamed Chat Completions reply into the JSON of a whole reply, so the
/// same `parse_response` reads both. A server that ignored `stream` and sent plain JSON is
/// handled too.
#[derive(Default)]
struct StreamAssembler {
    content: String,
    reasoning: String,
    /// `reasoning_content` or `reasoning`, whichever the server streamed first.
    reasoning_key: Option<&'static str>,
    calls: Vec<PartialCall>,
    finish_reason: Value,
    usage: Value,
    /// The server's `timings` object (llama.cpp and NInfer send one with the last chunk).
    timings: Value,
    /// When the request was sent, and how long after that the first text arrived.
    started: Option<Instant>,
    first_token: Option<Duration>,
    error: Value,
    id: Value,
    model: Value,
    saw_data: bool,
    plain: String,
}

impl StreamAssembler {
    /// Takes one line of the event stream. Lines that are not `data:` lines are kept in case the
    /// whole reply was plain JSON.
    /// @param line - one line, without its line ending
    fn push_line(&mut self, line: &str) {
        let Some(payload) = line.strip_prefix("data:") else {
            self.plain.push_str(line);
            self.plain.push('\n');
            return;
        };
        self.saw_data = true;
        let payload = payload.trim();
        if payload == "[DONE]" || payload.is_empty() {
            return;
        }
        if let Ok(chunk) = serde_json::from_str::<Value>(payload) {
            self.push_chunk(&chunk);
        }
    }

    /// Adds one chunk's deltas.
    /// @param chunk - one parsed `data:` payload
    fn push_chunk(&mut self, chunk: &Value) {
        if chunk.get("error").is_some_and(|e| !e.is_null()) {
            self.error = chunk["error"].clone();
        }
        for (key, slot) in [("id", &mut self.id), ("model", &mut self.model)] {
            if slot.is_null() && chunk[key].is_string() {
                *slot = chunk[key].clone();
            }
        }
        if chunk["usage"].is_object() {
            self.usage = chunk["usage"].clone();
        }
        if chunk["timings"].is_object() {
            self.timings = chunk["timings"].clone();
        }
        for choice in chunk["choices"].as_array().into_iter().flatten() {
            if !choice["finish_reason"].is_null() {
                self.finish_reason = choice["finish_reason"].clone();
            }
            let delta = &choice["delta"];
            self.note_first_token(delta);
            self.content.push_str(delta["content"].as_str().unwrap_or(""));
            self.push_reasoning_delta(delta);
            for (position, call) in delta["tool_calls"].as_array().into_iter().flatten().enumerate() {
                self.push_call_delta(call, position);
            }
        }
    }

    /// Records the time of the first delta that carries content, reasoning or a tool call.
    /// @param delta - the chunk's `delta` object
    fn note_first_token(&mut self, delta: &Value) {
        if self.first_token.is_some() {
            return;
        }
        let text = ["content", "reasoning_content", "reasoning"].iter().any(|key| delta[*key].as_str().is_some_and(|t| !t.is_empty()));
        if text || delta["tool_calls"].as_array().is_some_and(|calls| !calls.is_empty()) {
            self.first_token = self.started.map(|started| started.elapsed());
        }
    }

    /// Adds one chunk's reasoning text and remembers the field name the server used for it.
    /// @param delta - the chunk's `delta` object
    fn push_reasoning_delta(&mut self, delta: &Value) {
        for key in ["reasoning_content", "reasoning"] {
            if let Some(text) = delta[key].as_str().filter(|text| !text.is_empty()) {
                self.reasoning_key.get_or_insert(key);
                self.reasoning.push_str(text);
                return;
            }
        }
    }

    /// Adds one tool call delta. Calls are matched by `index`, or by position when a server
    /// sends none.
    /// @param call - the delta
    /// @param position - its position in this chunk
    fn push_call_delta(&mut self, call: &Value, position: usize) {
        let index = call["index"].as_u64().map(|i| i as usize).unwrap_or(position);
        while self.calls.len() <= index {
            self.calls.push(PartialCall::default());
        }
        let slot = &mut self.calls[index];
        if let Some(id) = call["id"].as_str().filter(|id| !id.is_empty()) {
            slot.id = id.to_string();
        }
        slot.name.push_str(call["function"]["name"].as_str().unwrap_or(""));
        slot.arguments.push_str(call["function"]["arguments"].as_str().unwrap_or(""));
    }

    /// Returns the reply as whole reply JSON.
    fn finish(self) -> Result<Value, String> {
        if !self.saw_data {
            return serde_json::from_str(&self.plain).map_err(|e| format!("neither an event stream nor JSON: {e}"));
        }
        let tool_calls: Vec<Value> = self.calls.iter().map(|c| json!({ "id": c.id, "type": "function", "function": { "name": c.name, "arguments": c.arguments } })).collect();
        let mut message = json!({ "role": "assistant", "content": self.content });
        if !self.reasoning.is_empty() {
            message[self.reasoning_key.unwrap_or("reasoning_content")] = json!(self.reasoning);
        }
        if !tool_calls.is_empty() {
            message["tool_calls"] = Value::Array(tool_calls);
        }
        let mut raw = json!({ "id": self.id, "model": self.model, "object": "chat.completion", "choices": [{ "index": 0, "message": message, "finish_reason": self.finish_reason }], "usage": self.usage, "streamed": true });
        if !self.timings.is_null() {
            raw["timings"] = self.timings;
        }
        if !self.error.is_null() {
            raw["error"] = self.error;
        }
        Ok(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_a_streamed_reply_with_tool_call_deltas() {
        let lines = [
            r#"data: {"id":"c1","model":"m","choices":[{"delta":{"role":"assistant","content":""},"finish_reason":null}]}"#,
            r#"data: {"choices":[{"delta":{"reasoning_content":"think"},"finish_reason":null}]}"#,
            r#"data: {"choices":[{"delta":{"content":"Listing."},"finish_reason":null}]}"#,
            "",
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","type":"function","function":{"name":"bash","arguments":"{\"comm"}}]},"finish_reason":null}]}"#,
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"and\":\"ls\"}"}}]},"finish_reason":null}]}"#,
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":1,"id":"call_b","function":{"name":"bash","arguments":"{\"command\":\"pwd\"}"}}]},"finish_reason":"tool_calls"}]}"#,
            r#"data: {"choices":[],"usage":{"prompt_tokens":11,"completion_tokens":4,"prompt_tokens_details":{"cached_tokens":3}}}"#,
            "data: [DONE]",
        ];
        let mut assembler = StreamAssembler::default();
        lines.iter().for_each(|l| assembler.push_line(l));
        let parsed = parse_response(&assembler.finish().unwrap());
        assert_eq!(parsed.content, "Listing.");
        assert_eq!(parsed.tool_calls.len(), 2);
        assert_eq!(parsed.tool_calls[0].id, "call_a");
        assert_eq!(parsed.tool_calls[0].arguments["command"], "ls");
        assert_eq!(parsed.tool_calls[1].arguments["command"], "pwd");
        assert_eq!(parsed.finish_reason, "tool_calls");
        assert_eq!((parsed.usage.input_tokens, parsed.usage.cache_read_tokens, parsed.usage.output_tokens), (8, 3, 4));
        assert_eq!(parsed.raw["choices"][0]["message"]["reasoning_content"], "think");
    }

    #[test]
    fn keeps_the_server_timings_reasoning_tokens_and_time_to_first_token() {
        let mut assembler = StreamAssembler { started: Some(Instant::now()), ..Default::default() };
        assembler.push_line(r#"data: {"choices":[{"delta":{"role":"assistant"},"finish_reason":null}]}"#);
        assert!(assembler.first_token.is_none(), "a delta with no text is not the first token");
        assembler.push_line(r#"data: {"choices":[{"delta":{"reasoning_content":"hm"},"finish_reason":null}]}"#);
        assert!(assembler.first_token.is_some());
        assembler.push_line(r#"data: {"choices":[{"delta":{"content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"completion_tokens_details":{"reasoning_tokens":3},"prompt_tokens_details":{"cached_tokens":8}},"timings":{"cache_n":8,"prompt_n":2}}"#);
        let first = assembler.first_token;
        let raw = assembler.finish().unwrap();
        let parsed = parse_response(&raw);
        assert_eq!(parsed.usage.reasoning_tokens, 3);
        let timing = call_timing(Duration::from_millis(40), first);
        assert_eq!(raw["timings"]["cache_n"], 8);
        assert!(timing.get("timings").is_none(), "the server timings stay in the raw reply only");
        assert_eq!(timing["total_ms"], 40);
        assert!(timing["first_token_ms"].is_u64());
    }

    #[test]
    fn reads_a_plain_json_reply_when_the_server_ignored_stream() {
        let mut assembler = StreamAssembler::default();
        assembler.push_line(r#"{"choices":[{"message":{"content":"hi"},"finish_reason":"stop"}]}"#);
        assert_eq!(parse_response(&assembler.finish().unwrap()).content, "hi");
    }

    #[test]
    fn sends_user_images_as_parts_and_tool_images_after_the_tool_results() {
        let image = ImageData { media_type: "image/png".into(), data: "QUJD".into(), width: 2, height: 2, source: "a.png".into() };
        let mut assistant = Message::new(Role::Assistant, "");
        assistant.tool_calls = vec![ToolCall { id: "a".into(), name: "shot".into(), arguments: json!({}) }, ToolCall { id: "b".into(), name: "bash".into(), arguments: json!({}) }];
        let messages = vec![Message::user("look").with_images(vec![image.clone()]), assistant, Message::tool("a", "took it").with_images(vec![image]), Message::tool("b", "ok"), Message::user("next")];
        let sent = convert_messages(&messages, true);
        assert_eq!(sent[0]["content"][0]["image_url"]["url"], "data:image/png;base64,QUJD");
        assert_eq!(sent[0]["content"][1]["text"], "look");
        let roles: Vec<&str> = sent.as_array().unwrap().iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(roles, vec!["user", "assistant", "tool", "tool", "user", "user"]);
        assert_eq!(sent[2]["content"], "took it");
        assert_eq!(sent[4]["content"][1]["text"], TOOL_IMAGES_TEXT);
        assert_eq!(sent[5]["content"], "next");
    }

    #[test]
    fn reads_context_sizes_from_server_replies() {
        assert_eq!(window_from_props(&json!({"default_generation_settings": {"n_ctx": 32768}})), Some(32768));
        let models = json!({"data": [{"id": "a", "max_model_len": 1000}, {"id": "b", "context_length": 2000}]});
        assert_eq!(window_from_models(&models, "b"), Some(2000));
        assert_eq!(window_from_models(&models, "c"), None);
        assert_eq!(window_from_models(&json!({"data": [{"id": "x", "meta": {"n_ctx_train": 4096}}]}), "qwen"), Some(4096));
    }

    #[test]
    fn sends_the_streamed_reasoning_back_byte_for_byte() {
        let reasoning = "Let me look.\n\n  Two spaces, a tab\t and \u{e9}.\n";
        let lines = [
            format!("data: {}", json!({"choices":[{"delta":{"reasoning_content": &reasoning[..9]},"finish_reason":null}]})),
            format!("data: {}", json!({"choices":[{"delta":{"reasoning_content": &reasoning[9..]},"finish_reason":null}]})),
            format!("data: {}", json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c","function":{"name":"bash","arguments":"{\"command\":\"ls\"}"}}]},"finish_reason":"tool_calls"}]})),
        ];
        let mut assembler = StreamAssembler::default();
        lines.iter().for_each(|l| assembler.push_line(l));
        let response = parse_response(&assembler.finish().unwrap());
        let mut assistant = Message::new(Role::Assistant, response.content.clone());
        assistant.tool_calls = response.tool_calls.clone();
        assistant.extra = json!({ "response": response.raw });
        let sent = convert_messages(&[Message::user("go"), assistant.clone()], true);
        assert_eq!(sent[1]["reasoning_content"].as_str().unwrap(), reasoning);
        assert!(convert_messages(&[assistant], false)[0].get("reasoning_content").is_none());
    }

    #[test]
    fn keeps_the_reasoning_field_name_the_server_used() {
        let mut assembler = StreamAssembler::default();
        assembler.push_line(r#"data: {"choices":[{"delta":{"reasoning":"hm"},"finish_reason":"stop"}]}"#);
        let raw = assembler.finish().unwrap();
        let mut assistant = Message::new(Role::Assistant, "ok");
        assistant.extra = json!({ "response": raw });
        let sent = convert_messages(&[assistant], true);
        assert_eq!(sent[0]["reasoning"], "hm");
        assert!(sent[0].get("reasoning_content").is_none());
    }

    #[test]
    fn sends_a_reply_limit_to_a_local_server_when_none_is_set() {
        let config = |url: &str, max: Option<u32>| ProviderConfig { kind: "openai".into(), base_url: url.into(), max_tokens: max, ..Default::default() };
        let body = OpenAiProvider::new(config("http://127.0.0.1:8080/v1", None), "m").request_body(&[], &[]);
        assert_eq!(body["max_tokens"], 16000);
        let body = OpenAiProvider::new(config("http://localhost:8080/v1", Some(4096)), "m").request_body(&[], &[]);
        assert_eq!(body["max_tokens"], 4096);
        let body = OpenAiProvider::new(config("https://api.openai.com/v1", None), "m").request_body(&[], &[]);
        assert!(body.get("max_tokens").is_none());
    }

    #[test]
    fn asks_a_local_server_to_preserve_thinking() {
        let config = |url: &str, preserve: Option<bool>| ProviderConfig { kind: "openai".into(), base_url: url.into(), preserve_thinking: preserve, ..Default::default() };
        let body = OpenAiProvider::new(config("http://127.0.0.1:8080/v1", None), "m").request_body(&[], &[]);
        assert_eq!(body["chat_template_kwargs"]["preserve_thinking"], true);
        let body = OpenAiProvider::new(config("https://api.openai.com/v1", None), "m").request_body(&[], &[]);
        assert!(body.get("chat_template_kwargs").is_none());
        let body = OpenAiProvider::new(config("http://localhost:8080/v1", Some(false)), "m").request_body(&[], &[]);
        assert!(body.get("chat_template_kwargs").is_none());
    }

    #[test]
    fn the_thinking_setting_turns_thinking_on_off_or_on_for_the_first_request_of_a_turn() {
        let config = |thinking: &str| ProviderConfig { kind: "openai".into(), base_url: "http://127.0.0.1:8080/v1".into(), thinking: Some(thinking.into()), ..Default::default() };
        let user = vec![Message::user("go")];
        let after_tool = vec![Message::user("go"), Message::new(Role::Assistant, ""), Message::tool("1", "out")];
        let enable = |thinking: &str, messages: &[Message]| OpenAiProvider::new(config(thinking), "m").request_body(messages, &[])["chat_template_kwargs"]["enable_thinking"].clone();
        assert_eq!(enable("off", &user), json!(false));
        assert_eq!(enable("on", &after_tool), json!(true));
        assert_eq!(enable("first", &user), json!(true));
        assert_eq!(enable("first", &after_tool), json!(false));
        let body = OpenAiProvider::new(config("first"), "m").request_body(&user, &[]);
        assert_eq!(body["chat_template_kwargs"]["preserve_thinking"], true, "preserve_thinking is kept beside it");
    }

    #[test]
    fn round_trips_tool_calls() {
        let raw = json!({"choices": [{"message": {"content": null, "tool_calls": [{"id": "x", "type": "function", "function": {"name": "bash", "arguments": "{\"command\":\"ls\"}"}}]}, "finish_reason": "tool_calls"}], "usage": {"prompt_tokens": 7, "completion_tokens": 3}});
        let parsed = parse_response(&raw);
        assert_eq!(parsed.tool_calls[0].arguments["command"], "ls");
        let mut assistant = Message::new(Role::Assistant, "");
        assistant.tool_calls = parsed.tool_calls.clone();
        let sent = convert_messages(&[assistant], true);
        assert_eq!(sent[0]["tool_calls"][0]["function"]["arguments"], "{\"command\":\"ls\"}");
    }
}
