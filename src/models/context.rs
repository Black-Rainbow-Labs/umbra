//! The things Umbra loads from disk before a run: instruction files, skills, slash commands
//! and MCP server definitions. Each one remembers where it was found.

use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;

/// User level files apply everywhere. Project files apply to one checkout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    User,
    Project,
}

/// One instruction file, such as CLAUDE.md or AGENTS.md, after its imports were expanded.
#[derive(Debug, Clone, Serialize)]
pub struct InstructionFile {
    pub path: PathBuf,
    /// Which harness this file belongs to, for example `claude` or `codex`.
    pub source: String,
    pub scope: Scope,
    pub content: String,
}

/// One Agent Skill: a folder with a SKILL.md whose frontmatter names and describes it.
#[derive(Debug, Clone, Serialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// The SKILL.md file. The model reads it with `cat` when the skill applies.
    pub path: PathBuf,
    pub source: String,
    pub scope: Scope,
}

/// One custom slash command, such as `.claude/commands/review.md`.
#[derive(Debug, Clone, Serialize)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    /// The prompt text. `$ARGUMENTS`, `{{args}}` and `$1`..`$9` are replaced with the arguments.
    pub body: String,
    pub path: PathBuf,
    pub source: String,
}

/// How Umbra talks to an MCP server.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpTransport {
    /// A child process speaking newline separated JSON-RPC on stdin and stdout.
    Stdio { command: String, args: Vec<String>, env: Vec<(String, String)>, cwd: Option<String> },
    /// The Streamable HTTP transport: JSON-RPC over POST, replies as JSON or server sent events.
    Http { url: String, headers: Vec<(String, String)> },
    /// The older HTTP+SSE transport: a GET stream that names a POST endpoint.
    Sse { url: String, headers: Vec<(String, String)> },
}

/// One MCP server definition found in a config file.
#[derive(Debug, Clone, Serialize)]
pub struct McpServerDefinition {
    pub name: String,
    pub transport: McpTransport,
    /// The file it was read from.
    pub origin: PathBuf,
    pub source: String,
    pub scope: Scope,
}

/// A tool offered by a running MCP server.
#[derive(Debug, Clone, Serialize)]
pub struct McpTool {
    pub server: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl McpTool {
    /// The name the model sees for this tool: `mcp__<server>__<tool>`, the Claude Code convention,
    /// with characters the providers reject replaced by underscores and capped at 64 characters.
    pub fn qualified_name(&self) -> String {
        let raw = format!("mcp__{}__{}", self.server, self.name);
        let clean: String = raw.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
        clean.chars().take(64).collect()
    }
}
