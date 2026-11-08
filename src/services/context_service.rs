//! Gathers what a run knows before the first model call: instruction files, skills, slash
//! commands and MCP servers, all from the locations the other harnesses use.

use std::path::Path;

use serde_json::{json, Value};

use crate::models::config::UmbraConfig;
use crate::models::context::{InstructionFile, McpServerDefinition, Skill, SlashCommand};
use crate::repositories::command_repository::{discover_commands, render_command};
use crate::repositories::instruction_repository::load_instructions;
use crate::repositories::mcp_config_repository::discover_mcp_servers;
use crate::repositories::skill_repository::{discover_skills, skill_body};
use crate::repositories::DiscoveryContext;
use crate::utils::paths::{home_dir, project_root};

/// Skill descriptions longer than this are shortened in the prompt, as Claude Code does.
const MAX_SKILL_DESCRIPTION: usize = 1024;

/// Everything loaded from disk for one run.
#[derive(Debug, Clone, Default)]
pub struct RunContext {
    pub instructions: Vec<InstructionFile>,
    pub skills: Vec<Skill>,
    pub commands: Vec<SlashCommand>,
    pub mcp_servers: Vec<McpServerDefinition>,
}

/// Builds the discovery folders for a working folder.
/// @param config - the merged config, for the enabled sources
/// @param cwd - the folder the run works in
pub fn discovery_context(config: &UmbraConfig, cwd: &Path) -> DiscoveryContext {
    DiscoveryContext { home: home_dir(), cwd: cwd.to_path_buf(), project_root: project_root(cwd), sources: config.sources.clone() }
}

/// Loads every enabled kind of context.
/// @param config - the merged config
/// @param ctx - the discovery folders
pub fn load_run_context(config: &UmbraConfig, ctx: &DiscoveryContext) -> RunContext {
    let instructions = if config.instructions.enabled { load_instructions(ctx, config.instructions.max_bytes, &config.instructions.extra_files) } else { Vec::new() };
    let skills = if config.skills.enabled { discover_skills(ctx, &config.skills.extra_paths, &config.skills.disabled) } else { Vec::new() };
    let commands = if config.commands.enabled { discover_commands(ctx) } else { Vec::new() };
    let mcp_servers = if config.mcp.enabled { discover_mcp_servers(ctx, &config.mcp_servers) } else { Vec::new() };
    RunContext { instructions, skills, commands, mcp_servers }
}

/// Replaces a task that starts with `/name` with that command's prompt, or with the body of the
/// skill of that name (Claude Code treats skills as slash commands too). Any other task is
/// returned unchanged.
/// @param task - the task text
/// @param context - the loaded commands and skills
pub fn expand_slash_command(task: &str, context: &RunContext) -> String {
    let trimmed = task.trim_start();
    let Some(rest) = trimmed.strip_prefix('/') else { return task.to_string() };
    let (name, arguments) = rest.split_once(char::is_whitespace).map(|(n, a)| (n, a.trim())).unwrap_or((rest, ""));
    if let Some(command) = context.commands.iter().find(|c| c.name == name) {
        return render_command(&command.body, arguments);
    }
    if let Some(skill) = context.skills.iter().find(|s| s.name == name) {
        let folder = skill.path.parent().map(|p| p.display().to_string()).unwrap_or_default();
        return format!("Use the {name} skill. Its folder is {folder}. Its SKILL.md says:\n\n{}\n\nARGUMENTS: {arguments}", skill_body(skill).trim());
    }
    task.to_string()
}

/// The template variables that describe the loaded context.
/// @param context - the loaded context
pub fn context_vars(context: &RunContext) -> Value {
    let skills: Vec<Value> = context.skills.iter().map(|s| json!({ "name": s.name, "description": shorten(&s.description, MAX_SKILL_DESCRIPTION), "path": s.path.display().to_string(), "source": s.source })).collect();
    let instructions: Vec<Value> = context.instructions.iter().map(|i| json!({ "path": i.path.display().to_string(), "source": i.source, "scope": i.scope, "content": i.content })).collect();
    json!({ "instructions": instructions, "skills": skills })
}

/// Shortens text to a number of characters, ending with an ellipsis when cut.
/// @param text - the text
/// @param limit - the most characters to keep
fn shorten(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let cut: String = text.chars().take(limit.saturating_sub(3)).collect();
    format!("{cut}...")
}
