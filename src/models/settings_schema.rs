//! The table behind the configuration modal: every setting that can be edited, which section it
//! is in, how it is edited and when a change takes effect. A test walks the default config and
//! fails when a key is in neither this table nor `EXCLUDED`.

/// A group of settings in the modal's left pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section { Models, Providers, Compaction, Mcp, Behavior, Display, Environment, Discovery, Images, Templates }

impl Section {
    /// Every section, in the order the modal lists them.
    pub const ALL: [Section; 10] = [Section::Models, Section::Providers, Section::Compaction, Section::Mcp, Section::Behavior, Section::Display, Section::Environment, Section::Discovery, Section::Images, Section::Templates];

    /// The name shown in the left pane.
    pub fn label(self) -> &'static str {
        match self {
            Section::Models => "Models",
            Section::Providers => "Providers",
            Section::Compaction => "Compaction",
            Section::Mcp => "MCP",
            Section::Behavior => "Behavior",
            Section::Display => "Display",
            Section::Environment => "Environment",
            Section::Discovery => "Discovery",
            Section::Images => "Images",
            Section::Templates => "Templates",
        }
    }
}

/// How a setting is edited.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldKind { Toggle, Integer { min: i64, max: i64 }, Float { min: f64, max: f64 }, Percent, Text, Secret, Choice(&'static [&'static str]), List, Map, Model, Url }

/// When a changed setting takes effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applies { Now, NextModelCall, NextSession }

/// One editable setting.
#[derive(Debug, Clone, Copy)]
pub struct SettingSpec {
    /// The config path, such as `compaction.trigger_ratio`. A `*` stands for any table name.
    pub path: &'static str,
    pub section: Section,
    pub label: &'static str,
    pub help: &'static str,
    pub kind: FieldKind,
    pub applies: Applies,
}

/// Keys that are deliberately not in the modal, with the reason. A path covers itself and every
/// key under it. The coverage test reads it.
#[cfg_attr(not(test), allow(dead_code))]
pub const EXCLUDED: &[(&str, &str)] = &[
    ("output", "set per run with --output; the default folder is right for nearly everyone"),
    ("mcp_servers", "server definitions are JSON objects; the MCP section lists them but they are edited in the file"),
    ("providers", "edited one provider at a time through the providers.* settings"),
];

use Applies::{Now, NextModelCall as Call, NextSession as Next};
use FieldKind::*;
use Section::*;

const MODES: &[&str] = &["confirm", "yolo", "human"];
const FORMATS: &[&str] = &["tools", "text"];
const FINISHES: &[&str] = &["reply", "submit"];
const DEFER: &[&str] = &["auto", "always", "never"];
const ENV_KINDS: &[&str] = &["local", "docker"];
const KINDS: &[&str] = &["anthropic", "openai", "replay"];
const AUTHS: &[&str] = &["auto", "api_key", "claude_code"];
const EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

/// Builds one `SettingSpec`; keeps the table to one line per setting.
macro_rules! s {
    ($path:expr, $section:expr, $label:expr, $help:expr, $kind:expr, $applies:expr) => {
        SettingSpec { path: $path, section: $section, label: $label, help: $help, kind: $kind, applies: $applies }
    };
}

