//! The logic behind the configuration modal: reading a setting's value and where it came from,
//! turning typed text into a value, saving changes into a config file with comments kept, and
//! testing a provider.

use std::path::Path;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use super::providers::credentials::{anthropic_credential, api_key, Credential};
use super::providers::create_provider;
use crate::models::config::{ProviderConfig, UmbraConfig};
use crate::models::message::Message;
use crate::models::settings_schema::{FieldKind, SettingSpec};
use crate::utils::http::{get_text, http_agent};
use crate::utils::paths::home_dir;

/// Where a value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin { Default, User, Project }

/// Reads a setting's current value from the merged config as JSON (`providers.local.base_url` style paths; None when absent).
/// @param config - the merged config
/// @param path - the setting path
pub fn current_value(config: &UmbraConfig, path: &str) -> Option<Value> {
    let mut value = serde_json::to_value(config).ok()?;
    for segment in path.split('.') {
        value = value.get_mut(segment)?.take();
    }
    (!value.is_null()).then_some(value)
}

/// Returns true when a TOML file sets the path.
/// @param file - a config file, which may not exist
/// @param path - the setting path
fn file_has_path(file: &Path, path: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(file) else { return false };
    let Ok(table) = text.parse::<toml::Table>() else { return false };
    let mut value: Option<&toml::Value> = None;
    let mut current = Some(&table);
    for segment in path.split('.') {
        let Some(found) = current.and_then(|t| t.get(segment)) else { return false };
        value = Some(found);
        current = found.as_table();
    }
    value.is_some()
}

/// Which file a value comes from: the project file wins over the user file; neither means Default. Reads the TOML files directly.
/// @param path - the setting path
/// @param user_file - `~/.umbra/config.toml`
/// @param project_file - `<project>/.umbra/config.toml`
pub fn value_origin(path: &str, user_file: &Path, project_file: &Path) -> Origin {
    if file_has_path(project_file, path) {
        Origin::Project
    } else if file_has_path(user_file, path) {
        Origin::User
    } else {
        Origin::Default
    }
}

/// Parses what the person typed for a field. Error text is one short sentence for the modal.
/// @param spec - the setting being edited
/// @param input - the typed text
pub fn parse_input(spec: &SettingSpec, input: &str) -> Result<Value, String> {
    let text = input.trim();
    match spec.kind {
        FieldKind::Toggle => match text.to_lowercase().as_str() {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err("Enter true or false.".into()),
        },
        FieldKind::Integer { min, max } => {
            let number: i64 = text.parse().map_err(|_| "Enter a whole number.".to_string())?;
            if number < min || number > max { return Err(format!("Enter a number from {min} to {max}.")); }
            Ok(Value::from(number))
        }
        FieldKind::Float { min, max } => float_in_range(text, min, max),
        FieldKind::Percent => {
            let number = text.trim_end_matches('%').trim().parse::<f64>().map_err(|_| "Enter a percent such as 70.".to_string())?;
            if !(0.0..=100.0).contains(&number) { return Err("Enter a percent from 0 to 100.".into()); }
            Ok(Value::from((number / 100.0 * 1e6).round() / 1e6))
        }
        FieldKind::Choice(options) => match options.contains(&text) {
            true => Ok(Value::String(text.to_string())),
            false => Err(format!("Choose one of: {}.", options.join(", "))),
        },
        FieldKind::List => Ok(Value::Array(text.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| Value::String(s.to_string())).collect())),
        FieldKind::Map => parse_map(text),
        FieldKind::Url => match text.starts_with("http://") || text.starts_with("https://") {
            true => Ok(Value::String(text.to_string())),
            false => Err("Enter an address that starts with http:// or https://.".into()),
        },
        FieldKind::Text | FieldKind::Secret | FieldKind::Model => Ok(Value::String(text.to_string())),
    }
}

/// Parses a decimal number and checks its range.
/// @param text - the typed text
/// @param min - the smallest allowed value
/// @param max - the largest allowed value
fn float_in_range(text: &str, min: f64, max: f64) -> Result<Value, String> {
    let number: f64 = text.parse().map_err(|_| "Enter a number.".to_string())?;
    if number < min || number > max { return Err(format!("Enter a number from {min} to {max}.")); }
    Ok(Value::from(number))
}

