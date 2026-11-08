//! Umbra's settings. Every field has a default, so an empty config file is a valid config.
//! The files are merged in order: built in defaults, `~/.umbra/config.toml`,
//! `<project>/.umbra/config.toml`, each `--config` file, then command line flags.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How the model asks for actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionFormat {
    /// Native tool calling with a `bash` tool and one tool per MCP tool. The mini-swe-agent v2 default.
    Tools,
    /// Plain text with exactly one fenced code block per reply. Works with any model.
    Text,
}

/// Whether a person approves each action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Ask before every action that is not on the whitelist.
    Confirm,
    /// Run every action without asking.
    Yolo,
    /// The person types the commands and the model is not called.
    Human,
}

/// The top level config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UmbraConfig {
    /// `provider/model`, for example `anthropic/claude-haiku-5-5` or `local/qwen`.
    pub model: String,
    pub mode: Mode,
    pub action_format: ActionFormat,
    pub step_limit: u32,
    pub cost_limit: f64,
    pub wall_time_limit_seconds: u64,
    pub max_consecutive_format_errors: u32,
    /// Ask for a follow up task when the agent submits, instead of exiting.
    pub confirm_exit: bool,
    /// Regexes. A command that matches one at its start runs without confirmation.
    pub whitelist_actions: Vec<String>,
    /// Where the trajectory JSON is written. Empty means `~/.umbra/trajectories/<time>.traj.json`.
    pub output: String,
    pub environment: EnvironmentConfig,
    pub providers: BTreeMap<String, ProviderConfig>,
    pub sources: SourcesConfig,
    pub instructions: InstructionsConfig,
    pub skills: SkillsConfig,
    pub commands: CommandsConfig,
    pub mcp: McpConfig,
    /// MCP servers defined in Umbra's own config, in the same shape as Claude Code's `mcpServers`.
    pub mcp_servers: BTreeMap<String, Value>,
    pub templates: TemplatesConfig,
}

impl Default for UmbraConfig {
    /// The settings used when no config file says otherwise.
    fn default() -> Self {
        UmbraConfig {
            model: String::new(),
            mode: Mode::Confirm,
            action_format: ActionFormat::Tools,
            step_limit: 0,
            cost_limit: 3.0,
            wall_time_limit_seconds: 0,
            max_consecutive_format_errors: 3,
            confirm_exit: true,
            whitelist_actions: Vec::new(),
            output: String::new(),
            environment: EnvironmentConfig::default(),
            providers: BTreeMap::new(),
            sources: SourcesConfig::default(),
            instructions: InstructionsConfig::default(),
            skills: SkillsConfig::default(),
            commands: CommandsConfig::default(),
            mcp: McpConfig::default(),
            mcp_servers: BTreeMap::new(),
            templates: TemplatesConfig::default(),
        }
    }
}

/// Where commands run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentConfig {
    /// `local` or `docker`.
    pub kind: String,
    /// Working directory. Empty means the directory Umbra was started in (local) or `/` (docker).
    pub cwd: String,
    /// Seconds before a command is killed.
    pub timeout: u64,
    /// The program and flags that run a command string. Empty means bash, found automatically.
    pub shell: Vec<String>,
    /// Extra environment variables for every command.
    pub env: BTreeMap<String, String>,
    pub image: String,
    pub docker_executable: String,
    pub run_args: Vec<String>,
    /// Host environment variables copied into the container when they are set.
    pub forward_env: Vec<String>,
}

