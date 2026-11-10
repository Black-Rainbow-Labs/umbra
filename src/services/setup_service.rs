//! `umbra setup`: choose a model, save it in `~/.umbra/config.toml`, and check that it answers.
//! Editors run it as `umbra acp --setup` for the "Set up Umbra in a terminal" sign in method.

use std::path::Path;

use anyhow::{bail, Context, Result};

use super::providers::{check_credentials, create_provider};
use super::run_service::prepare_config;
use crate::cli::{CommonArgs, SetupArgs};
use crate::models::message::Message;
use crate::utils::console::read_line;
use crate::utils::paths::umbra_home;

/// One provider the wizard offers.
struct Choice {
    provider: &'static str,
    label: &'static str,
    default_model: &'static str,
    /// The environment variable that holds its key, empty when it needs none.
    key_env: &'static str,
}

/// The providers, in the order they are offered.
const CHOICES: &[Choice] = &[
    Choice { provider: "anthropic", label: "Anthropic Claude (an API key, or your Claude Code login)", default_model: "claude-sonnet-5-5", key_env: "ANTHROPIC_API_KEY" },
    Choice { provider: "local", label: "A local llama.cpp server on http://localhost:8080", default_model: "local-model", key_env: "" },
    Choice { provider: "ollama", label: "Ollama on http://localhost:11434", default_model: "llama3.3", key_env: "" },
    Choice { provider: "openai", label: "OpenAI (an API key)", default_model: "gpt-5", key_env: "OPENAI_API_KEY" },
    Choice { provider: "openrouter", label: "OpenRouter (an API key)", default_model: "anthropic/claude-sonnet-5-5", key_env: "OPENROUTER_API_KEY" },
];

/// Runs the wizard and returns the exit code: 0 when the model is saved and answered.
/// @param args - the setup options
pub fn run_setup(args: &SetupArgs) -> Result<i32> {
    let model = match &args.model {
        Some(model) => model.clone(),
        None => ask_model()?,
    };
    let config_path = umbra_home().join("config.toml");
    let provider = model.split_once('/').map(|(p, _)| p).unwrap_or("anthropic").to_string();
    let key = ask_for_key(&provider, args.model.is_some())?;
    save_choice(&config_path, &model, &provider, key.as_deref())?;
    println!("Saved model = \"{model}\" in {}", config_path.display());
    let (config, _) = prepare_config(&CommonArgs::default())?;
    if let Err(error) = check_credentials(&config, &model) {
        println!("Umbra cannot use {model} yet: {error:#}");
        return Ok(1);
    }
    if args.no_test {
        return Ok(0);
    }
    println!("Asking {model} to say hello...");
    let mut provider = create_provider(&config, &model)?;
    match provider.complete(&[Message::system("Answer in one short sentence."), Message::user("Say hello.")], &[]) {
        Ok(reply) => {
            println!("{model} answered: {}", reply.content.trim().lines().next().unwrap_or(""));
            Ok(0)
        }
        Err(error) => {
            println!("{model} did not answer: {}", error.message.lines().next().unwrap_or(""));
            Ok(1)
        }
    }
}

/// Asks which provider and which model, with a default model for each provider.
fn ask_model() -> Result<String> {
    println!("Which model should Umbra use?");
    for (number, choice) in CHOICES.iter().enumerate() {
        println!("  {}. {}", number + 1, choice.label);
    }
    let answer = read_line("Number [1]: ").context("setup needs a terminal to ask in. Pass --model provider/model instead")?;
    let index = match answer.trim() {
        "" => 0,
        text => text.parse::<usize>().ok().filter(|n| (1..=CHOICES.len()).contains(n)).map(|n| n - 1).context("not one of the numbers")?,
    };
    let choice = &CHOICES[index];
    let name = read_line(&format!("Model name [{}]: ", choice.default_model)).unwrap_or_default();
    let name = if name.trim().is_empty() { choice.default_model.to_string() } else { name.trim().to_string() };
    Ok(format!("{}/{name}", choice.provider))
}

/// For a provider that needs a key and has none in its environment variable, offers to save one.
/// Returns the key to save, if any.
/// @param provider - the provider name
/// @param quiet - true when the model came from the command line, so nothing is asked
fn ask_for_key(provider: &str, quiet: bool) -> Result<Option<String>> {
    let Some(choice) = CHOICES.iter().find(|c| c.provider == provider) else { return Ok(None) };
    if choice.key_env.is_empty() || std::env::var(choice.key_env).is_ok_and(|v| !v.is_empty()) || quiet {
        return Ok(None);
    }
    if provider == "anthropic" && claude_code_login_exists() {
        println!("Found your Claude Code login; Umbra will use it. Set ANTHROPIC_API_KEY to use an API key instead.");
        return Ok(None);
    }
    println!("{} is not set. Umbra reads the key from it. You can set it in your shell profile,", choice.key_env);
    let key = read_line("or paste the key here to save it in ~/.umbra/config.toml (Enter to skip): ").unwrap_or_default();
    Ok((!key.trim().is_empty()).then(|| key.trim().to_string()))
}

/// Returns true when Claude Code's saved login exists, in its file or the macOS Keychain.
fn claude_code_login_exists() -> bool {
    super::providers::credentials::claude_code_login_exists()
}

/// Writes `model`, and the key when given, into the config file, keeping everything else in it.
/// @param path - `~/.umbra/config.toml`
/// @param model - the `provider/model` string
/// @param provider - the provider name, for the key's table
/// @param key - the API key to save, if any
fn save_choice(path: &Path, model: &str, provider: &str, key: Option<&str>) -> Result<()> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut table: toml::Table = text.parse().with_context(|| format!("{} is not valid TOML, so it was left alone", path.display()))?;
    table.insert("model".into(), toml::Value::String(model.into()));
    if let Some(key) = key {
        let providers = table.entry("providers").or_insert_with(|| toml::Value::Table(Default::default()));
        let Some(providers) = providers.as_table_mut() else { bail!("providers in {} is not a table", path.display()) };
        let entry = providers.entry(provider).or_insert_with(|| toml::Value::Table(Default::default()));
        let Some(entry) = entry.as_table_mut() else { bail!("providers.{provider} in {} is not a table", path.display()) };
        entry.insert("api_key".into(), toml::Value::String(key.into()));
    }
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, toml::to_string_pretty(&table)?).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_keeps_the_rest_of_the_config() {
        let folder = std::env::temp_dir().join(format!("umbra-setup-test-{}", std::process::id()));
        let path = folder.join("config.toml");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(&path, "step_limit = 7\n[providers.openai]\nbase_url = \"http://x\"\n").unwrap();
        save_choice(&path, "openai/gpt-5", "openai", Some("sk-test")).unwrap();
        let saved: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        assert_eq!(saved["model"].as_str(), Some("openai/gpt-5"));
        assert_eq!(saved["step_limit"].as_integer(), Some(7));
        assert_eq!(saved["providers"]["openai"]["base_url"].as_str(), Some("http://x"));
        assert_eq!(saved["providers"]["openai"]["api_key"].as_str(), Some("sk-test"));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
