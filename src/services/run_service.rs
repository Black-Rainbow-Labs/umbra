//! Puts a run together from the command line: config, context, MCP servers, environment and
//! model, then runs the agent and reports the result.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::json;

use super::agent_events::{AgentEvents, NoEvents};
use super::agent_service::{is_success, now_seconds, Agent, AgentParts, Opening, RunResult, INTERRUPTED_STATUS};
use super::context_service::{context_vars, discovery_context, expand_slash_command, load_run_context, RunContext};
use super::environment::create_environment;
use super::image_service::{images_in_text, load_image};
use super::interaction_service::{prompt_with_hint, show_event, show_user_text, Interaction, Tone};
use super::mcp::McpManager;
use super::prompt_service::platform_vars;
use super::providers::{check_credentials, create_provider};
use super::session_service::{find_resume_target, load_resumed, ResumedSession};
use crate::cli::{CommonArgs, PromptArgs, RunArgs};
use crate::models::config::{ActionFormat, ImagesConfig, Mode, UmbraConfig};
use crate::models::image::ImageData;
use crate::models::session::{ResumeRecord, SessionInfo};
use crate::repositories::config_repository::{load_config, merge_values};
use crate::repositories::session_repository::new_session_id;
use crate::repositories::trajectory_repository::trajectory_path;
use crate::utils::banner::{display_folder, render_banner_frames, render_banner_step, BannerInfo, REVEAL_STEPS, STEP_MS};
use crate::utils::terminal_mode::terminal_height;
use crate::utils::console::{color_enabled, emit, emit_replacing, paint, enable_status_bar, finish_status, is_verbose, set_quiet, set_verbose, start_fullscreen, stdin_is_terminal, stdout_is_terminal, terminal_width, Input};
use crate::utils::screen;
use crate::utils::status_bar::Phase;
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
    if let Some(finish) = &common.finish {
        config.finish = serde_json::from_value(json!(finish)).with_context(|| format!("unknown finish style '{finish}'. Use reply or submit"))?;
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
    if args.keep_open {
        config.keep_open = true;
    }
    if args.verbose {
        config.verbose = true;
    }
    config.status_bar &= !args.no_status_bar;
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
    let (resumed, resume_notice) = load_resume_target(&args)?;
    if let Some(resumed) = &resumed {
        fill_from_session(&mut args, &resumed.session);
    }
    let (mut config, cwd) = prepare_config(&args.common)?;
    apply_run_args(&mut config, &args)?;
    set_quiet(args.quiet);
    set_verbose(config.verbose && !args.quiet);
    enable_status_bar(config.status_bar);
    let mut interaction = Interaction::new(config.mode, &config.whitelist_actions, config.confirm_exit);
    interaction.keep_open = config.keep_open;
    if !interaction.interactive && config.mode != Mode::Yolo {
        bail!("stdin is not a terminal, so Umbra cannot ask before running commands. Pass --yolo to run without asking");
    }
    // Before the prompt, so a task is never asked for and then dropped because no model can run it.
    check_credentials(&config, &config.model)?;
    let context = load_run_context(&config, &discovery_context(&config, &cwd));
    // Closes the fullscreen view on every way out of this function, errors included.
    let _view = LeaveViewOnDrop;
    start_fullscreen(config.fullscreen && !args.no_fullscreen);
    if !args.quiet {
        show_banner(&config, &cwd, &context);
        if let Some(notice) = &resume_notice {
            show_event(Tone::Info, "↺", notice);
        }
    }
    let typed = task_text(&args, resumed.as_ref(), config.keep_open)?;
    if args.task.is_some() || !args.task_words.is_empty() {
        show_user_text("❯", &typed);
    }
    let task = expand_slash_command(&typed, &context);
    let images = opening_images(&args.images, &task, &cwd, &config.images)?;
    let (session, trajectory_path) = session_for_run(&config, &args, &cwd, &task, resumed.as_ref());
    let mut agent = build_agent(config, &cwd, &context, interaction, AgentSession { session, trajectory_path, need_provider: true, events: Box::new(NoEvents) })?;
    let opening = match resumed {
        Some(resumed) => Opening::Resume { session: Box::new(resumed), task, images },
        None => Opening::New { task, images },
    };
    let result = agent.run(opening)?;
    // Back on the main screen first, so the trajectory line stays in the shell.
    screen::leave();
    report(&result, args.quiet);
    finish_status(Phase::Finished);
    Ok(match result.exit_status.as_str() {
        status if is_success(status) => 0,
        INTERRUPTED_STATUS => 130,
        _ => 1,
    })
}

