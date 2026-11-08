//! Puts a run together from the command line: config, context, MCP servers, environment and
//! model, then runs the agent and reports the result.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::json;

use super::agent_service::{Agent, AgentParts, RunResult};
use super::context_service::{context_vars, discovery_context, expand_slash_command, load_run_context, RunContext};
use super::environment::create_environment;
use super::interaction_service::{show_notice, Interaction};
use super::mcp::McpManager;
use super::prompt_service::platform_vars;
use super::providers::create_provider;
use crate::cli::{CommonArgs, PromptArgs, RunArgs};
use crate::models::config::{ActionFormat, Mode, UmbraConfig};
use crate::repositories::config_repository::{load_config, merge_values};
use crate::repositories::trajectory_repository::trajectory_path;
use crate::utils::console::{read_line, set_quiet};
use crate::utils::paths::{absolute_path, project_root, umbra_home};

/// Loads the config and applies the options every subcommand shares.
/// @param common - the shared options
pub fn prepare_config(common: &CommonArgs) -> Result<(UmbraConfig, PathBuf)> {
    let cwd = match &common.cwd {
        Some(dir) if dir.is_dir() => absolute_path(dir),
        Some(dir) => bail!("no folder {}", dir.display()),
        None => std::env::current_dir()?,
    };
    let mut config = load_config(&project_root(&cwd), &common.config)?;
    apply_common(&mut config, common)?;
    Ok((config, cwd))
}

/// Applies the shared command line options on top of the config files.
/// @param config - the merged config
/// @param common - the shared options
fn apply_common(config: &mut UmbraConfig, common: &CommonArgs) -> Result<()> {
    if let Some(model) = &common.model {
        config.model = model.clone();
    }
    if config.model.is_empty() {
        config.model = std::env::var("UMBRA_MODEL").unwrap_or_default();
    }
    if common.text {
        config.action_format = ActionFormat::Text;
    }
    if let Some(image) = &common.docker {
        config.environment.kind = "docker".into();
        config.environment.image = image.clone();
    }
    config.mcp.enabled &= !common.no_mcp;
    if !common.mcp.is_empty() {
        config.mcp.only = common.mcp.clone();
    }
    config.skills.enabled &= !common.no_skills;
    config.instructions.enabled &= !common.no_instructions;
    disable_sources(config, &common.no_source)
}

/// Turns off the named sources.
/// @param config - the merged config
/// @param names - source names such as claude or codex
fn disable_sources(config: &mut UmbraConfig, names: &[String]) -> Result<()> {
    if names.is_empty() {
        return Ok(());
    }
    let mut sources = serde_json::to_value(&config.sources)?;
    for name in names {
        if sources.get(name).is_none() {
            bail!("unknown source '{name}'");
        }
        merge_values(&mut sources, json!({ name: false }));
    }
    config.sources = serde_json::from_value(sources)?;
    Ok(())
}

/// Applies the run options: mode, limits, output and exit behavior.
/// @param config - the merged config
/// @param args - the run options
fn apply_run_args(config: &mut UmbraConfig, args: &RunArgs) -> Result<()> {
    if let Some(mode) = &args.mode {
        config.mode = serde_json::from_value(json!(mode)).with_context(|| format!("unknown mode '{mode}'. Use confirm, yolo or human"))?;
    }
    if args.yolo {
        config.mode = Mode::Yolo;
    }
    if let Some(limit) = args.cost_limit {
        config.cost_limit = limit;
    }
    if let Some(limit) = args.step_limit {
        config.step_limit = limit;
    }
    if let Some(output) = &args.output {
        config.output = output.to_string_lossy().into_owned();
    }
    if args.exit_immediately {
        config.confirm_exit = false;
    }
    Ok(())
}

/// Runs a task from the command line and returns the process exit code: 0 when the agent
/// submitted, 1 for any other ending.
/// @param args - the run options
pub fn run_task(args: &RunArgs) -> Result<i32> {
    let (mut config, cwd) = prepare_config(&args.common)?;
    apply_run_args(&mut config, args)?;
    set_quiet(args.quiet);
    let interaction = Interaction::new(config.mode, &config.whitelist_actions, config.confirm_exit);
    if !interaction.interactive && config.mode != Mode::Yolo {
        bail!("stdin is not a terminal, so Umbra cannot ask before running commands. Pass --yolo to run without asking");
    }
    let context = load_run_context(&config, &discovery_context(&config, &cwd));
    let task = task_text(args)?;
    let task = expand_slash_command(&task, &context);
    let mut agent = build_agent(config, &cwd, &context, interaction, true)?;
    let result = agent.run(&task)?;
    report(&result, args.quiet);
    Ok(if result.exit_status == "Submitted" { 0 } else { 1 })
}

