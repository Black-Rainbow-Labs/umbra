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
    /// List MCP servers. With --connect, start them and list their tools.
    Mcp(McpArgs),
    /// Print the merged config.
    Config(InspectArgs),
    /// Print the system prompt and first user message a task would get, without running it.
    Prompt(PromptArgs),
    /// List saved sessions, newest first, for --resume.
    Sessions(SessionsArgs),
    /// Pick a saved session from a list and continue it. With an id, continue that session.
    Resume(ResumeArgs),
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
    #[command(flatten)]
    pub inspect: InspectArgs,
    /// Start each server and list its tools.
    #[arg(long)]
    pub connect: bool,
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