/// Loads the session named by `--resume` or `--continue`, if either was given, with a line that
/// says what was loaded.
/// @param args - the run options
fn load_resume_target(args: &RunArgs) -> Result<(Option<ResumedSession>, Option<String>)> {
    let here = match &args.common.cwd {
        Some(dir) => absolute_path(dir),
        None => std::env::current_dir()?,
    };
    let Some(path) = find_resume_target(args.resume.as_deref(), args.continue_last, &here, stdin_is_terminal())? else {
        if args.fork {
            bail!("--fork needs --resume or --continue");
        }
        return Ok((None, None));
    };
    let resumed = load_resumed(&path)?;
    let mut notice = format!("resuming session {} from {}: {} messages, {} model calls so far", resumed.session.id, path.display(), resumed.messages.len(), resumed.stats.api_calls);
    if resumed.repaired > 0 {
        notice.push_str(&format!(". {} tool calls that never ran were given a result", resumed.repaired));
    }
    Ok((Some(resumed), Some(notice)))
}

/// Prints the umbra banner: the logo, version, model, folder, mode and what was loaded. Only in a
/// terminal, unless `UMBRA_BANNER=1` asks for it in a pipe; `UMBRA_BANNER=0` or `banner = false`
/// turns it off. In a terminal the logo's stripes appear one at a time, unless
/// `UMBRA_BANNER_ANIMATION=0` turns that off.
/// @param config - the merged config
/// @param cwd - the working folder
/// @param context - the loaded instructions, skills and MCP definitions
fn show_banner(config: &UmbraConfig, cwd: &Path, context: &RunContext) {
    let wanted = match std::env::var("UMBRA_BANNER").ok().as_deref() {
        Some("0") => false,
        Some("1") => true,
        _ => config.banner && stdout_is_terminal(),
    };
    if !wanted {
        return;
    }
    let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned());
    let mode = serde_json::to_value(config.mode).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
    let mcp_servers = if config.mcp.enabled { context.mcp_servers.len() } else { 0 };
    let info = BannerInfo { version: env!("CARGO_PKG_VERSION").into(), model: config.model.clone(), folder: display_folder(&cwd.to_string_lossy(), home.as_deref()), mode, instruction_files: context.instructions.len(), skills: context.skills.len(), mcp_servers };
    let color = color_enabled();
    let animate = color && stdout_is_terminal() && std::env::var("UMBRA_BANNER_ANIMATION").map_or(true, |v| v != "0");
    if screen::is_active() {
        show_banner_in_view(info, color, animate);
        return;
    }
    play_banner(&render_banner_frames(&info, terminal_width(), color), animate);
}

/// Adds the banner to the fullscreen view as a block laid out again for each width, so a split
/// pane redraws the banner instead of wrapping it. When `animate` is set, the block starts at the
/// first step of the reveal and moves one step on after each pause.
/// @param info - what the banner shows
/// @param color - true to add ANSI colors
/// @param animate - true to play the stripe reveal
fn show_banner_in_view(info: BannerInfo, color: bool, animate: bool) {
    let step = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(if animate { 0 } else { REVEAL_STEPS }));
    let shown = step.clone();
    screen::push_layout(std::sync::Arc::new(move |width| render_banner_step(&info, width, color, shown.load(std::sync::atomic::Ordering::Relaxed))));
    if !animate {
        return;
    }
    for next in 1..=REVEAL_STEPS {
        std::thread::sleep(std::time::Duration::from_millis(STEP_MS));
        step.store(next, std::sync::atomic::Ordering::Relaxed);
        screen::redraw();
    }
}

