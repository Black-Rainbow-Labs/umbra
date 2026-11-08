//! Renders the Jinja templates that make up every message Umbra writes to the model. The
//! defaults are mini-swe-agent's `mini.yaml` (tool mode) and `mini_textbased.yaml` (text mode)
//! prompts, with sections added for instructions, skills and MCP tools.

use anyhow::{Context, Result};
use minijinja::{Environment, UndefinedBehavior};
use serde_json::{json, Value};

use crate::models::action::Observation;
use crate::models::config::{ActionFormat, TemplatesConfig};

const SYSTEM: &str = include_str!("../templates/system.jinja");
const INSTANCE_TOOLS: &str = include_str!("../templates/instance_tools.jinja");
const INSTANCE_TEXT: &str = include_str!("../templates/instance_text.jinja");
const OBSERVATION_TOOLS: &str = include_str!("../templates/observation_tools.jinja");
const OBSERVATION_TEXT: &str = include_str!("../templates/observation_text.jinja");
const FORMAT_ERROR_TOOLS: &str = include_str!("../templates/format_error_tools.jinja");
const FORMAT_ERROR_TEXT: &str = include_str!("../templates/format_error_text.jinja");
const COMPACT_SUMMARY: &str = include_str!("../templates/compact_summary.jinja");
const COMPACT_RESUME: &str = include_str!("../templates/compact_resume.jinja");

/// The regex that finds a bash command in text mode.
pub const DEFAULT_BASH_REGEX: &str = r"(?s)```umbra_bash\s*\n(.*?)\n```";
/// The regex that finds an MCP call in text mode.
pub const DEFAULT_MCP_REGEX: &str = r"(?s)```umbra_mcp\s*\n(.*?)\n```";

/// The templates in use for a run.
#[derive(Debug, Clone)]
pub struct Templates {
    pub system: String,
    pub instance: String,
    pub observation: String,
    pub format_error: String,
    pub action_regex: String,
    /// The request for a summary when the history is compacted.
    pub compact_summary: String,
    /// The message that carries the summary into the compacted history.
    pub compact_resume: String,
}

impl Templates {
    /// Picks each template from the config, or the built in one for the action format.
    /// @param config - the template overrides
    /// @param format - tools or text
    pub fn resolve(config: &TemplatesConfig, format: ActionFormat) -> Self {
        let text = format == ActionFormat::Text;
        let pick = |custom: &str, tools: &str, texts: &str| if custom.is_empty() { (if text { texts } else { tools }).to_string() } else { custom.to_string() };
        Templates {
            system: pick(&config.system, SYSTEM, SYSTEM),
            instance: pick(&config.instance, INSTANCE_TOOLS, INSTANCE_TEXT),
            observation: pick(&config.observation, OBSERVATION_TOOLS, OBSERVATION_TEXT),
            format_error: pick(&config.format_error, FORMAT_ERROR_TOOLS, FORMAT_ERROR_TEXT),
            action_regex: if config.action_regex.is_empty() { DEFAULT_BASH_REGEX.to_string() } else { config.action_regex.clone() },
            compact_summary: pick(&config.compact_summary, COMPACT_SUMMARY, COMPACT_SUMMARY),
            compact_resume: pick(&config.compact_resume, COMPACT_RESUME, COMPACT_RESUME),
        }
    }
}

/// Renders a template with strict undefined handling, so a misspelled variable is an error
/// instead of silently empty text, as in mini-swe-agent.
/// @param template - the template text
/// @param vars - the variables, as a JSON object
pub fn render(template: &str, vars: &Value) -> Result<String> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.add_template("t", template).context("template has a syntax error")?;
    let rendered = env.get_template("t")?.render(minijinja::Value::from(minijinja::value::Serde(vars))).context("template could not be rendered")?;
    Ok(rendered)
}

/// Renders the observation for one action.
/// @param template - the observation template
/// @param vars - the run's template variables
/// @param observation - the action's result
pub fn render_observation(template: &str, vars: &Value, observation: &Observation) -> Result<String> {
    let mut all = vars.clone();
    all["output"] = json!(observation);
    render(template, &all)
}

/// Renders a format error. Only the error fields are available, as in mini-swe-agent.
/// @param template - the format error template
/// @param error - what was wrong
/// @param finish_reason - why the model stopped
/// @param has_tool_calls - whether the reply had any tool calls
pub fn render_format_error(template: &str, error: &str, finish_reason: &str, has_tool_calls: bool) -> Result<String> {
    render(template, &json!({ "error": error, "actions": [], "finish_reason": finish_reason, "has_tool_calls": has_tool_calls }))
}

/// Facts about the machine for the prompt: `system` is `Windows`, `Linux` or `Darwin` (the
/// names Python's `platform.uname()` uses, which mini-swe-agent templates test for).
pub fn platform_vars() -> Value {
    let system = match std::env::consts::OS { "windows" => "Windows", "macos" => "Darwin", "linux" => "Linux", other => other };
    json!({ "system": system, "release": "", "version": "", "machine": std::env::consts::ARCH, "node": "" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_long_output_in_both_formats() {
        let long = Observation { output: "x".repeat(12_000), returncode: 0, exception_info: String::new() };
        let text = render_observation(OBSERVATION_TEXT, &json!({}), &long).unwrap();
        assert!(text.contains("2000 characters elided"));
        let tools = render_observation(OBSERVATION_TOOLS, &json!({}), &long).unwrap();
        let parsed: Value = serde_json::from_str(&tools).unwrap();
        assert_eq!(parsed["elided_chars"], 2000);
    }

    #[test]
    fn renders_the_length_branch_of_the_format_error() {
        let message = render_format_error(FORMAT_ERROR_TOOLS, "No tool calls", "length", false).unwrap();
        assert!(message.contains("output token limit"));
        let message = render_format_error(FORMAT_ERROR_TEXT, "Expected exactly 1 action, found 2.", "stop", false).unwrap();
        assert!(message.contains("Expected exactly 1 action"));
    }

    #[test]
    fn strict_mode_rejects_unknown_variables() {
        assert!(render("{{ nope }}", &json!({})).is_err());
    }
}