/// Parses `k=v, k=v` into a JSON object of strings.
/// @param text - the typed text
fn parse_map(text: &str) -> Result<Value, String> {
    let mut map = serde_json::Map::new();
    for pair in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let Some((key, value)) = pair.split_once('=') else { return Err(format!("\"{pair}\" needs a name, an equals sign and a value.")) };
        if key.trim().is_empty() { return Err("A name is empty.".into()); }
        map.insert(key.trim().to_string(), Value::String(value.trim().to_string()));
    }
    Ok(Value::Object(map))
}

/// How a value is shown in a field: Percent 0.7 -> "70%", Secret -> dots plus the last 4 characters (never more), List joined with ", ", Map "k=v, ...".
/// @param spec - the setting
/// @param value - its current value, if any
pub fn display_value(spec: &SettingSpec, value: Option<&Value>) -> String {
    let Some(value) = value.filter(|v| !v.is_null()) else { return String::new() };
    match (spec.kind, value) {
        (FieldKind::Percent, Value::Number(n)) => format!("{}%", (n.as_f64().unwrap_or(0.0) * 1000.0).round() / 10.0),
        (FieldKind::Secret, Value::String(s)) => mask_secret(s),
        (_, Value::Array(items)) => items.iter().map(plain_text).collect::<Vec<_>>().join(", "),
        (_, Value::Object(map)) => map.iter().map(|(k, v)| format!("{k}={}", plain_text(v))).collect::<Vec<_>>().join(", "),
        (_, other) => plain_text(other),
    }
}

/// Shows a secret as dots with its last 4 characters. A secret of 8 characters or fewer is all dots.
/// @param secret - the secret value
fn mask_secret(secret: &str) -> String {
    let chars: Vec<char> = secret.chars().collect();
    if chars.is_empty() { return String::new(); }
    if chars.len() <= 8 { return "\u{2022}".repeat(chars.len()); }
    format!("{}{}", "\u{2022}".repeat(8), chars[chars.len() - 4..].iter().collect::<String>())
}

/// A JSON value as plain text: a string without quotes, anything else as JSON.
/// @param value - the value
fn plain_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Converts a JSON value to a TOML value for `toml_edit`.
/// @param value - the JSON value
fn json_to_toml(value: &Value) -> Result<toml_edit::Value> {
    Ok(match value {
        Value::String(s) => toml_edit::Value::from(s.as_str()),
        Value::Bool(b) => toml_edit::Value::from(*b),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => toml_edit::Value::from(i),
            (None, Some(f)) => toml_edit::Value::from(f),
            _ => bail!("number {n} cannot be written"),
        },
        Value::Array(items) => toml_edit::Value::Array(items.iter().map(json_to_toml).collect::<Result<_>>()?),
        Value::Object(map) => {
            let mut table = toml_edit::InlineTable::new();
            for (key, item) in map { table.insert(key, json_to_toml(item)?); }
            toml_edit::Value::InlineTable(table)
        }
        Value::Null => bail!("null cannot be written to a TOML file"),
    })
}

/// Finds the table that holds a path's last key, creating missing tables when asked.
/// @param root - the document's root table
/// @param parents - the table names leading to the key
/// @param create - create tables that are missing; otherwise return None for a missing one
fn table_for<'a>(root: &'a mut toml_edit::Table, parents: &[&str], create: bool) -> Result<Option<&'a mut toml_edit::Table>> {
    let mut table = root;
    for name in parents {
        if !table.contains_key(name) {
            if !create { return Ok(None); }
            let mut fresh = toml_edit::Table::new();
            fresh.set_implicit(true);
            table.insert(name, toml_edit::Item::Table(fresh));
        }
        table = match table.get_mut(name).and_then(|item| item.as_table_mut()) {
            Some(next) => next,
            None => bail!("{name} is not a table"),
        };
    }
    Ok(Some(table))
}

