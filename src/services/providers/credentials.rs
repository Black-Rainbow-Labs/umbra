//! Finds the credential for a provider without ever printing or storing it.

use anyhow::{bail, Result};

use crate::models::config::ProviderConfig;
use crate::utils::paths::home_dir;

/// How a request proves who it is.
#[derive(Clone)]
pub enum Credential {
    /// Sent as `x-api-key` (Anthropic) or `Authorization: Bearer` (OpenAI compatible).
    ApiKey(String),
    /// An OAuth access token, sent as `Authorization: Bearer` with the Anthropic OAuth beta header.
    OAuthToken(String),
}

impl std::fmt::Debug for Credential {
    /// Prints the kind of credential and never its value.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self { Credential::ApiKey(_) => "api key", Credential::OAuthToken(_) => "oauth token" };
        write!(f, "Credential({kind})")
    }
}

impl Credential {
    /// A short label for logs and the trajectory.
    pub fn kind(&self) -> &'static str {
        match self { Credential::ApiKey(_) => "api_key", Credential::OAuthToken(_) => "oauth_token" }
    }
}

/// Reads the API key from the config or its environment variable.
/// @param config - the provider settings
pub fn api_key(config: &ProviderConfig) -> Option<String> {
    if !config.api_key.is_empty() {
        return Some(config.api_key.clone());
    }
    if config.api_key_env.is_empty() {
        return None;
    }
    std::env::var(&config.api_key_env).ok().filter(|v| !v.is_empty())
}

/// Resolves an Anthropic credential. With `auth = "auto"` (the default) the order is: the API key,
/// then `ANTHROPIC_AUTH_TOKEN`, then the login Claude Code saved in `~/.claude/.credentials.json`.
/// @param config - the provider settings
pub fn anthropic_credential(config: &ProviderConfig) -> Result<Credential> {
    let auth = if config.auth.is_empty() { "auto" } else { config.auth.as_str() };
    if auth == "auto" || auth == "api_key" {
        if let Some(key) = api_key(config) {
            return Ok(Credential::ApiKey(key));
        }
        if let Some(token) = std::env::var("ANTHROPIC_AUTH_TOKEN").ok().filter(|v| !v.is_empty()) {
            return Ok(Credential::OAuthToken(token));
        }
        if auth == "api_key" {
            bail!("no Anthropic API key: set {}", config.api_key_env);
        }
    }
    claude_code_token()
}

/// Reads the access token from Claude Code's saved login. Umbra only reads the file: it never
/// refreshes the token, because a refresh replaces the refresh token Claude Code depends on.
pub fn claude_code_token() -> Result<Credential> {
    let path = home_dir().join(".claude").join(".credentials.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        bail!("no Anthropic credential found. Set ANTHROPIC_API_KEY, or sign in to Claude Code so {} exists", path.display());
    };
    let json: serde_json::Value = serde_json::from_str(&text)?;
    let oauth = &json["claudeAiOauth"];
    let Some(token) = oauth["accessToken"].as_str() else {
        bail!("{} has no claudeAiOauth.accessToken", path.display());
    };
    if let Some(expires) = oauth["expiresAt"].as_i64() {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        if expires <= now {
            bail!("the Claude Code login in {} has expired. Run claude once to refresh it, or set ANTHROPIC_API_KEY", path.display());
        }
    }
    Ok(Credential::OAuthToken(token.to_string()))
}
