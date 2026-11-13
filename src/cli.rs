//! Command line arguments.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Umbra: a small coding agent harness modeled after mini-swe-agent.
#[derive(Debug, Parser)]
#[command(name = "umbra", version, about, args_conflicts_with_subcommands = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
    #[command(flatten)]
    pub run: RunArgs,
}

/// The subcommands. With none, Umbra runs a task.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run a task (the default).
    Run(RunArgs),
    /// List the skills Umbra found and where.
    Skills(InspectArgs),
    /// List the instruction files Umbra loads, in prompt order.
    Instructions(InspectArgs),
    /// List the custom slash commands Umbra found.
    Commands(InspectArgs),
    /// List MCP servers, or turn one on or off for every session. With --connect, start them and
    /// list their tools.
    Mcp(McpArgs),
    /// Print the merged config, or read and change one setting.
    Config(ConfigArgs),
    /// List the models: the default, the named models and the known models.
    Models(InspectArgs),
    /// Control a running session: its model, mode and MCP servers. Inside a session, the commands
    /// the agent runs reach that session; elsewhere, name it with --session.
    Session(SessionArgs),
    /// Check for a newer release of Umbra and install it.
    Update(UpdateArgs),
    /// Print the system prompt and first user message a task would get, without running it.
    Prompt(PromptArgs),
    /// List saved sessions, newest first, for --resume.
    Sessions(SessionsArgs),
    /// Pick a saved session from a list and continue it. With an id, continue that session.
    Resume(ResumeArgs),
    /// Write a saved session as a trajectory JSON file in the mini-swe-agent-1.1 format. Sessions
    /// are stored as a journal, so this is how to get the JSON when -o was not given.
    Export(ExportArgs),
    /// Run as an Agent Client Protocol server on stdin and stdout, for an editor such as Zed,
    /// a JetBrains IDE or Unluminous.
    Acp(AcpArgs),
    /// Choose a model and save it in ~/.umbra/config.toml.
    Setup(SetupArgs),
}

/// Options for `umbra acp`. The working folder comes from the editor, one per session.
#[derive(Debug, Clone, Args, Default)]
pub struct AcpArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    /// The mode a new session starts in: confirm (ask the editor before each command) or yolo.
    #[arg(long)]
    pub mode: Option<String>,
    /// Start new sessions in yolo mode.
    #[arg(short, long)]
    pub yolo: bool,
    /// Stop a turn once it has cost this many dollars. 0 means no limit.
    #[arg(short = 'l', long)]
    pub cost_limit: Option<f64>,
    /// Stop a turn after this many model calls. 0 means no limit.
    #[arg(long)]
    pub step_limit: Option<u32>,
    /// Run the setup wizard in this terminal instead of serving ACP. Editors run this for the
    /// "Set up Umbra in a terminal" sign in method.
    #[arg(long)]
    pub setup: bool,
}

/// Options for `umbra setup`.
#[derive(Debug, Clone, Args, Default)]
pub struct SetupArgs {
    /// Use this provider/model without asking, for example anthropic/claude-haiku-5-5.
    #[arg(short, long)]
    pub model: Option<String>,
    /// Do not send the test request to the model.
    #[arg(long)]
    pub no_test: bool,
}

/// Options for running a task.
#[derive(Debug, Clone, Args, Default)]
pub struct RunArgs {
    /// The task. Words after the options are joined with spaces. A task that starts with
    /// `/name` runs that slash command or skill.
    pub task_words: Vec<String>,
    /// The task, as one argument.
    #[arg(short, long)]
    pub task: Option<String>,
    #[command(flatten)]
    pub common: CommonArgs,
    /// Run every action without asking.
    #[arg(short, long)]
    pub yolo: bool,
    /// confirm, yolo or human.
    #[arg(long)]
    pub mode: Option<String>,
    /// Stop once the run has cost this many dollars. 0 means no limit.
    #[arg(short = 'l', long)]
    pub cost_limit: Option<f64>,
    /// Stop after this many model calls. 0 means no limit.
    #[arg(long)]
    pub step_limit: Option<u32>,
    /// Where to write the trajectory JSON.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Exit when the agent finishes instead of asking for a follow up task.
    #[arg(long)]
    pub exit_immediately: bool,
    /// Never end the run because of an empty line. The task prompt and the follow up prompt ask
    /// again instead. For a session another program types into, such as a task board, where a
    /// stray Enter must not end the agent.
    #[arg(long)]
    pub keep_open: bool,
    /// Do not draw the status bar under the output.
    #[arg(long)]
    pub no_status_bar: bool,
    /// Print the session inline below the shell prompt instead of opening the fullscreen view on
    /// the terminal's alternate screen.
    #[arg(long)]
    pub no_fullscreen: bool,
    /// Do not turn on mouse reporting in the fullscreen view. The terminal selects text itself and
    /// everything is done with the keyboard.
    #[arg(long)]
    pub no_mouse: bool,
    /// Print every detail of the run: step headings with the cost, whole command outputs, full
    /// format errors and every compaction attempt.
    #[arg(long)]
    pub verbose: bool,
    /// Print only the final output (and errors), for scripts.
    #[arg(short, long)]
    pub quiet: bool,
    /// Never compact the history, even when it nears the model's context window.
    #[arg(long)]
    pub no_compact: bool,
    /// The model's context window in tokens, for compaction. By default Umbra asks the server
    /// or uses a built in table.
    #[arg(long)]
    pub context_window: Option<u64>,
    /// Attach an image (PNG, JPEG, GIF or WebP) to the task. Repeatable. Image paths typed in a
    /// task or at a prompt are attached too.
    #[arg(short = 'i', long = "image", value_name = "PATH")]
    pub images: Vec<PathBuf>,
    /// Resume a saved session: its id, the start of its id, or a trajectory file. With no value,
    /// pick one of this folder's sessions.
    #[arg(short = 'r', long, value_name = "SESSION", num_args = 0..=1, default_missing_value = "")]
    pub resume: Option<String>,
    /// Resume the newest session in this folder.
    #[arg(long = "continue", conflicts_with = "resume")]
    pub continue_last: bool,
    /// With --resume or --continue: copy the history into a new session and leave the old one
    /// as it was.
    #[arg(long)]
    pub fork: bool,
    /// With --resume and --fork: branch from an earlier point, keeping only the first N messages
    /// of the saved history.
    #[arg(long, value_name = "N", requires = "fork")]
    pub at: Option<usize>,
    /// Set by `umbra resume --all`: the session picker lists every folder's sessions.
    #[arg(skip)]
    pub pick_all: bool,
}

