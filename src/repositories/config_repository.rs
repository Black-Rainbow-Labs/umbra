//! Reads Umbra's config files and merges them into one `UmbraConfig`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Map, Value};

use crate::models::config::{ProviderConfig, UmbraConfig};
use crate::utils::paths::umbra_home;

/// Loads the config: defaults, then `~/.umbra/config.toml`, then `<project>/.umbra/config.toml`,
/// then each extra file in order. Later files win, key by key.
/// @param project_root - the project the run is in
/// @param extra_files - files given with `--config`, TOML or a mini-swe-agent YAML config
pub fn load_config(project_root: &Path, extra_files: &[PathBuf]) -> Result<UmbraConfig> {
    let mut merged = serde_json::to_value(UmbraConfig::default())?;
    for path in config_file_order(project_root, extra_files) {
        if !path.exists() {
            continue;
        }
        let layer = read_config_layer(&path).with_context(|| format!("reading config {}", path.display()))?;
        merge_values(&mut merged, layer);
    }
    let mut config: UmbraConfig = serde_json::from_value(merged).context("config has a field of the wrong type")?;
    add_builtin_providers(&mut config);
    Ok(config)
}

/// Lists the config files in the order they are applied.
/// @param project_root - the project the run is in
/// @param extra_files - files given with `--config`
pub fn config_file_order(project_root: &Path, extra_files: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = vec![umbra_home().join("config.toml"), project_root.join(".umbra").join("config.toml")];
    files.extend(extra_files.iter().cloned());
    files
}

/// Reads one config file into JSON. YAML files are read as mini-swe-agent configs.
/// @param path - a `.toml`, `.yaml` or `.yml` file
fn read_config_layer(path: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(path)?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "yaml" || ext == "yml" {
        let yaml: Value = serde_norway::from_str(&text)?;
        return Ok(convert_mini_swe_agent_config(&yaml));
    }
    let toml_value: toml::Value = toml::from_str(&text)?;
    Ok(serde_json::to_value(toml_value)?)
}

/// Converts a mini-swe-agent YAML config (`agent`, `environment`, `model` sections) into
/// Umbra's field names, so its prompts and limits can be reused unchanged.
/// @param yaml - the parsed YAML file
pub fn convert_mini_swe_agent_config(yaml: &Value) -> Value {
    let mut out = Map::new();
    let mut templates = Map::new();
    let agent = yaml.get("agent").cloned().unwrap_or(Value::Null);
    let model = yaml.get("model").cloned().unwrap_or(Value::Null);
    copy_field(&agent, "system_template", &mut templates, "system");
    copy_field(&agent, "instance_template", &mut templates, "instance");
    copy_field(&model, "observation_template", &mut templates, "observation");
    copy_field(&model, "format_error_template", &mut templates, "format_error");
    copy_field(&model, "action_regex", &mut templates, "action_regex");
    for key in ["step_limit", "cost_limit", "wall_time_limit_seconds", "max_consecutive_format_errors", "mode", "confirm_exit", "whitelist_actions"] {
        copy_field(&agent, key, &mut out, key);
    }
    // mini-swe-agent's templates teach the submit command, so its configs end turns that way.
    out.insert("finish".into(), Value::String("submit".into()));
    if let Some(class) = model.get("model_class").and_then(|v| v.as_str()) {
        let format = if class.contains("textbased") { "text" } else { "tools" };
        out.insert("action_format".into(), Value::String(format.into()));
    }
    if let Some(env) = yaml.get("environment").cloned() {
        out.insert("environment".into(), convert_mini_environment(env));
    }
    if !templates.is_empty() {
        out.insert("templates".into(), Value::Object(templates));
    }
    Value::Object(out)
}

/// Maps a mini-swe-agent `environment` section onto Umbra's environment fields.
/// @param env - the YAML environment section
fn convert_mini_environment(env: Value) -> Value {
    let mut out = Map::new();
    for key in ["cwd", "timeout", "env", "image", "forward_env", "run_args"] {
        copy_field(&env, key, &mut out, key);
    }
    copy_field(&env, "interpreter", &mut out, "shell");
    copy_field(&env, "executable", &mut out, "docker_executable");
    if let Some(class) = env.get("environment_class").and_then(|v| v.as_str()) {
        out.insert("kind".into(), Value::String(class.to_string()));
    }
    Value::Object(out)
}