/// Applies one change to the document, keeping the key's comments and the old value's trailing comment.
/// @param doc - the parsed document
/// @param path - the setting path
/// @param change - the new value, or None to remove the key
fn apply_change(doc: &mut toml_edit::DocumentMut, path: &str, change: &Option<Value>) -> Result<()> {
    let segments: Vec<&str> = path.split('.').collect();
    let Some((key, parents)) = segments.split_last() else { bail!("empty setting path") };
    let Some(table) = table_for(doc.as_table_mut(), parents, change.is_some())? else { return Ok(()) };
    let Some(new_value) = change else {
        table.remove(key);
        return Ok(());
    };
    let mut value = json_to_toml(new_value)?;
    match table.get_mut(key) {
        Some(item) => {
            if let Some(old) = item.as_value() { *value.decor_mut() = old.decor().clone(); }
            *item = toml_edit::Item::Value(value);
        }
        None => { table.insert(key, toml_edit::Item::Value(value)); }
    }
    Ok(())
}

/// Writes changed keys into a TOML file with toml_edit so comments and key order are kept; Some(value) sets, None removes the key. Creates missing tables. Writes a temporary file next to it then renames. Returns an error and leaves the file unchanged on failure.
/// @param file - the config file, which may not exist yet
/// @param changes - setting paths with their new values
pub fn write_changes(file: &Path, changes: &[(String, Option<Value>)]) -> Result<()> {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let mut doc: toml_edit::DocumentMut = text.parse().with_context(|| format!("{} is not valid TOML, so it was left alone", file.display()))?;
    for (path, change) in changes {
        apply_change(&mut doc, path, change).with_context(|| format!("setting {path}"))?;
    }
    if let Some(folder) = file.parent() { std::fs::create_dir_all(folder)?; }
    let temporary = file.with_extension("toml.tmp");
    std::fs::write(&temporary, doc.to_string()).with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, file).with_context(|| format!("replacing {}", file.display()))
}

/// One step of a provider test.
#[derive(Debug, Clone, PartialEq)]
pub struct TestStep { pub name: &'static str, pub ok: bool, pub text: String }

/// Builds a test step.
/// @param name - the step name
/// @param ok - whether it passed
/// @param text - the line shown for it
fn step(name: &'static str, ok: bool, text: impl Into<String>) -> TestStep {
    TestStep { name, ok, text: text.into() }
}

/// Tests a provider: credentials, reachability, completion. Calls `progress` after each step. Never puts a key or token value in any text. Network calls use crate::utils::http with a 15 second timeout.
/// @param config - the merged config
/// @param provider - the provider name
/// @param model - the model to ask
/// @param progress - called with each step as it finishes
pub fn test_provider(config: &UmbraConfig, provider: &str, model: &str, progress: &mut dyn FnMut(&TestStep)) -> Vec<TestStep> {
    let mut steps: Vec<TestStep> = Vec::new();
    let Some(settings) = config.providers.get(provider) else {
        let first = step("credentials", false, format!("no provider named {provider}"));
        progress(&first);
        return vec![first];
    };
    let secret = api_key(settings).unwrap_or_default();
    let mut config = config.clone();
    if let Some(entry) = config.providers.get_mut(provider) { entry.max_retries = Some(0); }
    let model_config = &config;
    for number in 0..3 {
        let mut result = match number {
            0 => credentials_step(settings),
            1 => reachability_step(settings),
            _ => completion_step(model_config, provider, model),
        };
        if !secret.is_empty() { result.text = result.text.replace(&secret, "[key]"); }
        progress(&result);
        let stop = !result.ok && number == 0;
        steps.push(result);
        if stop { break; }
    }
    steps
}

/// Step 1: finds which credential the provider would use, without showing it.
/// @param settings - the provider settings
fn credentials_step(settings: &ProviderConfig) -> TestStep {
    match settings.kind.as_str() {
        "anthropic" => match anthropic_credential(settings) {
            Ok(Credential::ApiKey(_)) => step("credentials", true, key_source(settings)),
            Ok(Credential::OAuthToken(_)) => step("credentials", true, oauth_source()),
            Err(error) => step("credentials", false, error.to_string().lines().next().unwrap_or("no credential").to_string()),
        },
        "openai" if api_key(settings).is_some() => step("credentials", true, key_source(settings)),
        "openai" if is_local(&settings.base_url) => step("credentials", true, "no key needed for a local server"),
        "openai" => step("credentials", false, format!("no credential: set {}", if settings.api_key_env.is_empty() { "api_key" } else { &settings.api_key_env })),
        _ => step("credentials", true, "no credential needed"),
    }
}

/// Describes where the API key comes from, never the key.
/// @param settings - the provider settings
fn key_source(settings: &ProviderConfig) -> String {
    if !settings.api_key.is_empty() { "API key from the config file".to_string() } else { format!("API key from {}", settings.api_key_env) }
}

/// Describes an OAuth token: the environment variable, or the Claude Code login with its expiry.
fn oauth_source() -> String {
    if std::env::var("ANTHROPIC_AUTH_TOKEN").is_ok_and(|v| !v.is_empty()) { return "token from ANTHROPIC_AUTH_TOKEN".into(); }
    match claude_login_expiry_ms() {
        Some(expires) => {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
            let minutes = ((expires - now) / 60_000).max(0);
            format!("Claude Code login, expires in {}h {:02}m", minutes / 60, minutes % 60)
        }
        None => "Claude Code login".into(),
    }
}

/// Reads when Claude Code's saved login expires, in epoch milliseconds, from its file.
fn claude_login_expiry_ms() -> Option<i64> {
    let text = std::fs::read_to_string(home_dir().join(".claude").join(".credentials.json")).ok()?;
    serde_json::from_str::<Value>(&text).ok()?["claudeAiOauth"]["expiresAt"].as_i64()
}

/// Returns true for an address on this machine.
/// @param url - the base URL
fn is_local(url: &str) -> bool {
    let url = url.to_lowercase();
    url.contains("://localhost") || url.contains("://127.0.0.1") || url.contains("://[::1]")
}

/// The request headers for the provider's own server.
/// @param settings - the provider settings
fn request_headers(settings: &ProviderConfig) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    if let Some(key) = api_key(settings) { headers.push(("authorization".to_string(), format!("Bearer {key}"))); }
    headers.extend(settings.headers.iter().map(|(k, v)| (k.clone(), v.clone())));
    headers
}

