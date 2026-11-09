//! Puts a run together from the command line: config, context, MCP servers, environment and
//! model, then runs the agent and reports the result.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::json;

use super::agent_service::{now_seconds, Agent, AgentParts, Opening, RunResult, INTERRUPTED_STATUS};
use super::context_service::{context_vars, discovery_context, expand_slash_command, load_run_context, RunContext};
use super::environment::create_environment;
use super::image_service::{images_in_text, load_image};
use super::interaction_service::{show_notice, Interaction};
use super::mcp::McpManager;
use super::prompt_service::platform_vars;
use super::providers::create_provider;
use super::session_service::{find_resume_target, load_resumed, ResumedSession};
use crate::cli::{CommonArgs, PromptArgs, RunArgs};
use crate::models::config::{ActionFormat, ImagesConfig, Mode, UmbraConfig};
use crate::models::image::ImageData;
use crate::models::session::{ResumeRecord, SessionInfo};
use crate::repositories::config_repository::{load_config, merge_values};
use crate::repositories::session_repository::new_session_id;
use crate::repositories::trajectory_repository::trajectory_path;
use crate::utils::console::{read_line, set_quiet, stdin_is_terminal};
use crate::utils::interrupt;
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
    config.compaction.enabled &= !args.no_compact;
    if let Some(window) = args.context_window {
        config.compaction.context_window = window;
    }
    Ok(())
}

/// Runs a task from the command line, or continues a saved session, and returns the process exit
/// code: 0 when the agent submitted, 130 when the person ended it with Ctrl+C, 1 for any other
/// ending.
/// @param args - the run options
pub fn run_task(args: &RunArgs) -> Result<i32> {
    interrupt::install();
    let mut args = args.clone();
    let resumed = load_resume_target(&args)?;
    if let Some(resumed) = &resumed {
        fill_from_session(&mut args, &resumed.session);
    }
    let (mut config, cwd) = prepare_config(&args.common)?;
    apply_run_args(&mut config, &args)?;
    set_quiet(args.quiet);
    let interaction = Interaction::new(config.mode, &config.whitelist_actions, config.confirm_exit);
    if !interaction.interactive && config.mode != Mode::Yolo {
        bail!("stdin is not a terminal, so Umbra cannot ask before running commands. Pass --yolo to run without asking");
    }
    let context = load_run_context(&config, &discovery_context(&config, &cwd));
    let task = expand_slash_command(&task_text(&args, resumed.as_ref())?, &context);
    let images = opening_images(&args.images, &task, &cwd, &config.images)?;
    let (session, trajectory_path) = session_for_run(&config, &args, &cwd, &task, resumed.as_ref());
    let mut agent = build_agent(config, &cwd, &context, interaction, AgentSession { session, trajectory_path, need_provider: true })?;
    let opening = match resumed {
        Some(resumed) => Opening::Resume { session: Box::new(resumed), task, images },
        None => Opening::New { task, images },
    };
    let result = agent.run(opening)?;
    report(&result, args.quiet);
    Ok(match result.exit_status.as_str() {
        "Submitted" => 0,
        INTERRUPTED_STATUS => 130,
        _ => 1,
    })
}

/// Loads the session named by `--resume` or `--continue`, if either was given, and says what
/// was loaded.
/// @param args - the run options
fn load_resume_target(args: &RunArgs) -> Result<Option<ResumedSession>> {
    let here = match &args.common.cwd {
        Some(dir) => absolute_path(dir),
        None => std::env::current_dir()?,
    };
    let Some(path) = find_resume_target(args.resume.as_deref(), args.continue_last, &here, stdin_is_terminal())? else {
        if args.fork {
            bail!("--fork needs --resume or --continue");
        }
        return Ok(None);
    };
    let resumed = load_resumed(&path)?;
    let mut notice = format!("session {} from {}: {} messages, {} model calls so far", resumed.session.id, path.display(), resumed.messages.len(), resumed.stats.api_calls);
    if resumed.repaired > 0 {
        notice.push_str(&format!(". {} tool calls that never ran were given a result", resumed.repaired));
    }
    if !args.quiet {
        show_notice("resuming", &notice);
    }
    Ok(Some(resumed))
}

/// A resumed session runs in its own folder and with its own model unless the command line
/// says otherwise.
/// @param args - the run options, changed in place
/// @param session - the saved session
fn fill_from_session(args: &mut RunArgs, session: &SessionInfo) {
    if args.common.cwd.is_none() && !session.cwd.is_empty() && Path::new(&session.cwd).is_dir() {
        args.common.cwd = Some(PathBuf::from(&session.cwd));
    }
    if args.common.model.is_none() && !session.model.is_empty() {
        args.common.model = Some(session.model.clone());
    }
}

/// Loads the images for the first message: every `--image`, which must load, and every image
/// path in the task text, which is skipped with a notice when it cannot be used.
/// @param paths - the `--image` files
/// @param task - the task text
/// @param cwd - the working folder
/// @param settings - the image settings
fn opening_images(paths: &[PathBuf], task: &str, cwd: &Path, settings: &ImagesConfig) -> Result<Vec<ImageData>> {
    let mut images = Vec::new();
    for path in paths {
        images.push(load_image(&cwd.join(path), settings)?);
    }
    let (found, notices) = images_in_text(task, cwd, settings);
    for image in found {
        if !images.iter().any(|i: &ImageData| i.source == image.source) {
            images.push(image);
        }
    }
    for notice in notices {
        show_notice("image", &notice);
    }
    for image in &images {
        show_notice("image", &format!("attached {} {}", image.source, image.describe()));
    }
    Ok(images)
}

