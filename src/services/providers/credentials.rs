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
/// then `ANTHROPIC_AUTH_TOKEN`, then the login Claude Code saved (`claude_code_token`).
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

/// Reads the access token from Claude Code's saved login: `~/.claude/.credentials.json`, or on
/// macOS the `Claude Code-credentials` Keychain entry, where Claude Code keeps it there. Umbra only
/// reads the login: it never refreshes the token, because a refresh replaces the refresh token
/// Claude Code depends on.
pub fn claude_code_token() -> Result<Credential> {
    let Some((text, source)) = claude_code_login() else {
        bail!("no Anthropic credential found. Set ANTHROPIC_API_KEY, or sign in to Claude Code (run claude) so its login is saved");
    };
    token_from_login(&text, &source)
}

/// Whether Claude Code's saved login can be found, in the file or the macOS Keychain.
pub fn claude_code_login_exists() -> bool {
    claude_code_login().is_some()
}

/// Claude Code's saved login as JSON text, with where it was read from for messages.
fn claude_code_login() -> Option<(String, String)> {
    let path = home_dir().join(".claude").join(".credentials.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        return Some((text, path.display().to_string()));
    }
    keychain_login().map(|text| (text, format!("the macOS Keychain entry '{KEYCHAIN_SERVICE}'")))
}

/// The Keychain entry Claude Code saves its login in on macOS.
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

/// Reads Claude Code's login from the macOS Keychain with `/usr/bin/security`. Skipped when
/// `UMBRA_HOME_OVERRIDE` points at a fake home folder, so tests never read the real login.
fn keychain_login() -> Option<String> {
    if !cfg!(target_os = "macos") || std::env::var("UMBRA_HOME_OVERRIDE").is_ok_and(|v| !v.is_empty()) {
        return None;
    }
    let output = std::process::Command::new("/usr/bin/security").args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"]).stderr(std::process::Stdio::null()).output().ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    (output.status.success() && !text.trim().is_empty()).then(|| text.trim().to_string())
}

/// Takes the access token out of Claude Code's login JSON, refusing one that has expired.
/// @param text - the login JSON
/// @param source - where it was read from, for messages
pub fn token_from_login(text: &str, source: &str) -> Result<Credential> {
    let json: serde_json::Value = serde_json::from_str(text).map_err(|e| anyhow::anyhow!("{source} is not valid JSON: {e}"))?;
    let oauth = &json["claudeAiOauth"];
    let Some(token) = oauth["accessToken"].as_str() else {
        bail!("{source} has no claudeAiOauth.accessToken");
    };
    if let Some(expires) = oauth["expiresAt"].as_i64() {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        if expires <= now {
            bail!("the Claude Code login in {source} has expired. Run claude once to refresh it, or set ANTHROPIC_API_KEY");
        }
    }
    Ok(Credential::OAuthToken(token.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_json_gives_the_access_token() {
        let text = r#"{"claudeAiOauth":{"accessToken":"tok","expiresAt":99999999999999}}"#;
        assert!(matches!(token_from_login(text, "the Keychain").unwrap(), Credential::OAuthToken(t) if t == "tok"));
    }

    #[test]
    fn an_expired_login_names_where_it_came_from() {
        let text = r#"{"claudeAiOauth":{"accessToken":"tok","expiresAt":1}}"#;
        let message = token_from_login(text, "the macOS Keychain entry 'Claude Code-credentials'").unwrap_err().to_string();
        assert!(message.contains("has expired") && message.contains("Keychain"), "{message}");
    }

    #[test]
    fn a_login_without_a_token_is_refused() {
        assert!(token_from_login(r#"{"claudeAiOauth":{}}"#, "x").unwrap_err().to_string().contains("no claudeAiOauth.accessToken"));
    }
}