/// Step 2: for an OpenAI compatible server, asks for its model list and, for llama.cpp, its loaded model.
/// @param settings - the provider settings
fn reachability_step(settings: &ProviderConfig) -> TestStep {
    if settings.kind != "openai" { return step("reachability", true, if settings.kind == "anthropic" { "skipped for Anthropic" } else { "skipped" }); }
    let agent = http_agent(15);
    let base = settings.base_url.trim_end_matches('/');
    let started = Instant::now();
    let reply = match get_text(&agent, &format!("{base}/models"), &request_headers(settings)) {
        Ok(reply) => reply,
        Err(error) => return step("reachability", false, format!("could not connect: {error}")),
    };
    let mut text = format!("HTTP {} in {} ms", reply.status, started.elapsed().as_millis());
    if reply.status == 200 {
        let json: Value = serde_json::from_str(&reply.body).unwrap_or(Value::Null);
        let count = json["data"].as_array().or_else(|| json["models"].as_array()).map(|a| a.len());
        if let Some(count) = count { text.push_str(&format!(", {count} models")); }
    }
    if let Some(props) = llama_props_text(&agent, base) { text.push_str(&format!("; {props}")); }
    step("reachability", reply.status == 200, text)
}

/// Asks a llama.cpp server for `/props` and describes the loaded model and context size.
/// @param agent - the HTTP agent
/// @param base - the base URL without a trailing slash
fn llama_props_text(agent: &ureq::Agent, base: &str) -> Option<String> {
    let root = base.strip_suffix("/v1").unwrap_or(base);
    let reply = get_text(agent, &format!("{root}/props"), &[]).ok().filter(|r| r.status == 200)?;
    let json: Value = serde_json::from_str(&reply.body).ok()?;
    let path = json["model_path"].as_str()?;
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match json["default_generation_settings"]["n_ctx"].as_u64() {
        Some(context) => Some(format!("llama.cpp: {name}, context {context}")),
        None => Some(format!("llama.cpp: {name}")),
    }
}

