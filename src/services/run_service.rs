//! Puts a run together from the command line: config, context, MCP servers, environment and
//! model, then runs the agent and reports the result.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde_json::json;

use super::agent_events::{AgentEvents, NoEvents};
use super::agent_service::{is_success, now_seconds, Agent, AgentParts, NextSession, Opening, RunResult, INTERRUPTED_STATUS};
use super::command_service::{command_entries, handle_line, BuiltinCommand, LineAction};
use super::context_service::{context_vars, discovery_context, expand_slash_command, load_run_context, RunContext};
use super::terminal_events::TerminalEvents;
use super::environment::create_environment;
use super::image_service::{images_in_text, load_image};
use super::interaction_service::{prompt_with_hint, show_event, show_user_text, Interaction, Tone};
use super::mcp::McpManager;
use super::prompt_service::platform_vars;
use super::providers::{check_credentials, create_provider};
use super::session_service::{find_resume_target, load_resumed, ResumedSession};
use super::{usage_service, work_clock};
use crate::cli::{CommonArgs, PromptArgs, RunArgs};
use crate::models::config::{ActionFormat, ImagesConfig, Mode, UmbraConfig};
use crate::models::image::ImageData;
use crate::models::session::{ResumeRecord, SessionInfo};
use crate::repositories::config_repository::{load_config, merge_values};
use crate::repositories::session_repository::{find_session, new_session_id};
use crate::repositories::trajectory_repository::trajectory_path;
use crate::ui::block::Block;
use crate::ui::live::{self, HeaderInfo};
use crate::utils::banner::{display_folder, render_banner_frames, BannerInfo, STEP_MS};
use crate::utils::terminal_mode::terminal_height;
use crate::utils::console::{color_enabled, emit, emit_replacing, paint, enable_status_bar, finish_status, is_verbose, set_quiet, set_verbose, start_fullscreen, stdin_is_terminal, stdout_is_terminal, terminal_width, Input};
use crate::utils::screen::forced as fullscreen_forced;
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
    if args.no_mouse || std::env::var("UMBRA_MOUSE").is_ok_and(|v| v == "0") {
        config.mouse = false;
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
/// ending. MCP servers start before the first prompt is read and are shared by every session the
/// run moves to with `/clear` or `/resume`.
/// @param args - the run options
pub fn run_task(args: &RunArgs) -> Result<i32> {
    interrupt::install();
    let mut args = args.clone();
    // `--resume` with no id picks from a list: in the fullscreen view once it is open, else the
    // numbered list on the main screen.
    let pick = args.resume.as_deref() == Some("") && !args.continue_last && wants_fullscreen_picker(&args);
    if pick {
        args.resume = None;
    }
    let (mut resumed, mut resume_notice) = load_resume_target(&args)?;
    if let Some(resumed) = &resumed {
        fill_from_session(&mut args, &resumed.session);
    }
    let (mut config, cwd) = prepare_config(&args.common)?;
    apply_run_args(&mut config, &args)?;
    set_quiet(args.quiet);
    set_verbose(config.verbose && !args.quiet);
    crate::ui::highlight::set_enabled(config.syntax_highlight);
    enable_status_bar(config.status_bar);
    let interaction = Interaction::new(config.mode, &config.whitelist_actions, config.confirm_exit);
    if !interaction.interactive && config.mode != Mode::Yolo {
        bail!("stdin is not a terminal, so Umbra cannot ask before running commands. Pass --yolo to run without asking");
    }
    // Before the prompt, so a task is never asked for and then dropped because no model can run it.
    check_credentials(&config, &config.model)?;
    let context = load_run_context(&config, &discovery_context(&config, &cwd));
    // Closes the fullscreen view on every way out of this function, errors included.
    let _view = LeaveViewOnDrop;
    start_fullscreen(config.fullscreen && !args.no_fullscreen, mouse_wanted(&config));
    let off = resumed.as_ref().map(|r| r.session.mcp_off.clone()).unwrap_or_default();
    let mcp = start_mcp(&config, &context, &off);
    share_with_view(&config, &cwd, &context, &mcp);
    if screen::is_active() {
        super::update_service::start_background_check(&config.updates);
    }
    if crate::utils::console::status_bar_on() && screen::is_active() {
        crate::utils::console::update_status(crate::utils::status_bar::StatusInfo { model: super::providers::resolve_model(&config, &config.model), ..Default::default() });
    }
    if !args.quiet {
        show_banner(&config, &cwd, &context);
    }
    if pick {
        match pick_in_view(&cwd, args.pick_all) {
            Some(id) => (resumed, resume_notice) = resume_by_id(&id, &mut args)?,
            None => return Ok(0),
        }
    }
    loop {
        if let Some(session) = resumed.as_ref().filter(|_| args.common.model.is_none()) {
            adopt_session_model(&mut config, session);
        }
        if let Some(session) = &resumed {
            show_resumed(session, resume_notice.as_deref(), &mcp);
        }
        let opening = match first_task(&args, resumed.as_ref(), config.keep_open, &context)? {
            FirstTask::Task { typed, task } => (typed, task),
            FirstTask::Update => {
                show_event(Tone::Info, "·", "installing the newest Umbra release");
                match super::update_service::install(None, &mut |line| show_event(Tone::Info, "·", line)) {
                    Ok(_) => return restart_into(&args, resumed.as_ref().map(|r| r.session.id.as_str())),
                    Err(error) => show_event(Tone::Failure, "·", &format!("the update was not installed: {error:#}")),
                }
                continue;
            }
            FirstTask::Resume(id) => {
                (resumed, resume_notice) = resume_by_id(&id, &mut args)?;
                continue;
            }
        };
        let session = SessionRun { config: config.clone(), cwd: cwd.clone(), interaction: fresh_interaction(&config), mcp: mcp.clone() };
        let result = run_session(&args, session, &context, resumed.take(), opening)?;
        match result.next.clone() {
            Some(NextSession::Clear) => {
                crate::utils::screen::clear_transcript();
                args.task = None;
                args.task_words.clear();
                args.fork = false;
                resume_notice = None;
                show_banner(&config, &cwd, &context);
                work_clock::restore(0.0);
            }
            Some(NextSession::Resume(id)) => {
                crate::utils::screen::clear_transcript();
                show_banner(&config, &cwd, &context);
                (resumed, resume_notice) = resume_by_id(&id, &mut args)?;
            }
            Some(NextSession::Restart) => return restart_into(&args, Some(&result.session_id)),
            None => return Ok(finish_run(&result, args.quiet)),
        }
    }
}

/// Starts the newly installed binary in place of this one, resuming a session when there is one,
/// with the options this run was started with. The fullscreen view is closed first so the new
/// process gets the terminal as the shell left it. On Unix the process is replaced; elsewhere
/// the new process runs to its end and its exit code is returned.
/// @param args - this run's options
/// @param session - the session to resume, or None to start fresh
fn restart_into(args: &RunArgs, session: Option<&str>) -> Result<i32> {
    screen::leave();
    let exe = super::update_service::install_target()?;
    let argv = restart_args(args, session);
    let mut command = std::process::Command::new(&exe);
    command.args(&argv);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = command.exec();
        bail!("could not start {}: {error}", exe.display());
    }
    #[cfg(not(unix))]
    {
        let status = command.status().with_context(|| format!("could not start {}", exe.display()))?;
        Ok(status.code().unwrap_or(1))
    }
}

