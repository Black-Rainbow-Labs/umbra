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

/// How a turn ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Finish {
    /// A reply with no command ends the turn and goes to the person, as in Claude Code.
    Reply,
    /// Every reply needs a command, and a command that prints COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT
    /// ends the run. The mini-swe-agent behavior, for benchmark runs.
    Submit,
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
    /// Named models (`[models.opus]` tables) and the older list of `provider/model` strings, both
    /// offered in the model pickers after `model`.
    pub models: ModelsConfig,
    pub mode: Mode,
    pub action_format: ActionFormat,
    /// How a turn ends: `reply` (a reply with no command) or `submit` (the submit command).
    pub finish: Finish,
    pub step_limit: u32,
    pub cost_limit: f64,
    pub wall_time_limit_seconds: u64,
    pub max_consecutive_format_errors: u32,
    /// When the model replies to end its turn right after a command that changed files, with no
    /// command run since, send it one message asking it to run the check that proves the change
    /// and compare the result with the request. Once per turn.
    pub finish_check: bool,
    /// Offer the `edit` tool beside `bash` in tool mode: an exact text replacement that keeps the
    /// file's line endings and encoding.
    pub edit_tool: bool,
    /// Ask for a follow up task when the agent submits, instead of exiting.
    pub confirm_exit: bool,
    /// An empty line at the task prompt or the follow up prompt asks again instead of ending the run.
    pub keep_open: bool,
    /// Draw the status bar under the output in a terminal: context use, room before compaction
    /// and the compaction count.
    pub status_bar: bool,
    /// Draw the UMBRA banner with the version, model and folder when a run starts in a terminal.
    pub banner: bool,
    /// Open on the terminal's alternate screen with a scrollable transcript, as Claude Code's
    /// fullscreen mode does. Only in an interactive terminal.
    pub fullscreen: bool,
    /// Turn on mouse reporting in the fullscreen view, so clicks and drags reach Umbra. Off, the
    /// terminal selects text itself and everything is done with the keyboard.
    pub mouse: bool,
    pub sessions: SessionsConfig,
    pub usage: UsageConfig,
    /// Print every detail of the run: step headings with the cost, whole command outputs, full
    /// format errors and every compaction attempt. Off, routine events take one line each.
    pub verbose: bool,
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
    pub compaction: CompactionConfig,
    pub images: ImagesConfig,
}