/// Options shared by every subcommand.
#[derive(Debug, Clone, Args, Default)]
pub struct CommonArgs {
    /// provider/model, for example anthropic/claude-haiku-5-5 or local/qwen.
    #[arg(short, long)]
    pub model: Option<String>,
    /// More config files, TOML or mini-swe-agent YAML. Repeatable; later files win.
    #[arg(short, long = "config")]
    pub config: Vec<PathBuf>,
    /// Ask for actions in plain text instead of tool calls.
    #[arg(long)]
    pub text: bool,
    /// How a turn ends: reply (a reply with no command ends it, as in Claude Code) or submit
    /// (every reply runs a command until the submit command, as in mini-swe-agent).
    #[arg(long, value_name = "STYLE")]
    pub finish: Option<String>,
    /// The working folder. Defaults to the current folder.
    #[arg(long)]
    pub cwd: Option<PathBuf>,
    /// Run commands in a new container from this Docker image.
    #[arg(long)]
    pub docker: Option<String>,
    /// Do not start MCP servers.
    #[arg(long)]
    pub no_mcp: bool,
    /// Only start these MCP servers (comma separated).
    #[arg(long, value_delimiter = ',')]
    pub mcp: Vec<String>,
    /// Do not load skills.
    #[arg(long)]
    pub no_skills: bool,
    /// Do not load instruction files.
    #[arg(long)]
    pub no_instructions: bool,
    /// Ignore one harness's files: claude, codex, gemini, opencode, cursor, copilot, windsurf,
    /// cline, agents, vscode, claude_desktop, umbra. Repeatable or comma separated.
    #[arg(long, value_delimiter = ',')]
    pub no_source: Vec<String>,
}

/// Options for the listing subcommands.
#[derive(Debug, Clone, Args, Default)]
pub struct InspectArgs {
    #[command(flatten)]
    pub common: CommonArgs,
    /// Print JSON instead of a table.
    #[arg(long)]
    pub json: bool,
}

/// Options for `umbra mcp`.
#[derive(Debug, Clone, Args, Default)]
pub struct McpArgs {
    #[command(subcommand)]
    pub action: Option<McpAction>,
    #[command(flatten)]
    pub inspect: InspectArgs,
    /// Start each server and list its tools.
    #[arg(long)]
    pub connect: bool,
}

/// `umbra mcp enable` and `umbra mcp disable`.
#[derive(Debug, Clone, Subcommand)]
pub enum McpAction {
    /// Let a server start in every session (changes mcp.only and mcp.disabled).
    Enable(McpToggleArgs),
    /// Stop a server from starting in any session (changes mcp.only and mcp.disabled).
    Disable(McpToggleArgs),
}

/// The server `umbra mcp enable` or `disable` changes.
#[derive(Debug, Clone, Args, Default)]
pub struct McpToggleArgs {
    /// The server's name, as `umbra mcp` lists it.
    pub name: String,
    /// Change the project's .umbra/config.toml instead of ~/.umbra/config.toml.
    #[arg(long)]
    pub project: bool,
    /// The working folder whose project file --project changes.
    #[arg(long)]
    pub cwd: Option<PathBuf>,
}

/// Options for `umbra config`.
#[derive(Debug, Clone, Args, Default)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: Option<ConfigAction>,
    #[command(flatten)]
    pub inspect: InspectArgs,
}