/// The options a restarted Umbra gets: `--resume <id>`, then every option of this run that
/// changes how the session runs. The task itself is left out, because the session already has it.
/// @param args - this run's options
/// @param session - the session to resume
pub fn restart_args(args: &RunArgs, session: Option<&str>) -> Vec<String> {
    let mut argv: Vec<String> = Vec::new();
    let mut flag = |on: bool, name: &str| if on { argv.push(name.to_string()) };
    flag(args.yolo, "--yolo");
    flag(args.no_fullscreen, "--no-fullscreen");
    flag(args.no_mouse, "--no-mouse");
    flag(args.no_status_bar, "--no-status-bar");
    flag(args.keep_open, "--keep-open");
    flag(args.verbose, "--verbose");
    flag(args.no_compact, "--no-compact");
    flag(args.common.text, "--text");
    flag(args.common.no_mcp, "--no-mcp");
    flag(args.common.no_skills, "--no-skills");
    flag(args.common.no_instructions, "--no-instructions");
    let mut pair = |name: &str, value: Option<String>| if let Some(value) = value { argv.extend([name.to_string(), value]) };
    pair("--mode", args.mode.clone());
    pair("--cost-limit", args.cost_limit.map(|v| v.to_string()));
    pair("--step-limit", args.step_limit.map(|v| v.to_string()));
    pair("--context-window", args.context_window.map(|v| v.to_string()));
    pair("--model", args.common.model.clone());
    pair("--finish", args.common.finish.clone());
    pair("--cwd", args.common.cwd.as_ref().map(|p| p.display().to_string()));
    pair("--docker", args.common.docker.clone());
    pair("--mcp", (!args.common.mcp.is_empty()).then(|| args.common.mcp.join(",")));
    pair("--no-source", (!args.common.no_source.is_empty()).then(|| args.common.no_source.join(",")));
    for file in &args.common.config {
        pair("--config", Some(file.display().to_string()));
    }
    pair("--resume", session.map(String::from));
    argv
}