const MODELS_AND_PROVIDERS: &[SettingSpec] = &[
    s!("model", Models, "Default model", "The provider/model used when none is chosen on the command line.", Model, Call),
    s!("models", Models, "Model list", "Extra provider/model strings offered in the model pickers.", List, Call),
    s!("models.*.provider", Models, "Provider", "The provider this named model uses.", Choice(KINDS), Call),
    s!("models.*.model", Models, "Model", "The model id sent to the provider.", Model, Call),
    s!("models.*.label", Models, "Label", "The name shown in pickers.", Text, Call),
    s!("providers.*.kind", Providers, "Kind", "The wire format: anthropic, openai (any OpenAI compatible server) or replay.", Choice(KINDS), Call),
    s!("providers.*.base_url", Providers, "Base URL", "The server address, such as http://localhost:8080/v1.", Url, Call),
    s!("providers.*.auth", Providers, "Auth", "Anthropic only: auto, api_key or claude_code.", Choice(AUTHS), Call),
    s!("providers.*.api_key_env", Providers, "API key variable", "The environment variable that holds the key. Preferred over api_key.", Text, Call),
    s!("providers.*.api_key", Providers, "API key", "The key itself, saved as plain text in the config file.", Secret, Call),
    s!("providers.*.headers", Providers, "Headers", "Extra request headers as name=value pairs.", Map, Call),
    s!("providers.*.max_tokens", Providers, "Max tokens", "The most tokens a reply may have.", Integer { min: 1, max: 1_000_000 }, Call),
    s!("providers.*.temperature", Providers, "Temperature", "Only sent when set. Current Claude models reject any value but the default.", Float { min: 0.0, max: 2.0 }, Call),
    s!("providers.*.effort", Providers, "Effort", "Anthropic only: output_config.effort.", Choice(EFFORTS), Call),
    s!("providers.*.input_cost_per_mtok", Providers, "Input cost", "Dollars per million input tokens, for models not in the price table.", Float { min: 0.0, max: 100_000.0 }, Call),
    s!("providers.*.output_cost_per_mtok", Providers, "Output cost", "Dollars per million output tokens, for models not in the price table.", Float { min: 0.0, max: 100_000.0 }, Call),
    s!("providers.*.extra_body", Providers, "Extra body", "Extra JSON fields merged into every request, as name=value pairs.", Map, Call),
    s!("providers.*.replay_file", Providers, "Replay file", "Replay only: a JSON file of scripted replies.", Text, Call),
    s!("providers.*.max_retries", Providers, "Max retries", "How many times a failed call is tried again.", Integer { min: 0, max: 100 }, Call),
    s!("providers.*.context_window", Providers, "Context window", "Tokens. The model's window, used instead of asking the server.", Integer { min: 0, max: 100_000_000 }, Call),
    s!("providers.*.stream", Providers, "Stream", "OpenAI compatible servers: stream the reply so Ctrl+C stops generation.", Toggle, Call),
    s!("providers.*.send_reasoning", Providers, "Send reasoning back", "OpenAI compatible servers: send each reply's reasoning text back, so the server's prompt cache covers the whole history.", Toggle, Call),
    s!("providers.*.preserve_thinking", Providers, "Preserve thinking", "OpenAI compatible servers: ask the chat template to keep earlier turns' reasoning. On by default for a server on this machine.", Toggle, Call),
];

const COMPACTION_AND_MCP: &[SettingSpec] = &[
    s!("compaction.enabled", Compaction, "Enabled", "Compact the history when it nears the context window.", Toggle, Now),
    s!("compaction.context_window", Compaction, "Context window", "Tokens. 0 asks the provider, then a built in table.", Integer { min: 0, max: 100_000_000 }, Now),
    s!("compaction.trigger_ratio", Compaction, "Trigger", "Compact when the request passes this share of the window.", Percent, Now),
    s!("compaction.target_ratio", Compaction, "Target", "A mask pass that leaves more than this share is followed by a summary.", Percent, Now),
    s!("compaction.protect_recent_ratio", Compaction, "Protect recent", "The newest outputs worth this share of the window are never masked.", Percent, Now),
    s!("compaction.protect_recent_max", Compaction, "Protect recent max", "Tokens. The protected outputs are never more than this.", Integer { min: 0, max: 100_000_000 }, Now),
    s!("compaction.min_reclaim_ratio", Compaction, "Minimum reclaim", "A mask pass must free this share of the window or it masks nothing.", Percent, Now),
    s!("compaction.min_reclaim_max", Compaction, "Minimum reclaim max", "Tokens. The minimum to free is never more than this.", Integer { min: 0, max: 100_000_000 }, Now),
    s!("compaction.keep_recent_ratio", Compaction, "Keep recent", "The newest turns worth this share of the window are kept word for word.", Percent, Now),
    s!("compaction.mask_min_chars", Compaction, "Mask minimum", "Outputs shorter than this many characters are never masked.", Integer { min: 0, max: 10_000_000 }, Now),
    s!("compaction.max_context_tokens", Compaction, "Working window cap", "Tokens. Compact as if the window were at most this size, so a long session stays fast. 0 is no cap.", Integer { min: 0, max: 100_000_000 }, Now),
    s!("mcp.enabled", Mcp, "Enabled", "Start MCP servers.", Toggle, Next),
    s!("mcp.only", Mcp, "Only", "Start only these servers. Empty means every server found.", List, Next),
    s!("mcp.disabled", Mcp, "Disabled", "Never start these servers.", List, Next),
    s!("mcp.startup_timeout_seconds", Mcp, "Startup timeout", "Seconds to wait for a server to start.", Integer { min: 1, max: 3600 }, Next),
    s!("mcp.call_timeout_seconds", Mcp, "Call timeout", "Seconds to wait for a tool call.", Integer { min: 1, max: 86_400 }, Next),
    s!("mcp.defer_tools", Mcp, "Defer tools", "Send MCP tools by name only, behind tool_search.", Choice(DEFER), Next),
    s!("mcp.defer_ratio", Mcp, "Defer ratio", "The share of the context window the definitions may take before auto defers them.", Percent, Next),
];

