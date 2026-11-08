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