impl Default for EnvironmentConfig {
    /// The mini-swe-agent defaults: a 30 second timeout and pagers turned off.
    fn default() -> Self {
        let env = [("PAGER", "cat"), ("MANPAGER", "cat"), ("LESS", "-R"), ("PIP_PROGRESS_BAR", "off"), ("TQDM_DISABLE", "1"), ("GIT_PAGER", "cat")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        EnvironmentConfig {
            kind: "local".into(),
            cwd: String::new(),
            timeout: 30,
            shell: Vec::new(),
            env,
            image: String::new(),
            docker_executable: "docker".into(),
            run_args: vec!["--rm".into()],
            forward_env: Vec::new(),
        }
    }
}

/// One model provider. `kind` picks the wire format.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    /// `anthropic`, `openai` (any OpenAI compatible server, including llama.cpp) or `replay`.
    pub kind: String,
    pub base_url: String,
    /// Read the API key from this environment variable.
    pub api_key_env: String,
    /// The API key itself. Prefer `api_key_env` so the key is not stored in a file.
    pub api_key: String,
    /// Anthropic only: `auto` (API key, then Claude Code's login), `api_key`, or `claude_code`.
    pub auth: String,
    pub headers: BTreeMap<String, String>,
    pub max_tokens: Option<u32>,
    /// Only sent when set. Current Claude models reject any value other than the default.
    pub temperature: Option<f64>,
    /// Anthropic only: `output_config.effort` (`low`, `medium`, `high`, `xhigh`, `max`).
    pub effort: Option<String>,
    /// Dollars per million input tokens. Used when the model is not in the built in price table.
    pub input_cost_per_mtok: Option<f64>,
    pub output_cost_per_mtok: Option<f64>,
    /// Extra JSON fields merged into every request body.
    pub extra_body: BTreeMap<String, Value>,
    /// Replay only: a JSON file holding a list of scripted replies.
    pub replay_file: String,
    pub max_retries: Option<u32>,
}

/// Which other harnesses' files Umbra reads. All are on by default.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SourcesConfig {
    pub umbra: bool,
    pub claude: bool,
    pub codex: bool,
    pub gemini: bool,
    pub opencode: bool,
    pub cursor: bool,
    pub copilot: bool,
    pub windsurf: bool,
    pub cline: bool,
    /// The shared `AGENTS.md` and `.agents/skills` conventions.
    pub agents: bool,
    pub vscode: bool,
    pub claude_desktop: bool,
}

impl Default for SourcesConfig {
    /// Turns every source on.
    fn default() -> Self {
        SourcesConfig { umbra: true, claude: true, codex: true, gemini: true, opencode: true, cursor: true, copilot: true, windsurf: true, cline: true, agents: true, vscode: true, claude_desktop: true }
    }
}

/// Instruction files such as CLAUDE.md and AGENTS.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InstructionsConfig {
    pub enabled: bool,
    /// Stop adding files once this many bytes have been added.
    pub max_bytes: usize,
    /// More files to always include.
    pub extra_files: Vec<String>,
}

impl Default for InstructionsConfig {
    /// Instructions on, with a 256 KB budget.
    fn default() -> Self {
        InstructionsConfig { enabled: true, max_bytes: 256 * 1024, extra_files: Vec::new() }
    }
}

/// Agent Skills (folders holding a SKILL.md).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillsConfig {
    pub enabled: bool,
    /// More folders to search. Each holds skill folders.
    pub extra_paths: Vec<String>,
    /// Skill names to leave out.
    pub disabled: Vec<String>,
}

impl Default for SkillsConfig {
    /// Skills on.
    fn default() -> Self {
        SkillsConfig { enabled: true, extra_paths: Vec::new(), disabled: Vec::new() }
    }
}

/// Custom slash commands such as `.claude/commands/*.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandsConfig {
    pub enabled: bool,
}

impl Default for CommandsConfig {
    /// Commands on.
    fn default() -> Self {
        CommandsConfig { enabled: true }
    }
}

/// MCP client settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct McpConfig {
    pub enabled: bool,
    /// Only start these servers. Empty means every server found.
    pub only: Vec<String>,
    /// Never start these servers.
    pub disabled: Vec<String>,
    pub startup_timeout_seconds: u64,
    pub call_timeout_seconds: u64,
}

impl Default for McpConfig {
    /// MCP on, every server, 30 second startup and 120 second calls.
    fn default() -> Self {
        McpConfig { enabled: true, only: Vec::new(), disabled: Vec::new(), startup_timeout_seconds: 30, call_timeout_seconds: 120 }
    }
}

/// Jinja templates. An empty string means the built in template for the action format.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TemplatesConfig {
    pub system: String,
    pub instance: String,
    pub observation: String,
    pub format_error: String,
    /// Text mode only: the regex that finds the command. It must have one capture group.
    pub action_regex: String,
}