impl Default for UmbraConfig {
    /// The settings used when no config file says otherwise.
    fn default() -> Self {
        UmbraConfig {
            model: String::new(),
            models: ModelsConfig::default(),
            mode: Mode::Confirm,
            action_format: ActionFormat::Tools,
            finish: Finish::Reply,
            step_limit: 0,
            cost_limit: 3.0,
            wall_time_limit_seconds: 0,
            max_consecutive_format_errors: 3,
            finish_check: true,
            edit_tool: false,
            confirm_exit: true,
            keep_open: false,
            status_bar: true,
            banner: true,
            fullscreen: true,
            mouse: true,
            sessions: SessionsConfig::default(),
            usage: UsageConfig::default(),
            verbose: false,
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
            compaction: CompactionConfig::default(),
            images: ImagesConfig::default(),
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
    /// Seconds before a command is killed, unless the model asks for another timeout.
    pub timeout: u64,
    /// The most seconds the model may ask a command to be allowed with the bash tool's
    /// `timeout` argument.
    pub max_timeout: u64,
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
    /// A 120 second timeout that the model may raise to 600 for one command, and pagers turned off.
    fn default() -> Self {
        let env = [("PAGER", "cat"), ("MANPAGER", "cat"), ("LESS", "-R"), ("PIP_PROGRESS_BAR", "off"), ("TQDM_DISABLE", "1"), ("GIT_PAGER", "cat")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        EnvironmentConfig {
            kind: "local".into(),
            cwd: String::new(),
            timeout: 120,
            max_timeout: 600,
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
    /// OpenAI compatible servers only: stream the reply, so Ctrl+C closes the connection and the
    /// server stops generating. On unless set to false.
    pub stream: Option<bool>,
    /// OpenAI compatible servers only: send each assistant message's reasoning text back to the
    /// server, exactly as it was streamed. A server that renders thinking into the prompt can then
    /// reuse its prompt cache for the whole history. On unless set to false.
    pub send_reasoning: Option<bool>,
    /// OpenAI compatible servers only: send `chat_template_kwargs.preserve_thinking: true`, so a
    /// Qwen style chat template keeps the reasoning of earlier turns. Defaults to on for a server
    /// on this machine and off for any other server.
    pub preserve_thinking: Option<bool>,
    /// The model's context window in tokens, used instead of asking the server.
    pub context_window: Option<u64>,
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
    /// Send MCP tools by name only, with a `tool_search` tool that loads their definitions:
    /// `auto` when the definitions would take more than `defer_ratio` of the context window,
    /// `always`, or `never`.
    pub defer_tools: DeferTools,
    /// The share of the context window the MCP tool definitions may take before `auto` defers them.
    pub defer_ratio: f64,
}

/// When MCP tool definitions are deferred behind `tool_search`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeferTools {
    Auto,
    Always,
    Never,
}

impl Default for McpConfig {
    /// MCP on, every server, 30 second startup and 120 second calls, tools deferred when their
    /// definitions would take more than a tenth of the context window (Claude Code's default).
    fn default() -> Self {
        McpConfig { enabled: true, only: Vec::new(), disabled: Vec::new(), startup_timeout_seconds: 30, call_timeout_seconds: 120, defer_tools: DeferTools::Auto, defer_ratio: 0.1 }
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
    /// The request that asks the model to summarize the session when it is compacted.
    pub compact_summary: String,
    /// The message that carries the summary into the compacted history.
    pub compact_resume: String,
}

/// Context compaction. Sizes given as a ratio are shares of the model's context window, so
/// the same settings work for a 32,000 token local model and a 1,000,000 token Claude model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CompactionConfig {
    pub enabled: bool,
    /// The model's context window in tokens. 0 means ask the provider, then a built in table.
    pub context_window: u64,
    /// Compact when the estimated request size passes this share of the window.
    pub trigger_ratio: f64,
    /// A mask pass that leaves the estimate above this share is followed by a summary.
    pub target_ratio: f64,
    /// The newest command outputs worth this share of the window are never masked.
    pub protect_recent_ratio: f64,
    /// The protected newest outputs are never more than this many tokens.
    pub protect_recent_max: u64,
    /// A mask pass must free at least this share of the window, or it masks nothing.
    pub min_reclaim_ratio: f64,
    /// The minimum a mask pass must free is never more than this many tokens.
    pub min_reclaim_max: u64,
    /// The newest turns worth this share of the window are kept word for word by a summary.
    pub keep_recent_ratio: f64,
    /// Command outputs shorter than this many characters are never masked.
    pub mask_min_chars: usize,
    /// Work to a window of at most this many tokens, even when the model's is larger. A long
    /// session is then compacted before each request grows slow to process. 0 means no cap.
    pub max_context_tokens: u64,
}

impl Default for CompactionConfig {
    /// On, compacting at 80% of the window, with opencode's 40,000 and 20,000 token prune sizes
    /// as upper limits.
    fn default() -> Self {
        CompactionConfig {
            enabled: true,
            context_window: 0,
            trigger_ratio: 0.8,
            target_ratio: 0.6,
            protect_recent_ratio: 0.2,
            protect_recent_max: 40_000,
            min_reclaim_ratio: 0.1,
            min_reclaim_max: 20_000,
            keep_recent_ratio: 0.1,
            mask_min_chars: 400,
            max_context_tokens: 0,
        }
    }
}

/// Images the person attaches, and images MCP tools return.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ImagesConfig {
    /// Images whose long edge is bigger are scaled down to it. 0 never scales.
    pub max_long_edge: u32,
    /// Images whose base64 text is bigger than this many bytes are refused.
    pub max_bytes: usize,
    /// Attach image files named in a task, a follow up task or a comment.
    pub detect_paths: bool,
}

impl Default for ImagesConfig {
    /// Claude's 1568 pixel long edge and the Claude API's 10 MB limit, with path detection on.
    fn default() -> Self {
        ImagesConfig { max_long_edge: 1568, max_bytes: 10 * 1024 * 1024, detect_paths: true }
    }
}

/// Session listing settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionsConfig {
    /// Ask the model for a session title and summary after the first turn.
    pub auto_title: bool,
}

impl Default for SessionsConfig {
    /// Titles on.
    fn default() -> Self {
        SessionsConfig { auto_title: true }
    }
}

/// The weekly usage bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UsageConfig {
    /// Show the weekly usage bar for a Claude login.
    pub enabled: bool,
}

impl Default for UsageConfig {
    /// Usage on.
    fn default() -> Self {
        UsageConfig { enabled: true }
    }
}

/// One `[models.<name>]` table: a short name for a provider and model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NamedModel {
    pub provider: String,
    pub model: String,
    /// Shown in pickers when set.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
}

impl NamedModel {
    /// The `provider/model` string this name stands for.
    pub fn spec(&self) -> String {
        format!("{}/{}", self.provider, self.model)
    }
}

/// The `models` setting. It is written either as a table of named models or, as before, as a list
/// of `provider/model` strings. A table entry may also be a plain `provider/model` string.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelsConfig {
    pub named: BTreeMap<String, NamedModel>,
    pub listed: Vec<String>,
}

impl ModelsConfig {
    /// Every `provider/model` string, named models first, without repeats.
    pub fn specs(&self) -> Vec<String> {
        let mut specs: Vec<String> = self.named.values().map(NamedModel::spec).collect();
        for spec in &self.listed {
            if !specs.contains(spec) {
                specs.push(spec.clone());
            }
        }
        specs
    }
}

impl Serialize for ModelsConfig {
    /// Writes the named models as a table, or the list when there are no named models.
    /// @param serializer - the output
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.named.is_empty() { self.listed.serialize(serializer) } else { self.named.serialize(serializer) }
    }
}

impl<'de> Deserialize<'de> for ModelsConfig {
    /// Reads a list of `provider/model` strings or a table of named models.
    /// @param deserializer - the input
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let mut models = ModelsConfig::default();
        match value {
            Value::Array(items) => models.listed = items.into_iter().filter_map(|v| v.as_str().map(String::from)).collect(),
            Value::Object(map) => {
                for (name, entry) in map {
                    let named = match entry {
                        Value::String(spec) => match spec.split_once('/') { Some((p, m)) => NamedModel { provider: p.into(), model: m.into(), label: String::new() }, None => continue },
                        other => serde_json::from_value(other).map_err(serde::de::Error::custom)?,
                    };
                    models.named.insert(name, named);
                }
            }
            Value::Null => {}
            _ => return Err(serde::de::Error::custom("models must be a list of provider/model strings or a table of named models")),
        }
        Ok(models)
    }
}