/// The `umbra config` subcommands.
#[derive(Debug, Clone, Subcommand)]
pub enum ConfigAction {
    /// List every setting with its value, the file it comes from and what it does.
    List(ConfigListArgs),
    /// Print one setting's value, such as `compaction.trigger_ratio`.
    Get(ConfigKeyArgs),
    /// Set one setting in a config file. Comments in the file are kept.
    Set(ConfigSetArgs),
    /// Remove one setting from a config file, so the default or another file applies.
    Unset(ConfigKeyArgs),
    /// Print the path of the config file `set` and `unset` change.
    Path(ConfigFileArgs),
}

/// Options for `umbra config list`.
#[derive(Debug, Clone, Args, Default)]
pub struct ConfigListArgs {
    /// Print JSON instead of a table.
    #[arg(long)]
    pub json: bool,
    /// Only the settings whose path starts with this, such as `mcp`.
    pub prefix: Option<String>,
    #[command(flatten)]
    pub file: ConfigFileArgs,
}

/// Which config file a `umbra config` subcommand uses.
#[derive(Debug, Clone, Args, Default)]
pub struct ConfigFileArgs {
    /// Use the project's .umbra/config.toml instead of ~/.umbra/config.toml.
    #[arg(long)]
    pub project: bool,
    /// The working folder. Defaults to the current folder.
    #[arg(long)]
    pub cwd: Option<PathBuf>,
}

/// A setting path, for `umbra config get` and `unset`.
#[derive(Debug, Clone, Args, Default)]
pub struct ConfigKeyArgs {
    /// The setting's dotted path, such as `mcp.defer_tools` or `providers.local.base_url`.
    pub key: String,
    #[command(flatten)]
    pub file: ConfigFileArgs,
}

/// A setting path and its new value, for `umbra config set`.
#[derive(Debug, Clone, Args, Default)]
pub struct ConfigSetArgs {
    /// The setting's dotted path.
    pub key: String,
    /// The value. Lists are comma separated, maps are `k=v, k=v`, percents may be written as 70.
    pub value: String,
    #[command(flatten)]
    pub file: ConfigFileArgs,
}

/// Options for `umbra session`.
#[derive(Debug, Clone, Args)]
pub struct SessionArgs {
    #[command(subcommand)]
    pub action: SessionAction,
    /// The session to control: its id or the start of it. Inside a session this is not needed.
    #[arg(long, global = true)]
    pub session: Option<String>,
}

/// The `umbra session` subcommands.
#[derive(Debug, Clone, Subcommand)]
pub enum SessionAction {
    /// Print the session's id, model, mode and MCP servers.
    Status {
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
    /// List the running sessions.
    List,
    /// Switch the session's model from its next model call.
    Model {
        /// A name from [models] or provider/model.
        name: String,
    },
    /// Switch the session's mode: confirm, yolo or human.
    Mode {
        mode: String,
    },
    /// Turn an MCP server on or off in this session only.
    Mcp {
        /// on or off.
        state: String,
        /// The server's name.
        name: String,
    },
    /// Summarize the session's history before its next model call.
    Compact {
        /// What the summary should keep.
        focus: Vec<String>,
    },
    /// End the session after the current step.
    Quit,
}

/// Options for `umbra update`.
#[derive(Debug, Clone, Args, Default)]
pub struct UpdateArgs {
    /// Only check, and print whether a newer release exists.
    #[arg(long)]
    pub check: bool,
    /// Install this version instead of the newest.
    #[arg(long)]
    pub version: Option<String>,
}

/// Options for `umbra resume`.
#[derive(Debug, Clone, Args, Default)]
pub struct ResumeArgs {
    /// The session id, or the start of it. With none, pick from a list.
    pub id: Option<String>,
    /// List every folder's sessions, not only this folder's.
    #[arg(long)]
    pub all: bool,
    #[command(flatten)]
    pub run: RunArgs,
}

/// Options for `umbra export`.
#[derive(Debug, Clone, Args, Default)]
pub struct ExportArgs {
    /// The session id, the start of it, or a trajectory or journal path.
    pub session: String,
    /// Where to write the JSON. Defaults to the session's trajectory path, next to its journal.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

/// Options for `umbra sessions`.
#[derive(Debug, Clone, Args, Default)]
pub struct SessionsArgs {
    /// The folder whose sessions are listed. Defaults to the current folder.
    #[arg(long)]
    pub cwd: Option<PathBuf>,
    /// List every folder's sessions.
    #[arg(long)]
    pub all: bool,
    /// Print JSON instead of a table.
    #[arg(long)]
    pub json: bool,
}

/// Options for `umbra prompt`.
#[derive(Debug, Clone, Args, Default)]
pub struct PromptArgs {
    pub task_words: Vec<String>,
    #[command(flatten)]
    pub common: CommonArgs,
    /// Start MCP servers so their tools appear in the prompt.
    #[arg(long)]
    pub connect: bool,
}