/// Copies one field between JSON objects when it is present.
/// @param from - the source object
/// @param key - the source field
/// @param to - the destination object
/// @param new_key - the destination field
fn copy_field(from: &Value, key: &str, to: &mut Map<String, Value>, new_key: &str) {
    if let Some(value) = from.get(key) {
        if !value.is_null() {
            to.insert(new_key.to_string(), value.clone());
        }
    }
}

/// Merges `layer` into `base`. Objects merge key by key, and every other value replaces.
/// @param base - the config so far
/// @param layer - the next file's values
pub fn merge_values(base: &mut Value, layer: Value) {
    match (base, layer) {
        (Value::Object(base_map), Value::Object(layer_map)) => {
            for (key, value) in layer_map {
                match base_map.get_mut(&key) {
                    Some(existing) => merge_values(existing, value),
                    None => {
                        base_map.insert(key, value);
                    }
                }
            }
        }
        (base_slot, value) => *base_slot = value,
    }
}

/// Adds the providers Umbra knows without any config: `anthropic`, `openai`, `openrouter`,
/// `local` (llama.cpp on port 8080) and `replay`. A provider in a config file with the same
/// name keeps its own settings and only gains the missing defaults.
/// @param config - the merged config
pub fn add_builtin_providers(config: &mut UmbraConfig) {
    for (name, builtin) in builtin_providers() {
        let entry = config.providers.entry(name.to_string()).or_default();
        fill_missing(entry, &builtin);
    }
}

/// The built in provider definitions.
fn builtin_providers() -> Vec<(&'static str, ProviderConfig)> {
    let make = |kind: &str, base_url: &str, key_env: &str| ProviderConfig { kind: kind.into(), base_url: base_url.into(), api_key_env: key_env.into(), ..Default::default() };
    vec![
        ("anthropic", make("anthropic", "https://api.anthropic.com", "ANTHROPIC_API_KEY")),
        ("openai", make("openai", "https://api.openai.com/v1", "OPENAI_API_KEY")),
        ("openrouter", make("openai", "https://openrouter.ai/api/v1", "OPENROUTER_API_KEY")),
        ("local", make("openai", "http://localhost:8080/v1", "LOCAL_API_KEY")),
        ("ollama", make("openai", "http://localhost:11434/v1", "OLLAMA_API_KEY")),
        ("replay", make("replay", "", "")),
    ]
}

/// Copies each empty field of `entry` from `builtin`.
/// @param entry - the provider from the config files
/// @param builtin - the built in defaults for it
fn fill_missing(entry: &mut ProviderConfig, builtin: &ProviderConfig) {
    if entry.kind.is_empty() {
        entry.kind = builtin.kind.clone();
    }
    if entry.base_url.is_empty() {
        entry.base_url = builtin.base_url.clone();
    }
    if entry.api_key_env.is_empty() {
        entry.api_key_env = builtin.api_key_env.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_layers_win_and_objects_merge() {
        let mut base = serde_json::json!({"a": 1, "nested": {"x": 1, "y": 2}});
        merge_values(&mut base, serde_json::json!({"a": 2, "nested": {"y": 3}}));
        assert_eq!(base, serde_json::json!({"a": 2, "nested": {"x": 1, "y": 3}}));
    }

    #[test]
    fn converts_a_mini_swe_agent_yaml_config() {
        let yaml: Value = serde_norway::from_str("agent:\n  system_template: hi\n  step_limit: 5\nmodel:\n  model_class: litellm_textbased\n  observation_template: obs\nenvironment:\n  timeout: 60\n  interpreter: [bash, -c]\n").unwrap();
        let converted = convert_mini_swe_agent_config(&yaml);
        assert_eq!(converted["templates"]["system"], "hi");
        assert_eq!(converted["templates"]["observation"], "obs");
        assert_eq!(converted["step_limit"], 5);
        assert_eq!(converted["action_format"], "text");
        assert_eq!(converted["environment"]["shell"], serde_json::json!(["bash", "-c"]));
    }
}