const BEHAVIOR_AND_DISPLAY: &[SettingSpec] = &[
    s!("mode", Behavior, "Mode", "confirm asks before actions, yolo runs everything, human types the commands.", Choice(MODES), Now),
    s!("step_limit", Behavior, "Step limit", "Stop after this many steps. 0 means no limit.", Integer { min: 0, max: 1_000_000 }, Now),
    s!("cost_limit", Behavior, "Cost limit", "Stop after this many dollars. 0 means no limit.", Float { min: 0.0, max: 1_000_000.0 }, Now),
    s!("wall_time_limit_seconds", Behavior, "Time limit", "Stop after this many seconds. 0 means no limit.", Integer { min: 0, max: 31_536_000 }, Now),
    s!("max_consecutive_format_errors", Behavior, "Format error limit", "Stop after this many replies in a row that cannot be parsed.", Integer { min: 1, max: 1000 }, Now),
    s!("edit_tool", Behavior, "Edit tool", "Offer an edit tool beside bash: an exact text replacement that keeps line endings and encoding.", Toggle, Next),
    s!("finish_check", Behavior, "Finish check", "When the agent replies right after changing files, ask it once to run the check that proves the change.", Toggle, Next),
    s!("confirm_exit", Behavior, "Confirm exit", "Ask for a follow up task when the agent submits.", Toggle, Now),
    s!("keep_open", Behavior, "Keep open", "An empty line at a prompt asks again instead of ending the run.", Toggle, Now),
    s!("whitelist_actions", Behavior, "Whitelist", "Regexes. A command that starts with a match runs without asking.", List, Now),
    s!("action_format", Behavior, "Action format", "tools uses native tool calls; text uses one fenced code block per reply.", Choice(FORMATS), Next),
    s!("finish", Behavior, "Finish", "reply ends a turn on a reply with no command; submit needs the submit command.", Choice(FINISHES), Next),
    s!("fullscreen", Display, "Fullscreen", "Open on the alternate screen with a scrollable transcript.", Toggle, Next),
    s!("banner", Display, "Banner", "Draw the UMBRA banner when a run starts.", Toggle, Now),
    s!("status_bar", Display, "Status bar", "Draw the status bar under the output.", Toggle, Now),
    s!("verbose", Display, "Verbose", "Print every detail of the run.", Toggle, Now),
    s!("mouse", Display, "Mouse", "Mouse reporting in the fullscreen view. Off, the terminal selects text itself.", Toggle, Next),
    s!("sessions.auto_title", Display, "Automatic titles", "Ask the model for a session title and summary after the first turn.", Toggle, Now),
    s!("usage.enabled", Display, "Usage bar", "Show the weekly usage bar for a Claude login.", Toggle, Now),
];

const ENVIRONMENT_AND_DISCOVERY: &[SettingSpec] = &[
    s!("environment.kind", Environment, "Kind", "Where commands run: local or docker.", Choice(ENV_KINDS), Next),
    s!("environment.cwd", Environment, "Working directory", "Empty means the folder Umbra started in (local) or / (docker).", Text, Next),
    s!("environment.timeout", Environment, "Timeout", "Seconds before a command is killed.", Integer { min: 1, max: 86_400 }, Next),
    s!("environment.shell", Environment, "Shell", "The program and flags that run a command string. Empty means bash.", List, Next),
    s!("environment.env", Environment, "Variables", "Extra environment variables for every command, as name=value pairs.", Map, Next),
    s!("environment.image", Environment, "Docker image", "The image for docker.", Text, Next),
    s!("environment.docker_executable", Environment, "Docker program", "The docker command to run.", Text, Next),
    s!("environment.run_args", Environment, "Docker run args", "Arguments passed to docker run.", List, Next),
    s!("environment.forward_env", Environment, "Forwarded variables", "Host variables copied into the container when set.", List, Next),
    s!("environment.max_timeout", Environment, "Maximum timeout", "The most seconds the model may give one command with the bash timeout argument.", Integer { min: 1, max: 86_400 }, Next),
    s!("sources.*", Discovery, "Source", "Read the files of this other harness.", Toggle, Next),
    s!("instructions.enabled", Discovery, "Instructions", "Read CLAUDE.md, AGENTS.md and similar files.", Toggle, Next),
    s!("instructions.max_bytes", Discovery, "Instructions budget", "Stop adding instruction files after this many bytes.", Integer { min: 0, max: 1_000_000_000 }, Next),
    s!("instructions.extra_files", Discovery, "Extra instruction files", "More files to always include.", List, Next),
    s!("skills.enabled", Discovery, "Skills", "Load Agent Skills.", Toggle, Next),
    s!("skills.extra_paths", Discovery, "Extra skill folders", "More folders that hold skill folders.", List, Next),
    s!("skills.disabled", Discovery, "Disabled skills", "Skill names to leave out.", List, Next),
    s!("commands.enabled", Discovery, "Commands", "Load custom slash commands.", Toggle, Next),
];