/// Step 3: sends one short message and reports the time and the start of the reply, or the error.
/// @param config - the config, with retries turned off for this provider
/// @param provider - the provider name
/// @param model - the model to ask
fn completion_step(config: &UmbraConfig, provider: &str, model: &str) -> TestStep {
    let mut model_provider = match create_provider(config, &format!("{provider}/{model}")) {
        Ok(p) => p,
        Err(error) => return step("completion", false, error.to_string().lines().next().unwrap_or("").to_string()),
    };
    let started = Instant::now();
    let messages = [Message::system("Answer in one short sentence."), Message::user("Say hello.")];
    match model_provider.complete(&messages, &[]) {
        Ok(reply) => {
            let first: String = reply.content.trim().lines().next().unwrap_or("").chars().take(60).collect();
            step("completion", true, format!("answered in {:.1} s: {first}", started.elapsed().as_secs_f64()))
        }
        Err(error) => {
            let line = error.message.lines().next().unwrap_or("").to_string();
            let text = if line.starts_with("HTTP ") { line } else { format!("could not connect: {line}") };
            step("completion", false, text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::settings_schema::spec_for;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};

    /// Makes a fresh temporary folder for one test.
    /// @param name - the test name
    fn temp_folder(name: &str) -> std::path::PathBuf {
        let folder = std::env::temp_dir().join(format!("umbra-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    /// Writes changes for a test, with Percent input parsed through `parse_input`.
    #[test]
    fn write_changes_keeps_comments_and_order() {
        let folder = temp_folder("write");
        let file = folder.join("config.toml");
        std::fs::write(&file, "# my comment\nmodel = \"a/b\" # trailing\nstep_limit = 7\n\n[compaction]\ntrigger_ratio = 0.8 # keep\n").unwrap();
        let percent = parse_input(spec_for("compaction.trigger_ratio").unwrap(), "70").unwrap();
        let changes = vec![("compaction.trigger_ratio".to_string(), Some(percent)), ("providers.x.base_url".to_string(), Some(Value::String("http://h/v1".into()))), ("step_limit".to_string(), None)];
        write_changes(&file, &changes).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.starts_with("# my comment\nmodel = \"a/b\" # trailing\n"), "{text}");
        assert!(text.contains("trigger_ratio = 0.7 # keep"), "{text}");
        assert!(text.contains("[providers.x]\nbase_url = \"http://h/v1\""), "{text}");
        assert!(!text.contains("step_limit"), "{text}");
        assert!(!text.contains("[providers]\n"), "{text}");
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// A file that is not valid TOML is left as it was.
    #[test]
    fn write_changes_leaves_invalid_toml_alone() {
        let folder = temp_folder("invalid");
        let file = folder.join("config.toml");
        std::fs::write(&file, "model = = broken").unwrap();
        let result = write_changes(&file, &[("step_limit".to_string(), Some(Value::from(3)))]);
        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "model = = broken");
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// The origin is the project file, then the user file, then the default.
    #[test]
    fn origin_prefers_the_project_file() {
        let folder = temp_folder("origin");
        let (user, project) = (folder.join("user.toml"), folder.join("project.toml"));
        std::fs::write(&user, "step_limit = 1\n[compaction]\nenabled = false\n").unwrap();
        std::fs::write(&project, "step_limit = 2\n").unwrap();
        assert_eq!(value_origin("step_limit", &user, &project), Origin::Project);
        assert_eq!(value_origin("compaction.enabled", &user, &project), Origin::User);
        assert_eq!(value_origin("mode", &user, &project), Origin::Default);
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// Typed input is checked and converted, and secrets show at most 4 characters.
    #[test]
    fn parsing_and_display() {
        assert!(parse_input(spec_for("step_limit").unwrap(), "abc").is_err());
        assert_eq!(parse_input(spec_for("compaction.trigger_ratio").unwrap(), "70").unwrap(), Value::from(0.7));
        assert!(parse_input(spec_for("mode").unwrap(), "fast").is_err());
        assert!(parse_input(spec_for("providers.a.base_url").unwrap(), "localhost").is_err());
        assert_eq!(parse_input(spec_for("providers.a.headers").unwrap(), "a=1, b=2").unwrap()["b"], "2");
        let secret = display_value(spec_for("providers.a.api_key").unwrap(), Some(&Value::String("sk-abcdefgh1234".into())));
        assert!(secret.ends_with("1234") && !secret.contains("abcd") && secret.contains('\u{2022}'), "{secret}");
        assert_eq!(display_value(spec_for("compaction.trigger_ratio").unwrap(), Some(&Value::from(0.7))), "70%");
    }

    /// Reads one HTTP request and returns its method and path, draining the body.
    /// @param stream - the accepted connection
    fn read_request(stream: &mut TcpStream) -> (String, String) {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        let mut length = 0usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line.trim().is_empty() { break; }
            if let Some(v) = line.to_lowercase().strip_prefix("content-length:") { length = v.trim().parse().unwrap_or(0); }
        }
        let mut body = vec![0u8; length];
        reader.read_exact(&mut body).unwrap();
        let mut parts = first.split_whitespace();
        (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string())
    }

    /// Writes one HTTP/1.1 reply and closes the connection.
    /// @param stream - the connection
    /// @param status - the status line, such as `200 OK`
    /// @param body - the body text
    fn reply(stream: &mut TcpStream, status: &str, body: &str) {
        let text = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        let _ = stream.write_all(text.as_bytes());
    }

    /// Starts a fake server on a free port. Returns its port. `completion` is the status and body of the chat completion.
    /// @param completion - the status line and body for POST /v1/chat/completions
    fn start_server(completion: (&'static str, &'static str)) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                std::thread::spawn(move || {
                    let (method, path) = read_request(&mut stream);
                    match (method.as_str(), path.as_str()) {
                        ("GET", "/v1/models") => reply(&mut stream, "200 OK", r#"{"data":[{"id":"a"},{"id":"b"},{"id":"c"}]}"#),
                        ("POST", "/v1/chat/completions") => reply(&mut stream, completion.0, completion.1),
                        _ => reply(&mut stream, "404 Not Found", "{}"),
                    }
                });
            }
        });
        port
    }

    /// Builds a config with a `fake` OpenAI compatible provider.
    /// @param port - the port the fake server listens on
    fn fake_config(port: u16) -> UmbraConfig {
        let mut config = UmbraConfig::default();
        let provider = ProviderConfig { kind: "openai".into(), base_url: format!("http://127.0.0.1:{port}/v1"), api_key: "sk-secret-value-9999".into(), stream: Some(false), ..Default::default() };
        config.providers.insert("fake".into(), provider);
        config
    }

    /// Every step passes and the texts have the model count and the reply.
    #[test]
    fn provider_test_passes_against_a_working_server() {
        let port = start_server(("200 OK", r#"{"choices":[{"message":{"role":"assistant","content":"Hello there!"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1}}"#));
        let mut seen = 0;
        let steps = test_provider(&fake_config(port), "fake", "m", &mut |_| seen += 1);
        assert_eq!(seen, 3);
        assert!(steps.iter().all(|s| s.ok), "{steps:?}");
        assert!(steps[1].text.contains("3 models"), "{steps:?}");
        assert!(steps[2].text.contains("Hello there!"), "{steps:?}");
        assert!(steps.iter().all(|s| !s.text.contains("sk-secret-value-9999")));
    }

    /// A 401 shows the status and only the first line of the body.
    #[test]
    fn provider_test_reports_an_auth_error() {
        let port = start_server(("401 Unauthorized", "bad key\nmore"));
        let steps = test_provider(&fake_config(port), "fake", "m", &mut |_| {});
        assert!(!steps[2].ok);
        assert!(steps[2].text.contains("401") && steps[2].text.contains("bad key") && !steps[2].text.contains("more"), "{steps:?}");
        assert!(steps.iter().all(|s| !s.text.contains("sk-secret-value-9999")));
    }

    /// A closed port says could not connect.
    #[test]
    fn provider_test_reports_a_closed_port() {
        let port = { let l = TcpListener::bind("127.0.0.1:0").unwrap(); l.local_addr().unwrap().port() };
        let steps = test_provider(&fake_config(port), "fake", "m", &mut |_| {});
        assert!(steps[1].text.contains("could not connect"), "{steps:?}");
        assert!(steps.iter().all(|s| !s.text.contains("sk-secret-value-9999")));
    }
}