/// `umbra resume`: with an id, continues that session; with none, shows the session picker. With
/// no terminal it prints the sessions and says an id is needed.
/// @param args - the resume options
pub fn resume_command(args: &crate::cli::ResumeArgs) -> Result<i32> {
    let mut run = args.run.clone();
    if let Some(id) = &args.id {
        run.resume = Some(id.clone());
        return run_task(&run);
    }
    let terminal = fullscreen_forced() || (std::io::IsTerminal::is_terminal(&std::io::stdin()) && std::io::IsTerminal::is_terminal(&std::io::stdout()));
    if !terminal {
        let cwd = match &run.common.cwd { Some(dir) => absolute_path(dir), None => std::env::current_dir()? };
        for session in super::session_service::listed_sessions(&cwd, args.all) {
            println!("{}", super::session_service::describe_session(&session));
        }
        eprintln!("umbra: a session id is needed when stdin or stdout is not a terminal: umbra resume <id>");
        return Ok(2);
    }
    run.resume = Some(String::new());
    run.pick_all = args.all;
    run_task(&run)
}

/// True when `--resume` with no id should use the fullscreen session picker: the run can open
/// the fullscreen view.
/// @param args - the run options
fn wants_fullscreen_picker(args: &RunArgs) -> bool {
    let terminal = fullscreen_forced() || (std::io::IsTerminal::is_terminal(&std::io::stdin()) && std::io::IsTerminal::is_terminal(&std::io::stdout()));
    terminal && !args.no_fullscreen && !args.quiet && std::env::var("UMBRA_FULLSCREEN").map_or(true, |v| v != "0")
}

/// Shows the session picker in the fullscreen view and waits for a choice. Returns the chosen
/// session's id, or None when the person pressed Esc or there is no session.
/// @param cwd - the folder whose sessions are listed
/// @param all - list every folder's sessions
fn pick_in_view(cwd: &Path, all: bool) -> Option<String> {
    let sessions = super::session_service::listed_sessions(cwd, all);
    if sessions.is_empty() {
        show_event(Tone::Info, "·", &format!("no saved session in {}", cwd.display()));
        return None;
    }
    screen::open_overlay(crate::ui::overlay::pickers::start_picker(sessions));
    loop {
        match crate::utils::console::prompt_line("❯ ") {
            Input::Line(line) => match line.trim().strip_prefix("/resume ") {
                Some(id) => return Some(id.trim().to_string()),
                None if line.trim() == "/quit" => return None,
                None => continue,
            },
            _ => return None,
        }
    }
}

/// Makes a resumed session use its own model, when that model can still be used. Called only
/// when the command line did not name a model.
/// @param config - the merged config, changed in place
/// @param session - the resumed session
fn adopt_session_model(config: &mut UmbraConfig, session: &ResumedSession) {
    let model = &session.session.model;
    if model.is_empty() || *model == config.model || check_credentials(config, model).is_err() {
        return;
    }
    config.model = model.clone();
    live::set_config(config.clone());
    let shown = super::providers::resolve_model(config, model);
    usage_service::start(config, model);
    live::update_header(|h| h.model = shown);
}

/// What a session of the run needs besides the options.
struct SessionRun {
    config: UmbraConfig,
    cwd: PathBuf,
    interaction: Interaction,
    mcp: Arc<McpManager>,
}