/// Shows the banner's reveal on the main screen: the first step, then each next step drawn over it
/// after a pause. Only the finished banner is printed when `animate` is off or the banner is taller
/// than the terminal.
/// @param frames - the banner's lines at each step, the finished banner last
/// @param animate - true to play the stripe reveal
fn play_banner(frames: &[Vec<String>], animate: bool) {
    let Some(last) = frames.last() else { return };
    if !animate || frames.len() < 2 || last.len() + 2 >= terminal_height() {
        emit(&last.join("\n"));
        return;
    }
    emit(&frames[0].join("\n"));
    for pair in frames.windows(2) {
        std::thread::sleep(std::time::Duration::from_millis(STEP_MS));
        emit_replacing(pair[0].len(), &pair[1].join("\n"));
    }
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
        show_event(Tone::Warning, "", &notice);
    }
    for image in &images {
        show_event(Tone::Info, "+", &format!("attached {} {}", image.source, image.describe()));
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
    let session = AgentSession { session: SessionInfo::default(), trajectory_path: PathBuf::new(), need_provider: false, events: Box::new(NoEvents) };
    let agent = build_agent(config, &cwd, &context, interaction, session)?;
    let (system, instance) = agent.render_prompts(&task)?;
    println!("===== system =====\n{system}\n\n===== user =====\n{instance}");
    Ok(())
}

/// The session parts of an agent.
pub struct AgentSession {
    pub session: SessionInfo,
    pub trajectory_path: PathBuf,
    /// False for `umbra prompt`, which never calls a model.
    pub need_provider: bool,
    /// What the front end is told as the run goes.
    pub events: Box<dyn AgentEvents>,
}

/// Builds the agent: the model, the environment, the MCP servers and the template variables.
/// @param config - the merged config
/// @param cwd - the working folder
/// @param context - the loaded instructions, skills and MCP definitions
/// @param interaction - the interactive settings
/// @param run - the session record, the trajectory file, and whether a model is needed
pub fn build_agent(config: UmbraConfig, cwd: &Path, context: &RunContext, interaction: Interaction, run: AgentSession) -> Result<Agent> {
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
    Ok(Agent::new(AgentParts { config, provider, environment, mcp, interaction, vars, trajectory_path: run.trajectory_path, config_record, cwd: cwd.to_path_buf(), session: run.session, events: run.events }))
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
        show_event(Tone::Warning, "", &format!("MCP server {} was not started: {}", failure.name, failure.error.lines().next().unwrap_or("")));
    }
    manager
}

/// Gets the task from the options, or asks for it in a terminal. A resumed session asks for
/// the next task. With keep_open an empty line asks again rather than ending the run.
/// @param args - the run options
/// @param resumed - the saved session, if resuming
/// @param keep_open - ask again after an empty line
fn task_text(args: &RunArgs, resumed: Option<&ResumedSession>, keep_open: bool) -> Result<String> {
    if let Some(task) = &args.task {
        return Ok(task.clone());
    }
    if !args.task_words.is_empty() {
        return Ok(args.task_words.join(" "));
    }
    let hint = match resumed {
        Some(r) => format!("  Resuming session {} ({} messages). What next?", r.session.id, r.messages.len()),
        None => "  What do you want to do?".to_string(),
    };
    if !stdin_is_terminal() {
        bail!("no task given. Pass it as an argument or with -t");
    }
    loop {
        match prompt_with_hint(&hint) {
            Input::Line(task) if !task.trim().is_empty() => return Ok(task),
            Input::Line(_) | Input::Interrupted if keep_open => continue,
            _ => bail!("no task given. Pass it as an argument or with -t"),
        }
    }
}

/// Prints where the trajectory was saved, and the run's totals in verbose mode. Each turn's
/// ending, final output and resume command were already printed as the turn ended. In quiet mode prints only the final output, and an
/// interrupted run says how to resume on stderr.
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
    if is_verbose() {
        let details = format!("exit status {} · {} model calls · tokens in/out {}/{} · {} compactions", result.exit_status, result.stats.api_calls, result.stats.input_tokens, result.stats.output_tokens, result.stats.compactions);
        show_event(Tone::Info, "·", &details);
    }
    emit(&paint("90", &format!("  trajectory {}", result.trajectory_path.display())));
}

/// Closes the fullscreen view when dropped, so an error that ends a run early still gives the
/// shell its screen back. Closing an already closed view does nothing.
struct LeaveViewOnDrop;

impl Drop for LeaveViewOnDrop {
    /// Closes the fullscreen view.
    fn drop(&mut self) {
        screen::leave();
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