/// Prints the system prompt and first message for a task, for `umbra prompt`.
/// @param args - the prompt options
pub fn print_prompt(args: &PromptArgs) -> Result<()> {
    let (mut config, cwd) = prepare_config(&args.common)?;
    config.mcp.enabled &= args.connect;
    if config.model.is_empty() {
        config.model = "replay/none".into();
    }
    let context = load_run_context(&config, &discovery_context(&config, &cwd));
    let task = expand_slash_command(&args.task_words.join(" "), &context);
    let interaction = Interaction::new(Mode::Yolo, &[], false);
    let agent = build_agent(config, &cwd, &context, interaction, false)?;
    let (system, instance) = agent.render_prompts(&task)?;
    println!("===== system =====\n{system}\n\n===== user =====\n{instance}");
    Ok(())
}

/// Builds the agent: the model, the environment, the MCP servers and the template variables.
/// @param config - the merged config
/// @param cwd - the working folder
/// @param context - the loaded instructions, skills and MCP definitions
/// @param interaction - the interactive settings
/// @param need_provider - false for `umbra prompt`, which never calls a model
fn build_agent(config: UmbraConfig, cwd: &Path, context: &RunContext, interaction: Interaction, need_provider: bool) -> Result<Agent> {
    let provider = if need_provider { create_provider(&config, &config.model)? } else { Box::new(NoProvider) as Box<dyn super::providers::Provider> };
    let environment = create_environment(&config.environment, cwd)?;
    let mcp = connect_mcp(&config, context);
    let mut vars = platform_vars();
    merge_values(&mut vars, environment.template_vars());
    merge_values(&mut vars, context_vars(context));
    vars["mcp_instructions"] = json!(mcp.instructions().into_iter().map(|(name, instructions)| json!({ "name": name, "instructions": instructions })).collect::<Vec<_>>());
    vars["model_name"] = json!(provider.model_name());
    let config_record = json!({ "agent": { "mode": config.mode, "action_format": config.action_format, "step_limit": config.step_limit, "cost_limit": config.cost_limit }, "model": provider.describe(), "environment": environment.describe(), "mcp_servers": mcp.connected(), "skills": context.skills.len(), "instruction_files": context.instructions.iter().map(|i| i.path.display().to_string()).collect::<Vec<_>>() });
    let trajectory_path = trajectory_path(&config.output);
    Ok(Agent::new(AgentParts { config, provider, environment, mcp, interaction, vars, trajectory_path, config_record }))
}

/// Starts the MCP servers and says which ones did not start.
/// @param config - the merged config
/// @param context - the loaded MCP definitions
fn connect_mcp(config: &UmbraConfig, context: &RunContext) -> McpManager {
    if !config.mcp.enabled || context.mcp_servers.is_empty() {
        return McpManager::default();
    }
    let manager = McpManager::connect_all(&context.mcp_servers, &config.mcp, &umbra_home().join("logs").join("mcp"));
    for failure in &manager.failures {
        eprintln!("umbra: MCP server {} was not started: {}", failure.name, failure.error.lines().next().unwrap_or(""));
    }
    manager
}

/// Gets the task from the options, or asks for it in a terminal.
/// @param args - the run options
fn task_text(args: &RunArgs) -> Result<String> {
    if let Some(task) = &args.task {
        return Ok(task.clone());
    }
    if !args.task_words.is_empty() {
        return Ok(args.task_words.join(" "));
    }
    match read_line("What do you want to do? ") {
        Some(task) if !task.trim().is_empty() => Ok(task),
        _ => bail!("no task given. Pass it as an argument or with -t"),
    }
}

/// Prints how the run ended.
/// @param result - the run's result
/// @param quiet - print only the submission
fn report(result: &RunResult, quiet: bool) {
    if quiet {
        print!("{}", result.submission);
        return;
    }
    let summary = format!("exit status: {}\nmodel calls: {}, cost: ${:.4}, tokens in/out: {}/{}\ntrajectory: {}", result.exit_status, result.stats.api_calls, result.stats.instance_cost, result.stats.input_tokens, result.stats.output_tokens, result.trajectory_path.display());
    show_notice("finished", &summary);
    if !result.submission.trim().is_empty() {
        show_notice("final output", &result.submission);
    }
}

/// A provider that is never called, used when only the prompts are printed.
struct NoProvider;

impl super::providers::Provider for NoProvider {
    /// Always fails; `umbra prompt` never calls it.
    fn complete(&mut self, _m: &[crate::models::message::Message], _t: &[super::providers::ToolSpec]) -> Result<crate::models::model_response::ModelResponse, super::providers::ProviderError> {
        Err(super::providers::ProviderError::fatal("no model in prompt mode"))
    }

    /// The placeholder model name.
    fn model_name(&self) -> &str {
        "none"
    }

    /// Nothing to record.
    fn describe(&self) -> serde_json::Value {
        json!({ "provider": "none" })
    }
}