/// Runs one session to its end: builds the agent for it and runs the first task and the follow
/// ups.
/// @param args - the run options
/// @param run - the config, folder, interaction and MCP servers
/// @param context - the loaded instructions, skills and commands
/// @param resumed - the saved session, if resuming
/// @param opening - what the person typed and the task it expands to
fn run_session(args: &RunArgs, run: SessionRun, context: &RunContext, resumed: Option<ResumedSession>, opening: (String, String)) -> Result<RunResult> {
    let (typed, task) = opening;
    let images = opening_images(&args.images, &task, &run.cwd, &run.config.images)?;
    let (session, trajectory_path) = session_for_run(&run.config, args, &run.cwd, &task, resumed.as_ref());
    live::set_session(&session.id);
    work_clock::restore(session.work_seconds);
    let events: Box<dyn AgentEvents> = Box::new(TerminalEvents::default());
    let parts = AgentSession { session, trajectory_path, need_provider: true, events, mcp: Some(run.mcp) };
    let mut agent = build_agent(run.config, &run.cwd, context, run.interaction, parts)?;
    let opening = match resumed {
        Some(resumed) => Opening::Resume { session: Box::new(resumed), task, typed, images },
        None => Opening::New { task, typed, images },
    };
    agent.run(opening)
}

/// Ends the run: back on the main screen, the trajectory line, and the exit code.
/// @param result - the last session's result
/// @param quiet - print only the submission
fn finish_run(result: &RunResult, quiet: bool) -> i32 {
    // Back on the main screen first, so the trajectory line stays in the shell.
    screen::leave();
    if crate::utils::console::used_fullscreen() {
        // The bar was drawn by the view; it must not be drawn again under the trajectory line.
        finish_status(Phase::Finished);
        report(result, quiet);
    } else {
        report(result, quiet);
        finish_status(Phase::Finished);
    }
    match result.exit_status.as_str() {
        status if is_success(status) => 0,
        INTERRUPTED_STATUS => 130,
        _ => 1,
    }
}

/// Starts the MCP servers in the background, or none when MCP is off.
/// @param config - the merged config
/// @param context - the loaded MCP definitions
/// @param off - servers a resumed session had turned off
fn start_mcp(config: &UmbraConfig, context: &RunContext, off: &[String]) -> Arc<McpManager> {
    if !config.mcp.enabled || context.mcp_servers.is_empty() {
        return Arc::new(McpManager::default());
    }
    McpManager::start(&context.mcp_servers, &config.mcp, &umbra_home().join("logs").join("mcp"), off)
}

/// Gives the fullscreen view what it reads: the config, the MCP servers, the commands the popup
/// lists, and the usage service for the model.
/// @param config - the merged config
/// @param cwd - the working folder
/// @param context - the loaded commands and skills
/// @param mcp - the MCP servers
fn share_with_view(config: &UmbraConfig, cwd: &Path, context: &RunContext, mcp: &Arc<McpManager>) {
    live::set_config(config.clone());
    live::set_mcp(Some(mcp.clone()));
    live::set_commands(command_entries(context));
    live::set_mcp_origins(context.mcp_servers.iter().map(|d| (d.name.clone(), d.origin.display().to_string())).collect());
    let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned());
    let mode = serde_json::to_value(config.mode).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
    live::set_header(HeaderInfo { version: env!("CARGO_PKG_VERSION").into(), model: super::providers::resolve_model(config, &config.model), folder: display_folder(&cwd.to_string_lossy(), home.as_deref()), cwd: cwd.to_path_buf(), mode, skills: context.skills.len(), mcp_enabled: config.mcp.enabled, reveal_from: None });
    usage_service::start(config, &config.model);
}

/// Loads a saved session by id for `/resume`, and its notice.
/// @param id - the session id or the start of it
/// @param args - the run options; a resumed session keeps the run's folder and model options
fn resume_by_id(id: &str, args: &mut RunArgs) -> Result<(Option<ResumedSession>, Option<String>)> {
    let entry = find_session(id)?;
    args.resume = Some(entry.id.clone());
    args.task = None;
    args.task_words.clear();
    load_resume_target(args)
}

