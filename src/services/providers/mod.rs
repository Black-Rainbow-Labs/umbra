//! Model providers. Each one turns Umbra's message list into a request for one API and turns the
//! reply back into a `ModelResponse`.

pub mod anthropic;
pub mod credentials;
pub mod openai;
pub mod pricing;
pub mod replay;

use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::Value;

use crate::models::config::{ProviderConfig, UmbraConfig};
use crate::models::message::Message;
use crate::models::model_response::ModelResponse;

/// A tool offered to the model in tool mode.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Why a model call failed, and whether trying again could help.
#[derive(Debug, Clone)]
pub struct ProviderError {
    pub message: String,
    pub retryable: bool,
    /// True when the server refused the request because it is longer than the context window.
    pub context_overflow: bool,
}

/// Phrases servers use when a request is longer than the model's context window: Anthropic,
/// OpenAI, llama.cpp, vLLM and OpenRouter.
const OVERFLOW_PHRASES: &[&str] = &["prompt is too long", "context length", "context_length_exceeded", "maximum context", "exceeds the available context size", "too many tokens", "context window"];

impl ProviderError {
    /// Builds an error from an HTTP status and body. 408, 409, 429 and 5xx are retryable;
    /// authentication, permission, not found and bad request errors are not.
    /// @param status - the HTTP status code
    /// @param body - the response body
    pub fn from_status(status: u16, body: &str) -> Self {
        let retryable = matches!(status, 408 | 409 | 429 | 500..=599);
        let context_overflow = matches!(status, 400 | 413) && is_overflow_text(body);
        ProviderError { message: format!("HTTP {status}: {}", body.chars().take(2000).collect::<String>()), retryable: retryable && !context_overflow, context_overflow }
    }

    /// Builds a retryable error for a request that never reached the server.
    /// @param message - the network error
    pub fn network(message: impl Into<String>) -> Self {
        ProviderError { message: message.into(), retryable: true, context_overflow: false }
    }

    /// Builds an error that retrying will not fix.
    /// @param message - the error text
    pub fn fatal(message: impl Into<String>) -> Self {
        ProviderError { message: message.into(), retryable: false, context_overflow: false }
    }
}

/// Returns true when an error body says the request was longer than the context window.
/// @param body - the error text from the server
pub fn is_overflow_text(body: &str) -> bool {
    let lower = body.to_lowercase();
    OVERFLOW_PHRASES.iter().any(|phrase| lower.contains(phrase))
}

/// One model behind one API.
pub trait Provider {
    /// Sends the conversation and returns the model's reply.
    /// @param messages - the whole history; `Exit` messages are never sent
    /// @param tools - the tools offered in tool mode, empty in text mode
    fn complete(&mut self, messages: &[Message], tools: &[ToolSpec]) -> Result<ModelResponse, ProviderError>;

    /// The model id sent to the API.
    fn model_name(&self) -> &str;

    /// The settings recorded in the trajectory, without secrets.
    fn describe(&self) -> Value;

    /// The model's context window in tokens, when the provider can find it out.
    fn context_window(&mut self) -> Option<u64> {
        None
    }
}

/// Builds the provider for a `provider/model` string such as `anthropic/claude-haiku-5-5`.
/// @param config - the merged config, which holds the provider table
/// @param model_spec - the model string from the command line or config
pub fn create_provider(config: &UmbraConfig, model_spec: &str) -> Result<Box<dyn Provider>> {
    let (provider_name, model) = split_model_spec(model_spec)?;
    let provider_config = config.providers.get(provider_name).ok_or_else(|| anyhow!("unknown provider '{provider_name}'. Known providers: {}", config.providers.keys().cloned().collect::<Vec<_>>().join(", ")))?;
    match provider_config.kind.as_str() {
        "anthropic" => Ok(Box::new(anthropic::AnthropicProvider::new(provider_config.clone(), model)?)),
        "openai" => Ok(Box::new(openai::OpenAiProvider::new(provider_config.clone(), model))),
        "replay" => Ok(Box::new(replay::ReplayProvider::new(provider_config, model)?)),
        other => bail!("provider '{provider_name}' has unknown kind '{other}'. Use anthropic, openai or replay"),
    }
}

/// Splits `provider/model`. A bare name that starts with `claude` means the anthropic provider.
/// @param model_spec - the model string
pub fn split_model_spec(model_spec: &str) -> Result<(&str, &str)> {
    if model_spec.is_empty() {
        bail!("no model set. Pass -m provider/model, for example -m anthropic/claude-haiku-5-5 or -m local/qwen, or set model in ~/.umbra/config.toml");
    }
    match model_spec.split_once('/') {
        Some((provider, model)) => Ok((provider, model)),
        None if model_spec.starts_with("claude") => Ok(("anthropic", model_spec)),
        None => bail!("model '{model_spec}' needs a provider prefix, for example local/{model_spec}"),
    }
}

/// Runs a model call, retrying retryable failures with exponential backoff: waits of 4, 8, 16,
/// 32 then 60 seconds, the mini-swe-agent schedule. `UMBRA_RETRY_BASE_MS` shortens the waits.
/// @param max_retries - how many times to retry after the first attempt
/// @param call - the request to make
pub fn with_retries<T>(max_retries: u32, mut call: impl FnMut() -> Result<T, ProviderError>) -> Result<T, ProviderError> {
    let base_ms: u64 = std::env::var("UMBRA_RETRY_BASE_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(4000);
    let mut attempt = 0;
    loop {
        match call() {
            Ok(value) => return Ok(value),
            Err(error) if error.retryable && attempt < max_retries => {
                let wait = (base_ms.saturating_mul(1 << attempt.min(6))).min(60_000);
                eprintln!("umbra: model call failed ({}), retrying in {:.1}s", error.message.lines().next().unwrap_or(""), wait as f64 / 1000.0);
                std::thread::sleep(Duration::from_millis(wait));
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Returns the configured retry count, 9 by default (10 attempts, like mini-swe-agent).
/// @param config - the provider settings
pub fn retry_count(config: &ProviderConfig) -> u32 {
    config.max_retries.unwrap_or(9)
}

/// Copies the provider's `extra_body` fields into a request body.
/// @param body - the request body being built
/// @param config - the provider settings
pub fn apply_extra_body(body: &mut Value, config: &ProviderConfig) {
    if let Value::Object(map) = body {
        for (key, value) in &config.extra_body {
            map.insert(key.clone(), value.clone());
        }
    }
}