/// The session record and trajectory file for this run. A new run gets a new id. A resume keeps
/// the session's id and file, unless `--fork` asks for a new one, and records the resume.
/// @param config - the merged config
/// @param args - the run options
/// @param cwd - the working folder
/// @param task - the task text
/// @param resumed - the saved session, if resuming
fn session_for_run(config: &UmbraConfig, args: &RunArgs, cwd: &Path, task: &str, resumed: Option<&ResumedSession>) -> (SessionInfo, PathBuf) {
    let cwd_text = cwd.to_string_lossy().replace('\\', "/");
    let Some(resumed) = resumed else {
        let session = SessionInfo { id: new_session_id(), cwd: cwd_text, model: config.model.clone(), task: task.chars().take(200).collect(), started_at: now_seconds(), resumes: Vec::new() };
        let path = trajectory_path(&config.output, &session.id);
        return (session, path);
    };
    let mut session = resumed.session.clone();
    let mut record = ResumeRecord { at: now_seconds(), model: config.model.clone(), messages_before: resumed.messages.len(), forked_from: String::new() };
    if args.fork {
        record.forked_from = session.id.clone();
        session.id = new_session_id();
    }
    session.resumes.push(record);
    session.model = config.model.clone();
    session.cwd = cwd_text;
    if session.task.is_empty() {
        session.task = task.chars().take(200).collect();
    }
    let path = match (&args.output, args.fork) {
        (Some(output), _) => output.clone(),
        (None, true) => trajectory_path("", &session.id),
        (None, false) => resumed.trajectory_path.clone(),
    };
    (session, path)
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
    let session = AgentSession { session: SessionInfo::default(), trajectory_path: PathBuf::new(), need_provider: false };
    let agent = build_agent(config, &cwd, &context, interaction, session)?;
    let (system, instance) = agent.render_prompts(&task)?;
    println!("===== system =====\n{system}\n\n===== user =====\n{instance}");
    Ok(())
}

/// The session parts of an agent.
struct AgentSession {
    session: SessionInfo,
    trajectory_path: PathBuf,
    /// False for `umbra prompt`, which never calls a model.
    need_provider: bool,
}

/// Builds the agent: the model, the environment, the MCP servers and the template variables.
/// @param config - the merged config
/// @param cwd - the working folder
/// @param context - the loaded instructions, skills and MCP definitions
/// @param interaction - the interactive settings
/// @param run - the session record, the trajectory file, and whether a model is needed
fn build_agent(config: UmbraConfig, cwd: &Path, context: &RunContext, interaction: Interaction, run: AgentSession) -> Result<Agent> {
    let provider = if run.need_provider { create_provider(&config, &config.model)? } else { Box::new(NoProvider) as Box<dyn super::providers::Provider> };
    let environment = create_environment(&config.environment, cwd)?;
    let mut mcp = connect_mcp(&config, context);
    mcp.set_image_settings(config.images.clone());
    let mut vars = platform_vars();
    merge_values(&mut vars, environment.template_vars());
    merge_values(&mut vars, context_vars(context));
    vars["mcp_instructions"] = json!(mcp.instructions().into_iter().map(|(name, instructions)| json!({ "name": name, "instructions": instructions })).collect::<Vec<_>>());
    vars["model_name"] = json!(provider.model_name());
    let config_record = json!({ "agent": { "mode": config.mode, "action_format": config.action_format, "step_limit": config.step_limit, "cost_limit": config.cost_limit, "compaction": config.compaction }, "model": provider.describe(), "environment": environment.describe(), "mcp_servers": mcp.connected(), "skills": context.skills.len(), "instruction_files": context.instructions.iter().map(|i| i.path.display().to_string()).collect::<Vec<_>>() });
    Ok(Agent::new(AgentParts { config, provider, environment, mcp, interaction, vars, trajectory_path: run.trajectory_path, config_record, cwd: cwd.to_path_buf(), session: run.session }))
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

/// Gets the task from the options, or asks for it in a terminal. A resumed session asks for
/// the next task.
/// @param args - the run options
/// @param resumed - the saved session, if resuming
fn task_text(args: &RunArgs, resumed: Option<&ResumedSession>) -> Result<String> {
    if let Some(task) = &args.task {
        return Ok(task.clone());
    }
    if !args.task_words.is_empty() {
        return Ok(args.task_words.join(" "));
    }
    let prompt = match resumed {
        Some(r) => format!("Resuming session {} ({} messages). What next? ", r.session.id, r.messages.len()),
        None => "What do you want to do? ".to_string(),
    };
    if !stdin_is_terminal() {
        bail!("no task given. Pass it as an argument or with -t");
    }
    match read_line(&prompt) {
        Some(task) if !task.trim().is_empty() => Ok(task),
        _ => bail!("no task given. Pass it as an argument or with -t"),
    }
}

/// Prints how the run ended and how to resume it. An interrupted run always says how, even
/// in quiet mode, on stderr.
/// @param result - the run's result
/// @param quiet - print only the submission
fn report(result: &RunResult, quiet: bool) {
    if quiet {
        print!("{}", result.submission);
        if result.exit_status == INTERRUPTED_STATUS {
            eprintln!("umbra: interrupted. Resume with: umbra --resume {}", result.session_id);
        }
        return;
    }
    let summary = format!("exit status: {}\nmodel calls: {}, cost: ${:.4}, tokens in/out: {}/{}, compactions: {}\ntrajectory: {}\nsession: {} (resume with: umbra --resume {})", result.exit_status, result.stats.api_calls, result.stats.instance_cost, result.stats.input_tokens, result.stats.output_tokens, result.stats.compactions, result.trajectory_path.display(), result.session_id, result.session_id);
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