/// Shows a resumed session in the fullscreen view: its prompts, replies and commands, then the
/// notice that says what was loaded. Turns off the MCP servers it had turned off.
/// @param session - the saved session
/// @param notice - the notice, if any
/// @param mcp - the MCP servers
fn show_resumed(session: &ResumedSession, notice: Option<&str>, mcp: &Arc<McpManager>) {
    for name in &session.session.mcp_off {
        if mcp.snapshot().iter().any(|(n, s)| n == name && !matches!(s, super::mcp::ServerState::Off)) {
            mcp.set_enabled(name, false);
        }
    }
    if crate::utils::screen::is_active() {
        crate::utils::screen::with_transcript(|t| crate::ui::rebuild::rebuild(t, &session.messages));
    }
    if let Some(notice) = notice {
        show_event(Tone::Info, "↺", notice);
    }
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

/// Prints the umbra banner: the logo, version, model, folder, mode, skills and MCP servers. Only
/// in a terminal, unless `UMBRA_BANNER=1` asks for it in a pipe; `UMBRA_BANNER=0` or
/// `banner = false` turns it off. In a terminal the logo's stripes appear one at a time, unless
/// `UMBRA_BANNER_ANIMATION=0` turns that off. In the fullscreen view the banner is the header
/// block, drawn from live state, and the reveal runs on the redraw ticker.
/// @param config - the merged config
/// @param cwd - the working folder
/// @param context - the loaded skills and MCP definitions
fn show_banner(config: &UmbraConfig, cwd: &Path, context: &RunContext) {
    let wanted = match std::env::var("UMBRA_BANNER").ok().as_deref() {
        Some("0") => false,
        Some("1") => true,
        _ => config.banner && (stdout_is_terminal() || fullscreen_forced()),
    };
    if !wanted {
        return;
    }
    let color = color_enabled();
    let animate = color && stdout_is_terminal() && std::env::var("UMBRA_BANNER_ANIMATION").map_or(true, |v| v != "0");
    if screen::is_active() {
        live::update_header(|h| h.reveal_from = animate.then(std::time::Instant::now));
        screen::push_block(Block::Header);
        return;
    }
    let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned());
    let mode = serde_json::to_value(config.mode).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
    let mcp = match (config.mcp.enabled, context.mcp_servers.len()) {
        (false, _) => "off".to_string(),
        (true, 0) => "none".to_string(),
        (true, 1) => "1 server".to_string(),
        (true, n) => format!("{n} servers"),
    };
    let info = BannerInfo { version: env!("CARGO_PKG_VERSION").into(), model: config.model.clone(), folder: display_folder(&cwd.to_string_lossy(), home.as_deref()), mode, skills: context.skills.len(), mcp: vec![("", mcp)], model_menu: false };
    play_banner(&render_banner_frames(&info, terminal_width(), color), animate);
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
        let session = SessionInfo { id: new_session_id(), cwd: cwd_text, model: config.model.clone(), task: task.chars().take(200).collect(), started_at: now_seconds(), ..Default::default() };
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
    let session = AgentSession { session: SessionInfo::default(), trajectory_path: PathBuf::new(), need_provider: false, events: Box::new(NoEvents), mcp: None };
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
    /// MCP servers already started, or None to start them here. A run with no terminal waits
    /// for them either way.
    pub mcp: Option<Arc<McpManager>>,
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
    // ACP never asks stdin: on Windows that waits while the reader thread reads the same pipe.
    let at_terminal = interaction.interactive && (screen::is_active() || (std::io::IsTerminal::is_terminal(&std::io::stdin()) && std::io::IsTerminal::is_terminal(&std::io::stdout())));
    let mcp = connect_mcp(&config, context, run.mcp, at_terminal);
    mcp.set_image_settings(config.images.clone());
    let mut vars = platform_vars();
    merge_values(&mut vars, environment.template_vars());
    merge_values(&mut vars, context_vars(context));
    vars["mcp_instructions"] = json!(mcp.instructions().into_iter().map(|(name, instructions)| json!({ "name": name, "instructions": instructions })).collect::<Vec<_>>());
    vars["model_name"] = json!(provider.model_name());
    let config_record = json!({ "agent": { "mode": config.mode, "action_format": config.action_format, "step_limit": config.step_limit, "cost_limit": config.cost_limit, "compaction": config.compaction }, "model": provider.describe(), "environment": environment.describe(), "mcp_servers": mcp.connected(), "skills": context.skills.len(), "instruction_files": context.instructions.iter().map(|i| i.path.display().to_string()).collect::<Vec<_>>() });
    Ok(Agent::new(AgentParts { config, provider, environment, mcp, interaction, context: context.clone(), vars, trajectory_path: run.trajectory_path, config_record, cwd: cwd.to_path_buf(), session: run.session, events: run.events }))
}