const IMAGES_AND_TEMPLATES: &[SettingSpec] = &[
    s!("images.max_long_edge", Images, "Long edge", "Pixels. Larger images are scaled down. 0 never scales.", Integer { min: 0, max: 100_000 }, Now),
    s!("images.max_bytes", Images, "Largest image", "Bytes of base64 text. Larger images are refused.", Integer { min: 0, max: 2_000_000_000 }, Now),
    s!("images.detect_paths", Images, "Detect paths", "Attach image files named in a task or comment.", Toggle, Now),
    s!("templates.*", Templates, "Template", "A template file or text. Empty means the built in template.", Text, Next),
];

/// Every editable setting, in the order the modal lists them.
pub fn settings() -> &'static [SettingSpec] {
    static ALL: std::sync::OnceLock<Vec<SettingSpec>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| [MODELS_AND_PROVIDERS, COMPACTION_AND_MCP, BEHAVIOR_AND_DISPLAY, ENVIRONMENT_AND_DISCOVERY, IMAGES_AND_TEMPLATES].concat())
}

/// Returns true when a spec path, which may contain `*` segments, matches a concrete path.
/// @param pattern - the spec path, such as `providers.*.base_url`
/// @param path - a concrete path, such as `providers.local.base_url`
fn path_matches(pattern: &str, path: &str) -> bool {
    let (mut a, mut b) = (pattern.split('.'), path.split('.'));
    loop {
        match (a.next(), b.next()) {
            (None, None) => return true,
            (Some("*"), Some(_)) => {}
            (Some(x), Some(y)) if x == y => {}
            _ => return false,
        }
    }
}

/// Finds the spec for a concrete path, matching `*` against any table name.
/// @param path - the config path, such as `providers.local.base_url`
pub fn spec_for(path: &str) -> Option<&'static SettingSpec> {
    settings().iter().find(|spec| spec.path == path).or_else(|| settings().iter().find(|spec| spec.path.contains('*') && path_matches(spec.path, path)))
}

/// Lists the settings of one section, in table order.
/// @param section - the section to list
pub fn in_section(section: Section) -> Vec<&'static SettingSpec> {
    settings().iter().filter(|spec| spec.section == section).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::config::UmbraConfig;

    /// Collects every leaf path of a TOML value. Tables recurse, except `environment.env`;
    /// arrays and empty tables count as one leaf.
    fn leaf_paths(prefix: &str, value: &toml::Value, out: &mut Vec<String>) {
        match value {
            toml::Value::Table(table) if !table.is_empty() && prefix != "environment.env" => {
                for (key, child) in table {
                    let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
                    leaf_paths(&path, child, out);
                }
            }
            _ => out.push(prefix.to_string()),
        }
    }

    /// Returns every leaf path of the default config.
    fn default_paths() -> Vec<String> {
        let value = toml::Value::try_from(UmbraConfig::default()).unwrap();
        let mut paths = Vec::new();
        leaf_paths("", &value, &mut paths);
        paths
    }

    /// Returns true when a path is excluded, directly or through a parent.
    fn is_excluded(path: &str) -> bool {
        EXCLUDED.iter().any(|(prefix, _)| path == *prefix || path.starts_with(&format!("{prefix}.")))
    }

    /// Fails when a default config key has no spec and no exclusion.
    #[test]
    fn every_config_key_has_a_spec_or_an_exclusion() {
        let missing: Vec<String> = default_paths().into_iter().filter(|p| spec_for(p).is_none() && !is_excluded(p)).collect();
        assert!(missing.is_empty(), "config keys with no SettingSpec and no EXCLUDED entry: {missing:?}");
    }

    /// Fails when a spec names a path that is not in the config (a typo).
    #[test]
    fn every_spec_path_is_a_real_config_path() {
        let known = default_paths();
        let bad: Vec<&str> = settings().iter().map(|s| s.path).filter(|p| !p.contains('*') && !known.iter().any(|k| k == p)).collect();
        assert!(bad.is_empty(), "spec paths that are not config keys: {bad:?}");
    }

    /// Wildcard paths resolve and sections list their settings.
    #[test]
    fn wildcards_and_sections_resolve() {
        assert_eq!(spec_for("providers.local.base_url").unwrap().kind, FieldKind::Url);
        assert!(spec_for("providers.local.nothing").is_none());
        assert!(in_section(Section::Compaction).iter().any(|s| s.path == "compaction.trigger_ratio"));
        assert_eq!(Section::ALL.len(), 10);
    }
}