/// The MCP servers for an agent: the ones already started, or new ones. A run with no terminal
/// (a pipe, `-q`, ACP) waits until every server has answered or the start timeout passed, so its
/// first model call has every tool, and says which servers did not start. A run with a terminal
/// never waits.
/// @param config - the merged config
/// @param context - the loaded MCP definitions
/// @param started - servers started earlier, if any
/// @param at_terminal - true in the fullscreen view or with stdin and stdout on a terminal
fn connect_mcp(config: &UmbraConfig, context: &RunContext, started: Option<Arc<McpManager>>, at_terminal: bool) -> Arc<McpManager> {
    let manager = match started {
        Some(manager) => manager,
        None if !config.mcp.enabled || context.mcp_servers.is_empty() => return Arc::new(McpManager::default()),
        None => McpManager::start(&context.mcp_servers, &config.mcp, &umbra_home().join("logs").join("mcp"), &[]),
    };
    if at_terminal {
        return manager;
    }
    manager.wait_settled(std::time::Instant::now() + std::time::Duration::from_secs(config.mcp.startup_timeout_seconds.max(1) + 2));
    for failure in manager.failures() {
        show_event(Tone::Warning, "", &format!("MCP server {} was not started: {}", failure.name, failure.error.lines().next().unwrap_or("")));
    }
    manager
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

/// True when the fullscreen view turns mouse reporting on: `mouse = true`, except in a classic
/// Windows console window, where the mouse reports could not be confirmed (task-2611), so the
/// console keeps quick edit and selects text itself. `UMBRA_MOUSE=1` turns it on there too.
/// @param config - the merged config
fn mouse_wanted(config: &UmbraConfig) -> bool {
    if !config.mouse {
        return false;
    }
    std::env::var("UMBRA_MOUSE").is_ok_and(|v| v == "1") || !crate::utils::terminal_mode::in_classic_console_window()
}

/// The interactive settings for a new session of the run.
/// @param config - the merged config
fn fresh_interaction(config: &UmbraConfig) -> Interaction {
    let mut interaction = Interaction::new(config.mode, &config.whitelist_actions, config.confirm_exit);
    interaction.keep_open = config.keep_open;
    interaction
}

/// The first prompt of a session: a task, or a session to resume instead.
enum FirstTask {
    /// What the person typed and the task it expands to.
    Task { typed: String, task: String },
    /// `/resume <id>` at the first prompt.
    Resume(String),
    /// `/update` at the first prompt.
    Update,
}

/// Gets the first task from the options, or asks for it in a terminal. A resumed session asks
/// for the next task. With keep_open an empty line asks again rather than ending the run. A slash
/// command or skill is expanded; a built in command typed here runs, or says where it works.
/// @param args - the run options
/// @param resumed - the saved session, if resuming
/// @param keep_open - ask again after an empty line
/// @param context - the loaded commands and skills
fn first_task(args: &RunArgs, resumed: Option<&ResumedSession>, keep_open: bool, context: &RunContext) -> Result<FirstTask> {
    let given = args.task.clone().or_else(|| (!args.task_words.is_empty()).then(|| args.task_words.join(" ")));
    if let Some(typed) = given {
        show_user_text("❯", &typed);
        let task = expand_slash_command(&typed, context);
        return Ok(FirstTask::Task { typed, task });
    }
    let hint = match resumed {
        Some(r) => format!("  Resuming session {} ({} messages). What next?", r.session.id, r.messages.len()),
        None => "  What do you want to do?".to_string(),
    };
    if !stdin_is_terminal() {
        bail!("no task given. Pass it as an argument or with -t");
    }
    loop {
        let typed = match prompt_with_hint(&hint) {
            Input::Line(task) if !task.trim().is_empty() => task,
            Input::Line(_) | Input::Interrupted if keep_open => continue,
            _ => bail!("no task given. Pass it as an argument or with -t"),
        };
        // A test hook: this exact line makes the run panic, to check the panic path leaves the
        // terminal usable.
        if std::env::var("UMBRA_TEST_PANIC").is_ok_and(|v| v == typed) {
            panic!("a test panic");
        }
        match handle_line(&typed, context) {
            LineAction::SendToModel(task) => return Ok(FirstTask::Task { typed, task }),
            LineAction::Builtin(BuiltinCommand::Resume(Some(id))) => return Ok(FirstTask::Resume(id)),
            LineAction::Builtin(BuiltinCommand::Quit) => bail!("no task given"),
            LineAction::Builtin(BuiltinCommand::Update) => return Ok(FirstTask::Update),
            LineAction::Builtin(BuiltinCommand::Clear | BuiltinCommand::Compact(_)) => show_event(Tone::Info, "·", "nothing to clear or compact yet"),
            LineAction::Builtin(_) => show_event(Tone::Info, "·", "that command opens in the fullscreen view"),
        }
    }
}
