//! The agent loop. It follows mini-swe-agent's DefaultAgent: one linear list of messages, a
//! model call per step, the actions run in order and the observations appended. In the reply
//! style (the default) a turn ends when the model replies without a command, as in Claude Code.
//! In the submit style it ends when a command prints COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT.
//! Either way a limit also ends it.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use serde_json::{json, Value};

use super::action_parser::{parse_text_reply, parse_tool_reply, Parsed};
use super::agent_events::AgentEvents;
use super::command_service::{handle_line, BuiltinCommand, LineAction};
use super::context_service::RunContext;
use super::session_title_service::{initial_title, request_title, summary_from_compaction};
use super::{usage_service, work_clock};
use super::compaction_service::{apply_mask, budget_for, estimate_history_tokens, estimate_request_tokens, estimate_tools_tokens, find_cut, mask_record, plan_mask, resolve_window, summarizable_tokens, summarize, summary_record, ReportedSize, SummaryInputs};
use super::environment::{submission_from, Environment};
use super::image_service::images_in_text;
use super::interaction_service::{show_assistant, show_event, show_observation, show_reply_text, show_turn_end, show_user_text, Comment, Decision, HumanInput, Interaction, Tone, TurnSummary};
use super::session_service::ResumedSession;
use super::tool_search_service::{describe_found, is_tool_search, max_results, search_tools, should_defer, tool_search_spec, tool_search_tool};
use super::mcp::McpManager;
use super::prompt_service::{render, render_format_error, render_observation, Templates};
use super::providers::anthropic::RAW_CONTENT_KEY;
use super::providers::{Provider, ProviderError, ToolSpec};
use crate::models::action::{Action, Observation};
use crate::models::compaction::{CompactionKind, CompactionReason, CompactionRecord};
use crate::models::config::{ActionFormat, Finish, Mode, UmbraConfig};
use crate::models::context::McpTool;
use crate::models::image::ImageData;
use crate::models::message::{Message, Role};
use crate::models::model_response::ModelResponse;
use crate::models::session::{SessionIndexEntry, SessionInfo};
use crate::models::trajectory::{ModelStats, Trajectory, TrajectoryInfo};
use crate::repositories::session_repository::save_index;
use crate::repositories::trajectory_repository::save_trajectory;
use crate::utils::console;
use crate::utils::interrupt;
use crate::utils::paths::absolute_path;
use crate::utils::status_bar::{short_tokens, Phase, StatusInfo};
use crate::ui::live::{self, UiRequest};

/// How a run ended.
#[derive(Debug, Clone)]
pub struct RunResult {
    /// `Submitted`, `LimitsExceeded`, `TimeExceeded`, `RepeatedFormatError`, `ModelError`, `UserQuit` or `Interrupted`.
    pub exit_status: String,
    pub submission: String,
    pub trajectory_path: PathBuf,
    pub stats: ModelStats,
    pub session_id: String,
    /// What the person asked for next with `/clear` or `/resume`, when the run ended for it.
    pub next: Option<NextSession>,
}

/// The session a run hands over to when it ends for `/clear` or `/resume`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextSession {
    /// A new session with an empty history.
    Clear,
    /// The saved session with this id.
    Resume(String),
    /// A newer Umbra was installed: start it and resume this session.
    Restart,
}

/// How a run starts: a new task, or a saved session with the next task.
pub enum Opening {
    /// `task` is what the model gets; `typed` is what the person typed, such as `/review x` for a
    /// custom command, kept as `user_text`.
    New { task: String, typed: String, images: Vec<ImageData> },
    Resume { session: Box<ResumedSession>, task: String, typed: String, images: Vec<ImageData> },
}

/// The exit status of a run the person ended with Ctrl+C.
pub const INTERRUPTED_STATUS: &str = "Interrupted";

/// The exit status of a turn the model ended by replying without a command. It is a successful
/// ending like `Submitted`, but the answer is already shown, so it is not shown again.
pub const REPLIED_STATUS: &str = "Replied";

/// Returns true for the exit statuses that mean the turn finished its work.
/// @param status - the exit status
pub fn is_success(status: &str) -> bool {
    status == "Submitted" || status == REPLIED_STATUS
}

/// The result given to actions that did not run because the person pressed Ctrl+C.
pub const NOT_RUN_INTERRUPTED: &str = "This action was not executed because the user interrupted the run (Ctrl+C).";

/// What one step decided.
enum Step {
    Continue,
    Exit(String, String),
}

/// Everything a run needs, built by the caller.
pub struct AgentParts {
    pub config: UmbraConfig,
    pub provider: Box<dyn Provider>,
    pub environment: Box<dyn Environment>,
    pub mcp: Arc<McpManager>,
    pub interaction: Interaction,
    /// The custom commands and skills, so a slash command works on every prompt.
    pub context: RunContext,
    /// Variables for the templates: platform, environment, context and config values.
    pub vars: Value,
    pub trajectory_path: PathBuf,
    /// The config section of the trajectory.
    pub config_record: Value,
    /// The working folder, for image paths typed at a prompt.
    pub cwd: PathBuf,
    /// What the trajectory records about the session.
    pub session: SessionInfo,
    /// What the front end is told as the run goes. The terminal passes `NoEvents`.
    pub events: Box<dyn AgentEvents>,
}

/// How one turn ended: the exit status and the final output.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnEnd {
    /// `Submitted`, `LimitsExceeded`, `TimeExceeded`, `RepeatedFormatError`, `ModelError`, `UserQuit` or `Interrupted`.
    pub status: String,
    pub submission: String,
}

/// A run in progress.
pub struct Agent {
    parts: AgentParts,
    templates: Templates,
    tools: Vec<ToolSpec>,
    /// Every tool the MCP servers offer.
    mcp_tools: Vec<McpTool>,
    /// True when the MCP tools are sent by name only and loaded with `tool_search`.
    deferred: bool,
    /// The deferred tools loaded so far, by full name.
    loaded: BTreeSet<String>,
    messages: Vec<Message>,
    stats: ModelStats,
    format_errors: u32,
    started: Instant,
    exit: (String, String),
    compaction: CompactionState,
    /// The model calls and cost already spent when this run started; the limits count from here.
    baseline: (u32, f64),
    /// The MCP manager's version when the tools were last read.
    mcp_version: u64,
    /// Set by `/clear` or `/resume` at the follow up prompt.
    next: Option<NextSession>,
    /// Set by `/quit` typed while the agent works: the run ends before the next step.
    quit_requested: bool,
    /// Set by `/update` typed while the agent works: the update runs when the turn ends.
    update_after_turn: bool,
    /// The MCP servers whose failure was already printed in the inline view.
    announced_failures: BTreeSet<String>,
    /// The background request for the session's title, after the first turn.
    title_job: Option<std::thread::JoinHandle<Option<(String, String, f64)>>>,
    /// True once the first turn has ended, so the title is asked for once.
    titled: bool,
}

/// What the agent tracks for compaction.
#[derive(Default)]
struct CompactionState {
    /// The context window, looked up on first use.
    window: Option<u64>,
    /// The size of the last request as the provider reported it. None after a compaction.
    reported: Option<ReportedSize>,
    /// The estimate for the tool definitions, added when there is no reported size.
    tools_tokens: u64,
    records: Vec<CompactionRecord>,
    /// True once the person was told compaction cannot shrink the history, so it is said once.
    stuck_warned: bool,
}

impl Agent {
    /// Prepares a run: picks the templates, decides whether the MCP tools are deferred behind
    /// `tool_search` (when their definitions would take a large share of the context window), and
    /// builds the tool list for tool mode.
    /// @param parts - the provider, environment, MCP servers and settings
    pub fn new(mut parts: AgentParts) -> Self {
        let templates = Templates::resolve(&parts.config.templates, parts.config.action_format, parts.config.finish);
        let mcp_tools = parts.mcp.ready_tools();
        let mcp_version = parts.mcp.version();
        let window = resolve_window(&parts.config.compaction, parts.provider.as_mut());
        let mcp_tokens = estimate_tools_tokens(&tool_specs(&mcp_tools)[1..]);
        let mcp = &parts.config.mcp;
        let deferred = !mcp_tools.is_empty() && should_defer(mcp.defer_tools, mcp_tokens, window, mcp.defer_ratio);
        let compaction = CompactionState { window: Some(window), ..Default::default() };
        let mut agent = Agent { parts, templates, tools: Vec::new(), mcp_tools, deferred, loaded: BTreeSet::new(), messages: Vec::new(), stats: ModelStats::default(), format_errors: 0, started: Instant::now(), exit: (String::new(), String::new()), compaction, baseline: (0, 0.0), mcp_version, next: None, quit_requested: false, update_after_turn: false, announced_failures: BTreeSet::new(), title_job: None, titled: false };
        agent.rebuild_tools();
        agent
    }

    /// Builds the tools sent with each request: `bash`, then every MCP tool, or with deferral
    /// `bash`, `tool_search` and only the MCP tools loaded so far. Text mode sends no tools.
    fn rebuild_tools(&mut self) {
        self.tools = if self.parts.config.action_format != ActionFormat::Tools {
            Vec::new()
        } else if self.deferred {
            let loaded: Vec<McpTool> = self.mcp_tools.iter().filter(|t| self.loaded.contains(&t.qualified_name())).cloned().collect();
            let mut specs = tool_specs(&loaded);
            specs.insert(1, tool_search_spec());
            specs
        } else {
            tool_specs(&self.mcp_tools)
        };
        self.compaction.tools_tokens = estimate_tools_tokens(&self.tools);
    }

    /// Reads the tools again when an MCP server became ready, failed or was turned on or off since
    /// the last model call. A server that became ready is announced, and deferral is decided again
    /// for the new set of tools.
    fn refresh_mcp(&mut self) {
        let version = self.parts.mcp.version();
        if version == self.mcp_version {
            return;
        }
        self.mcp_version = version;
        let tools = self.parts.mcp.ready_tools();
        let before: BTreeSet<String> = self.mcp_tools.iter().map(|t| t.server.clone()).collect();
        for (server, _, count) in self.parts.mcp.connected() {
            if !before.contains(&server) && self.parts.interaction.interactive {
                self.notice(Tone::Info, "mcp", &format!("mcp {server} ready, {count} {}", if count == 1 { "tool" } else { "tools" }));
            }
        }
        // The fullscreen view shows failures in its error box; the inline view prints each once.
        if !crate::utils::screen::is_active() && self.parts.interaction.interactive {
            for failure in self.parts.mcp.failures() {
                if self.announced_failures.insert(failure.name.clone()) {
                    show_event(Tone::Warning, "", &format!("MCP server {} was not started: {}", failure.name, failure.error.lines().next().unwrap_or("")));
                }
            }
        }
        let window = self.context_window();
        let mcp = &self.parts.config.mcp;
        let tokens = estimate_tools_tokens(&tool_specs(&tools)[1..]);
        self.deferred = !tools.is_empty() && should_defer(mcp.defer_tools, tokens, window, mcp.defer_ratio);
        self.mcp_tools = tools;
        self.rebuild_tools();
        self.compaction.reported = None;
    }

    /// Applies what the fullscreen view asked for since the last step: another model, another
    /// mode, or the end of the session.
    fn apply_ui_requests(&mut self) {
        self.apply_control_requests();
        if !self.parts.interaction.interactive {
            return;
        }
        for request in live::take_requests() {
            match request {
                UiRequest::SetModel(spec) => self.switch_model(&spec),
                UiRequest::SetMode(mode) => self.parts.interaction.mode = mode,
                UiRequest::Quit => self.quit_requested = true,
                UiRequest::ApplyConfig(config) => self.apply_config(*config),
            }
        }
    }

    /// Applies the commands `umbra session` sent to this session through its control folder, then
    /// writes the session's state again so `umbra session status` sees the result.
    fn apply_control_requests(&mut self) {
        use super::control_service::ControlRequest;
        let requests = super::control_service::take_requests();
        if requests.is_empty() {
            return;
        }
        for request in requests {
            match request {
                ControlRequest::Model { name } => self.switch_model(&name),
                ControlRequest::Mode { mode } => self.running_command(BuiltinCommand::Mode(Some(mode))),
                ControlRequest::Mcp { name, on } => {
                    self.parts.mcp.set_enabled(&name, on);
                    self.notice(Tone::Info, "mcp", &format!("MCP server {name} {} for this session", if on { "on" } else { "off" }));
                }
                ControlRequest::Compact { focus } => {
                    self.compact_by_summary(CompactionReason::Manual, &focus);
                }
                ControlRequest::Quit => self.quit_requested = true,
            }
        }
        self.publish_state();
    }

    /// Writes what the session uses now to its control folder, for `umbra session status`.
    fn publish_state(&self) {
        let mcp = self.parts.mcp.snapshot().into_iter().map(|(name, state)| {
            let word = match state {
                super::mcp::ServerState::Off => "off",
                super::mcp::ServerState::Starting { .. } => "starting",
                super::mcp::ServerState::Ready { .. } => "running",
                super::mcp::ServerState::Failed { .. } => "failed",
            };
            (name, word.to_string())
        }).collect();
        let state = super::control_service::SessionState { id: self.parts.session.id.clone(), cwd: self.parts.session.cwd.clone(), model: self.parts.config.model.clone(), mode: crate::ui::overlay::pickers::mode_name(self.parts.interaction.mode).to_string(), mcp, ..Default::default() };
        super::control_service::write_state(state);
    }

    /// Applies settings saved in the configuration modal. Settings that take effect in the open
    /// session replace the running ones; settings that apply to the next session keep their
    /// running values. A change to the model or its provider makes a new provider.
    /// @param saved - the config with the saved changes
    fn apply_config(&mut self, saved: UmbraConfig) {
        let old = self.parts.config.clone();
        let mut next = saved;
        next.action_format = old.action_format;
        next.finish = old.finish;
        next.templates = old.templates.clone();
        next.environment = old.environment.clone();
        next.mcp = old.mcp.clone();
        next.mcp_servers = old.mcp_servers.clone();
        next.sources = old.sources.clone();
        next.instructions = old.instructions.clone();
        next.skills = old.skills.clone();
        next.commands = old.commands.clone();
        next.fullscreen = old.fullscreen;
        next.mouse = old.mouse;
        let provider_changed = next.model != old.model || serde_json::to_value(&next.providers).ok() != serde_json::to_value(&old.providers).ok() || next.models != old.models;
        self.parts.interaction.mode = next.mode;
        console::set_verbose(next.verbose);
        if next.syntax_highlight != old.syntax_highlight {
            crate::ui::highlight::set_enabled(next.syntax_highlight);
            crate::utils::screen::with_transcript(|t| t.forget_layouts());
        }
        let model = next.model.clone();
        self.parts.config = next;
        self.compaction.window = None;
        if provider_changed {
            self.switch_model(&model);
        }
        self.show_status(Phase::Waiting);
    }

    /// Switches to another model from the next model call, after checking it can be used.
    /// @param spec - a model name from `[models]` or a `provider/model` string
    pub fn switch_model(&mut self, spec: &str) {
        let resolved = super::providers::resolve_model(&self.parts.config, spec);
        let checked = super::providers::check_credentials(&self.parts.config, spec).and_then(|_| super::providers::create_provider(&self.parts.config, spec));
        match checked {
            Ok(provider) => {
                self.set_provider(provider, &resolved);
                usage_service::start(&self.parts.config, &resolved);
                live::update_header(|h| h.model = resolved.clone());
            }
            Err(error) => self.notice(Tone::Failure, "model", &format!("could not switch to {spec}: {error:#}")),
        }
    }

    /// Loads deferred tools by their full names, so their definitions go with every later request.
    /// The size the provider last reported no longer counts the tools, so it is dropped.
    /// @param names - the `mcp__server__tool` names
    /// @param announce - true to say at the terminal which tools were loaded
    fn load_tools(&mut self, names: Vec<String>, announce: bool) {
        if !self.deferred {
            return;
        }
        let new: Vec<String> = names.into_iter().filter(|n| !self.loaded.contains(n) && self.mcp_tools.iter().any(|t| t.qualified_name() == *n)).collect();
        if new.is_empty() {
            return;
        }
        if announce {
            self.notice(Tone::Info, "tools", &format!("loaded {}", new.join(", ")));
        }
        self.loaded.extend(new);
        self.rebuild_tools();
        self.compaction.reported = None;
    }

    /// Runs `tool_search`: finds the deferred tools the query names, loads them, and returns
    /// their definitions for the model to read.
    /// @param arguments - the call's arguments: `query`, and `max_results`
    fn run_tool_search(&mut self, arguments: &Value) -> Observation {
        let query = arguments.get("query").and_then(Value::as_str).unwrap_or("");
        let found = search_tools(query, &self.mcp_tools, max_results(arguments));
        let output = describe_found(&found.iter().map(|&i| &self.mcp_tools[i]).collect::<Vec<_>>(), query);
        let names = found.iter().map(|&i| self.mcp_tools[i].qualified_name()).collect();
        self.load_tools(names, true);
        Observation { output, returncode: if found.is_empty() { 1 } else { 0 }, exception_info: String::new(), ..Default::default() }
    }

    /// The tools a reply may call: every MCP tool, and `tool_search` when tools are deferred.
    /// A deferred tool that was never loaded can still be called; it is loaded when it runs.
    fn callable_tools(&self) -> Vec<McpTool> {
        let mut tools = self.mcp_tools.clone();
        if self.deferred {
            tools.push(tool_search_tool());
        }
        tools
    }

    /// Loads again the deferred tools a resumed session had loaded: the ones its searches found
    /// and the ones it called.
    fn restore_loaded_tools(&mut self) {
        if !self.deferred {
            return;
        }
        let mut names = Vec::new();
        for message in self.messages.iter().filter(|m| m.role == Role::Assistant) {
            let actions: Vec<Action> = serde_json::from_value(message.extra["actions"].clone()).unwrap_or_default();
            for action in actions {
                if let Action::Mcp { server, tool, arguments, .. } = action {
                    if is_tool_search(&server, &tool) {
                        let query = arguments.get("query").and_then(Value::as_str).unwrap_or("");
                        names.extend(search_tools(query, &self.mcp_tools, max_results(&arguments)).into_iter().map(|i| self.mcp_tools[i].qualified_name()));
                    } else {
                        names.push(mcp_name(&server, &tool));
                    }
                }
            }
        }
        self.load_tools(names, false);
    }

    /// Renders the system prompt for a task without running anything, for `umbra prompt`.
    /// @param task - the task text
    pub fn render_prompts(&self, task: &str) -> Result<(String, String)> {
        let vars = self.template_vars(task);
        Ok((render(&self.templates.system, &vars)?, render(&self.templates.instance, &vars)?))
    }

    /// Runs the task to the end and returns how it ended. The trajectory is saved after every step.
    /// @param opening - a new task, or a saved session and the next task
    pub fn run(&mut self, opening: Opening) -> Result<RunResult> {
        match opening {
            Opening::New { task, typed, images } => self.open_new_typed(&task, &typed, images)?,
            Opening::Resume { session, task, typed, images } => {
                self.open_resumed(*session);
                self.add_task_typed(&task, &typed, images);
            }
        }
        self.save();
        super::control_service::open(&self.parts.session.id);
        self.publish_state();
        self.show_status(Phase::Starting);
        // Read the terminal from the start, so a line typed while the first steps run is queued
        // for the next model call rather than left waiting until the run ends.
        if self.parts.interaction.interactive {
            console::start_reading_stdin();
        }
        loop {
            if self.parts.interaction.interactive {
                work_clock::start();
            }
            let end = self.run_turn();
            work_clock::stop();
            self.after_turn(&end);
            self.show_turn_end(&end);
            if is_success(&end.status) && self.follow_up() {
                continue;
            }
            self.end_turn(&end);
            break;
        }
        self.finish_title_job(true);
        self.save();
        self.close();
        Ok(RunResult { exit_status: self.exit.0.clone(), submission: self.exit.1.clone(), trajectory_path: self.parts.trajectory_path.clone(), stats: self.stats.clone(), session_id: self.parts.session.id.clone(), next: self.next.clone() })
    }

    /// After a turn: the usage is polled again, and after the first turn of a session in a
    /// terminal a background request asks the model for the session's title and summary.
    /// @param end - how the turn ended
    fn after_turn(&mut self, end: &TurnEnd) {
        usage_service::turn_ended();
        if self.titled || !self.parts.interaction.interactive {
            return;
        }
        self.titled = true;
        if !self.parts.config.sessions.auto_title || !self.parts.session.summary.is_empty() {
            return;
        }
        let prompt = self.messages.iter().find_map(|m| m.extra["user_text"].as_str().map(String::from)).unwrap_or_default();
        let reply = if end.submission.is_empty() { self.messages.iter().rev().find(|m| m.role == Role::Assistant).map(|m| m.content.clone()).unwrap_or_default() } else { end.submission.clone() };
        let Ok(mut provider) = super::providers::create_provider(&self.parts.config, &self.parts.config.model) else { return };
        self.title_job = Some(std::thread::spawn(move || request_title(provider.as_mut(), &prompt, &reply)));
    }

    /// Takes the title request's answer when it has arrived, or waits for it when `wait` is set.
    /// A failed request leaves the title from the first prompt.
    /// @param wait - true to wait for the request to finish
    fn finish_title_job(&mut self, wait: bool) {
        if !self.title_job.as_ref().is_some_and(|job| wait || job.is_finished()) {
            return;
        }
        let Some(job) = self.title_job.take() else { return };
        if let Ok(Some((title, summary, cost))) = job.join() {
            self.parts.session.title = title;
            self.parts.session.summary = summary;
            self.stats.instance_cost += cost;
        }
    }

    /// Starts a new session: renders the system prompt and the first message. The person's own
    /// words are kept in `extra.user_text`, so a front end can show them again later.
    /// @param task - the task text
    /// @param images - the images attached to the task
    pub fn open_new(&mut self, task: &str, images: Vec<ImageData>) -> Result<()> {
        self.open_new_typed(task, task, images)
    }

    /// Starts a new session whose first task was expanded from what the person typed, such as a
    /// custom command. The typed text is kept as `user_text` and names the session.
    /// @param task - the task text the model gets
    /// @param typed - what the person typed
    /// @param images - the images attached to the task
    pub fn open_new_typed(&mut self, task: &str, typed: &str, images: Vec<ImageData>) -> Result<()> {
        let (system, instance) = self.render_prompts(task)?;
        self.messages = vec![Message::system(system), Message::user(instance).with_images(images).with_extra(json!({ "user_text": typed }))];
        if self.parts.session.title.is_empty() {
            self.parts.session.title = initial_title(typed);
        }
        Ok(())
    }

    /// Continues a saved session from its history and totals.
    /// @param session - the saved session
    pub fn open_resumed(&mut self, session: ResumedSession) {
        self.messages = session.messages;
        self.stats = session.stats;
        self.compaction.records = session.compactions;
        self.baseline = (self.stats.api_calls, self.stats.instance_cost);
        self.restore_loaded_tools();
    }

    /// Gives the model its next task, the way a follow up task is given. An exit marker left by
    /// the previous turn is removed first, so the history stays one conversation.
    /// @param task - the task text
    /// @param images - the images attached to it
    pub fn add_task(&mut self, task: &str, images: Vec<ImageData>) {
        self.add_task_typed(task, task, images);
    }

    /// Gives the model its next task, expanded from what the person typed.
    /// @param task - the task text the model gets
    /// @param typed - what the person typed, kept as `user_text`
    /// @param images - the images attached to it
    pub fn add_task_typed(&mut self, task: &str, typed: &str, images: Vec<ImageData>) {
        while self.messages.last().is_some_and(|m| m.role == Role::Exit) {
            self.messages.pop();
        }
        if self.parts.session.title.is_empty() && !typed.trim().is_empty() {
            self.parts.session.title = initial_title(typed);
        }
        self.messages.push(new_task_message(task, typed, self.parts.config.finish).with_images(images));
    }

    /// Makes the step, cost and time limits count from now, for a front end that runs one turn
    /// per request.
    pub fn start_turn_limits(&mut self) {
        self.baseline = (self.stats.api_calls, self.stats.instance_cost);
        self.started = Instant::now();
        self.format_errors = 0;
    }

    /// Runs steps until one ends the run, saving after each, and returns how it ended. It does
    /// not ask for a follow up task and does not add the exit marker; `end_turn` does that.
    pub fn run_turn(&mut self) -> TurnEnd {
        loop {
            let ended = match self.step() {
                Ok(Step::Continue) => None,
                Ok(Step::Exit(status, submission)) => Some(TurnEnd { status, submission }),
                Err(error) => Some(TurnEnd { status: "ModelError".into(), submission: format!("{error:#}") }),
            };
            self.save();
            if let Some(end) = ended {
                return end;
            }
        }
    }

    /// Records how a turn ended: the exit marker in the history, then a save.
    /// @param end - how the turn ended
    pub fn end_turn(&mut self, end: &TurnEnd) {
        self.messages.push(Message::exit(&end.status, &end.submission));
        self.exit = (end.status.clone(), end.submission.clone());
        self.save();
    }

    /// Prints how a turn ended at the terminal, with the session's steps and cost.
    /// @param end - how the turn ended
    fn show_turn_end(&mut self, end: &TurnEnd) {
        self.show_status(Phase::Waiting);
        show_turn_end(&TurnSummary { status: &end.status, submission: &end.submission });
    }

    /// Stops the environment and the MCP servers.
    pub fn close(&mut self) {
        work_clock::stop();
        super::control_service::close();
        self.parts.environment.close();
        self.parts.mcp.close();
    }

    /// Replaces the model, for a front end that lets the person pick another one.
    /// @param provider - the new model
    /// @param model - its `provider/model` string, recorded in the session
    pub fn set_provider(&mut self, provider: Box<dyn Provider>, model: &str) {
        self.parts.provider = provider;
        self.parts.config.model = model.to_string();
        self.parts.session.model = model.to_string();
        self.compaction.window = None;
        self.compaction.reported = None;
    }

    /// The conversation so far.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Names the session after its first task, for listings, when it has no name yet.
    /// @param task - the person's words
    pub fn name_session(&mut self, task: &str) {
        if self.parts.session.task.is_empty() {
            self.parts.session.task = task.chars().take(200).collect();
        }
    }

    /// After a submission, asks the person at the terminal for a follow up task, which continues
    /// the same conversation. `/compact` compacts and asks again. Returns true when there is one.
    fn follow_up(&mut self) -> bool {
        self.show_status(Phase::Waiting);
        if std::mem::take(&mut self.update_after_turn) && self.run_update() {
            return false;
        }
        while let Some(line) = self.parts.interaction.follow_up_task() {
            self.apply_ui_requests();
            let task = match handle_line(&line, &self.parts.context) {
                LineAction::SendToModel(task) => task,
                LineAction::Builtin(command) => match self.idle_command(command) {
                    Some(true) => return false,
                    _ => continue,
                },
            };
            let images = self.typed_images(&line);
            self.add_task_typed(&task, &line, images);
            return true;
        }
        false
    }

    /// Runs a built in command typed at the follow up prompt. Returns Some(true) when the run
    /// ends for it, Some(false) when it was handled. The commands that only open something on
    /// screen are run by the fullscreen view; in the inline view they say so.
    /// @param command - the command
    fn idle_command(&mut self, command: BuiltinCommand) -> Option<bool> {
        match command {
            BuiltinCommand::Compact(focus) => {
                self.compact_by_summary(CompactionReason::Manual, &focus);
            }
            BuiltinCommand::Clear => {
                self.next = Some(NextSession::Clear);
                return Some(true);
            }
            BuiltinCommand::Resume(Some(id)) => {
                self.next = Some(NextSession::Resume(id));
                return Some(true);
            }
            BuiltinCommand::Quit => return Some(true),
            BuiltinCommand::Update => {
                if self.run_update() {
                    return Some(true);
                }
            }
            other => self.running_command(other),
        }
        Some(false)
    }

    /// Installs the newest release with the session saved first. Returns true when it was
    /// installed and the run should end so the new binary starts and resumes this session. A
    /// failure is shown and the session carries on with the old binary.
    fn run_update(&mut self) -> bool {
        self.save();
        self.notice(Tone::Info, "update", "installing the newest Umbra release");
        let result = super::update_service::install(None, &mut |line| show_event(Tone::Info, "·", line));
        match result {
            Ok(version) => {
                self.notice(Tone::Info, "update", &format!("installed {version}; restarting into this session"));
                self.next = Some(NextSession::Restart);
                true
            }
            Err(error) => {
                self.notice(Tone::Failure, "update", &format!("the update was not installed: {error:#}"));
                false
            }
        }
    }

    /// Runs a built in command that works the same while the agent is idle or working: the mode
    /// and the model. Any other command says where it works.
    /// @param command - the command
    fn running_command(&mut self, command: BuiltinCommand) {
        match command {
            BuiltinCommand::Mode(Some(word)) => match crate::ui::overlay::pickers::parse_mode(&word) {
                Some(mode) => {
                    self.parts.interaction.mode = mode;
                    live::update_header(|h| h.mode = word.clone());
                    self.notice(Tone::Info, "mode", &format!("{word} mode"));
                }
                None => self.notice(Tone::Warning, "mode", &format!("unknown mode '{word}'. Use confirm, yolo or human")),
            },
            BuiltinCommand::Model(Some(name)) => self.switch_model(&name),
            BuiltinCommand::Help => self.notice(Tone::Info, "help", &super::command_service::help_text()),
            BuiltinCommand::Update => {
                self.update_after_turn = true;
                self.notice(Tone::Info, "update", "Umbra updates and restarts when this turn ends");
            }
            BuiltinCommand::Clear | BuiltinCommand::Resume(_) => self.notice(Tone::Warning, "command", "wait for the agent to finish, or press Ctrl+C, before starting another session"),
            other => self.notice(Tone::Info, "command", &format!("{} opens in the fullscreen view", command_name(&other))),
        }
    }

    /// One step: a model call (or a typed command in human mode), then the actions. A Ctrl+C
    /// at any point stops the step and shows the comment prompt.
    fn step(&mut self) -> Result<Step> {
        if interrupt::take() {
            return self.interrupted();
        }
        if self.parts.interaction.interactive {
            work_clock::start();
        }
        self.apply_ui_requests();
        self.publish_state();
        self.finish_title_job(false);
        if self.quit_requested {
            return Ok(Step::Exit("UserQuit".into(), String::new()));
        }
        if self.parts.interaction.mode == Mode::Human {
            return self.human_step();
        }
        self.add_queued_messages();
        if let Some(exit) = self.check_limits() {
            return Ok(exit);
        }
        self.compact_if_needed();
        if interrupt::take() {
            return self.interrupted();
        }
        self.refresh_mcp();
        self.stats.api_calls += 1;
        self.show_status(Phase::Thinking);
        let response = match self.call_model() {
            Ok(response) => response,
            Err(_) if interrupt::take() => return self.interrupted(),
            Err(error) => return Err(error),
        };
        self.record_usage(&response);
        self.report_usage();
        let actions = match self.parse_actions(&response) {
            Ok(Parsed::Actions(actions)) => actions,
            Ok(Parsed::Reply) => return Ok(self.reply_ends_turn(&response)),
            Err(error) => return self.format_error(&response, &error),
        };
        self.format_errors = 0;
        self.messages.push(assistant_message(&response, &actions));
        show_assistant(self.stats.api_calls, self.stats.instance_cost, &response.content, &actions);
        self.parts.events.reply(&response.content, &actions);
        self.show_status(running_phase(&actions));
        let decision = match self.parts.events.confirm(&actions) {
            Some(decision) => decision,
            None => self.parts.interaction.confirm(&actions),
        };
        match decision {
            Decision::Run => {
                // A confirmation prompt showed the agent as waiting; it is running again.
                self.show_status(running_phase(&actions));
                self.execute_actions(&actions)
            }
            Decision::Reject(reason) => {
                self.push_observations(&actions, &[], Some(&reason))?;
                Ok(Step::Continue)
            }
            Decision::Interrupted => {
                self.push_observations(&actions, &[], Some(NOT_RUN_INTERRUPTED))?;
                self.interrupted()
            }
        }
    }

    /// Adds the lines typed while the agent was working to the conversation, before the next model
    /// call, so a message does not wait for the run to finish (task-2569). `/compact` typed this
    /// way compacts. Nothing happens without a terminal.
    fn add_queued_messages(&mut self) {
        if !self.parts.interaction.interactive {
            return;
        }
        for line in console::take_queued_lines() {
            let text = match handle_line(&line, &self.parts.context) {
                LineAction::SendToModel(text) => text,
                LineAction::Builtin(BuiltinCommand::Compact(focus)) => {
                    self.compact_by_summary(CompactionReason::Manual, &focus);
                    continue;
                }
                LineAction::Builtin(BuiltinCommand::Quit) => {
                    self.quit_requested = true;
                    continue;
                }
                LineAction::Builtin(command) => {
                    self.running_command(command);
                    continue;
                }
            };
            show_queued_message(&line);
            let images = self.typed_images(&line);
            self.messages.push(queued_message(&text).with_images(images));
        }
    }

    /// After a Ctrl+C: saves, then asks for a comment the way mini-swe-agent does. The comment
    /// goes to the model as `Interrupted by user: <text>`. With no terminal there is nobody to
    /// ask, so the run ends.
    fn interrupted(&mut self) -> Result<Step> {
        self.save();
        self.show_status(Phase::Waiting);
        if !self.parts.interaction.interactive {
            return Ok(Step::Exit(INTERRUPTED_STATUS.into(), String::new()));
        }
        loop {
            match self.parts.interaction.comment_prompt() {
                Comment::Quit => return Ok(Step::Exit(INTERRUPTED_STATUS.into(), String::new())),
                Comment::Compact(focus) => {
                    self.compact_by_summary(CompactionReason::Manual, &focus);
                }
                Comment::Text(text) => {
                    let images = self.typed_images(&text);
                    self.messages.push(Message::user(format!("Interrupted by user: {text}")).with_images(images).with_extra(json!({ "interrupt_type": "UserInterruption" })));
                    return Ok(Step::Continue);
                }
            }
        }
    }

    /// Loads the images named in text the person typed, and says what was attached.
    /// @param text - what the person typed
    fn typed_images(&mut self, text: &str) -> Vec<ImageData> {
        let (images, notices) = images_in_text(text, &self.parts.cwd, &self.parts.config.images);
        for notice in notices {
            self.notice(Tone::Warning, "image", &notice);
        }
        for image in &images {
            self.notice(Tone::Info, "image", &format!("attached {} {}", image.source, image.describe()));
        }
        images
    }

    /// Checks the step, cost and wall time limits before a model call. In a terminal the person
    /// may raise the step and cost limits and carry on.
    /// A resumed session counts from where it was resumed.
    fn check_limits(&mut self) -> Option<Step> {
        let config = &self.parts.config;
        let limit = config.wall_time_limit_seconds;
        if limit > 0 && self.started.elapsed().as_secs() >= limit {
            return Some(Step::Exit("TimeExceeded".into(), String::new()));
        }
        let steps_hit = config.step_limit > 0 && self.stats.api_calls - self.baseline.0 >= config.step_limit;
        let cost_hit = config.cost_limit > 0.0 && self.stats.instance_cost - self.baseline.1 >= config.cost_limit;
        if !steps_hit && !cost_hit {
            return None;
        }
        match self.parts.interaction.raise_limits(config.step_limit, config.cost_limit) {
            Some((steps, cost)) => {
                self.parts.config.step_limit = steps;
                self.parts.config.cost_limit = cost;
                self.check_limits()
            }
            None => Some(Step::Exit("LimitsExceeded".into(), String::new())),
        }
    }

    /// Calls the model with the history. When the provider says the request is longer than the
    /// context window, the history is compacted with a summary and the call is made once more.
    fn call_model(&mut self) -> Result<ModelResponse> {
        match self.call_model_once() {
            Err(error) if error.context_overflow && !error.interrupted && self.parts.config.compaction.enabled => {
                self.notice(Tone::Warning, "compaction", &format!("The model refused the request as too long ({}), so the session is being summarized and the call sent again.", error.message.lines().next().unwrap_or("")));
                if !self.compact_by_summary(CompactionReason::Overflow, "") {
                    return Err(anyhow::anyhow!("model call failed: {}", error.message));
                }
                self.call_model_once().map_err(|e| anyhow::anyhow!("model call failed: {}", e.message))
            }
            other => other.map_err(|e| anyhow::anyhow!("model call failed: {}", e.message)),
        }
    }

    /// Calls the model once and keeps the request size it reports, for the next estimate.
    fn call_model_once(&mut self) -> Result<ModelResponse, ProviderError> {
        let sent = self.messages.len();
        let response = self.parts.provider.complete(&self.messages, &self.tools)?;
        let usage = &response.usage;
        let tokens = usage.input_tokens + usage.cache_read_tokens + usage.cache_write_tokens;
        self.compaction.reported = (tokens > 0).then_some(ReportedSize { tokens, messages: sent });
        Ok(response)
    }

    /// The context window, asked of the config or the provider the first time it is needed.
    fn context_window(&mut self) -> u64 {
        if let Some(window) = self.compaction.window {
            return window;
        }
        let window = resolve_window(&self.parts.config.compaction, self.parts.provider.as_mut());
        self.compaction.window = Some(window);
        window
    }

    /// The estimated size of the next request in tokens.
    fn request_estimate(&self) -> u64 {
        estimate_request_tokens(&self.messages, self.compaction.tools_tokens, self.compaction.reported)
    }

    /// Before a model call: past the trigger, masks old outputs, and when that leaves the
    /// estimate above the target, replaces the middle of the history with a summary.
    fn compact_if_needed(&mut self) {
        if !self.parts.config.compaction.enabled {
            return;
        }
        let window = self.context_window();
        let budget = budget_for(&self.parts.config.compaction, window);
        if self.request_estimate() < budget.trigger {
            return;
        }
        self.compact_by_masking(CompactionReason::Threshold);
        if self.request_estimate() <= budget.target {
            return;
        }
        // A summary only replaces the conversation history. When the system prompt, the
        // instructions and the tool definitions fill most of the window, the history it could
        // remove is small, and summarizing it before every call costs a model call each time
        // and frees almost nothing (task-2588).
        let removable = summarizable_tokens(&self.messages, budget.keep_recent);
        if removable < budget.min_reclaim {
            self.summary_not_worth_it(removable, window);
            return;
        }
        self.compact_by_summary(CompactionReason::Threshold, "");
    }

    /// The tokens compaction can never remove: the system prompt, the first message and the tool
    /// definitions. The character estimate is scaled by how the provider's last reported size
    /// compared with the estimate for the same messages, so it is in the provider's own tokens.
    fn fixed_tokens(&self) -> u64 {
        let tools = self.compaction.tools_tokens;
        let head_end = self.messages.iter().position(|m| m.role == Role::User).map_or(self.messages.len(), |i| i + 1);
        let estimate = estimate_history_tokens(&self.messages[..head_end]) + tools;
        match self.compaction.reported {
            Some(reported) if reported.messages <= self.messages.len() => {
                let estimated_then = estimate_history_tokens(&self.messages[..reported.messages]) + tools;
                if estimated_then == 0 { estimate } else { estimate * reported.tokens / estimated_then }
            }
            _ => estimate,
        }
    }

    /// Says, once until a compaction works again, that the context is past the trigger but a
    /// summary would free too little to be worth a model call, and how big the fixed part is.
    /// @param removable - the tokens a summary could remove
    /// @param window - the context window in tokens
    fn summary_not_worth_it(&mut self, removable: u64, window: u64) {
        if self.compaction.stuck_warned && !console::is_verbose() {
            return;
        }
        self.compaction.stuck_warned = true;
        let used = self.request_estimate();
        let fixed = self.fixed_tokens();
        let percent = |n: u64| n * 100 / window.max(1);
        let text = format!("Context is {}% full ({} of {}), but a summary could only remove {} tokens of conversation, so none was made. The system prompt, instruction files and tool definitions take {} ({}% of the window) and compaction cannot remove them. Start fewer MCP servers (--mcp or mcp.disabled) or use a model with a larger window.", percent(used), short_tokens(used), short_tokens(window), short_tokens(removable), short_tokens(fixed), percent(fixed));
        self.notice(Tone::Warning, "compaction", &text);
    }

    /// Replaces old command outputs with placeholders, when that frees enough tokens to be
    /// worth one prompt cache miss.
    /// @param reason - why the compaction runs
    fn compact_by_masking(&mut self, reason: CompactionReason) {
        let window = self.context_window();
        let budget = budget_for(&self.parts.config.compaction, window);
        let before = self.request_estimate();
        let (indexes, _) = plan_mask(&self.messages, &budget, self.parts.config.compaction.mask_min_chars);
        if indexes.is_empty() {
            return;
        }
        apply_mask(&mut self.messages, &indexes);
        self.compaction.reported = None;
        let after = self.request_estimate();
        self.notice(Tone::Info, "compaction", &format!("compacted: cleared {} old command {} · {} → {} tokens of {}", indexes.len(), if indexes.len() == 1 { "output" } else { "outputs" }, short_tokens(before), short_tokens(after), short_tokens(window)));
        self.compaction.stuck_warned = false;
        self.compaction.records.push(mask_record(reason, self.stats.api_calls, (before, after, window), indexes));
        self.stats.compactions += 1;
        self.show_status(Phase::Compacting);
    }

    /// Asks the model for a summary of the middle of the history and continues from it. A failed
    /// summary is reported and the history is left as it was. Returns true when it compacted.
    /// @param reason - why the compaction runs
    /// @param focus - what the person asked the summary to focus on, or empty
    fn compact_by_summary(&mut self, reason: CompactionReason, focus: &str) -> bool {
        let window = self.context_window();
        let keep_recent = budget_for(&self.parts.config.compaction, window).keep_recent;
        let before = self.request_estimate();
        if find_cut(&self.messages, keep_recent).is_none() {
            self.nothing_to_summarize(reason, before, window);
            return false;
        }
        self.show_status(Phase::Compacting);
        let vars = self.template_vars("");
        let inputs = SummaryInputs { provider: self.parts.provider.as_mut(), tools: &self.tools, vars: &vars, summary_template: &self.templates.compact_summary, resume_template: &self.templates.compact_resume, focus };
        let outcome = match summarize(&self.messages, keep_recent, inputs) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.notice(Tone::Failure, "compaction", &format!("Compaction failed: {error:#}. The history was left as it was."));
                return false;
            }
        };
        self.record_usage(&outcome.response);
        self.messages = outcome.history.clone();
        self.compaction.reported = None;
        let after = self.request_estimate();
        self.parts.session.summary = summary_from_compaction(&outcome.summary);
        self.notice(Tone::Info, "compaction", &format!("compacted: summarized {} {} · {} → {} tokens of {}", outcome.removed.len(), if outcome.removed.len() == 1 { "message" } else { "messages" }, short_tokens(before), short_tokens(after), short_tokens(window)));
        self.compaction.stuck_warned = false;
        self.compaction.records.push(summary_record(reason, self.stats.api_calls, (before, after, window), &outcome));
        self.stats.compactions += 1;
        self.show_status(Phase::Compacting);
        true
    }

    /// Says why a summary was not made when the history has nothing between the task and the
    /// newest turns. The person asked with `/compact`, so they get an answer. Past the trigger it
    /// is a warning, once until a compaction works again, because the same check runs before every
    /// model call and would otherwise print the same line each time.
    /// @param reason - why the compaction was asked for
    /// @param used - the estimated request size in tokens
    /// @param window - the context window in tokens
    fn nothing_to_summarize(&mut self, reason: CompactionReason, used: u64, window: u64) {
        const WHY: &str = "the history is only the system prompt, the task and the newest turns";
        if reason == CompactionReason::Manual {
            self.notice(Tone::Info, "compaction", &format!("nothing to compact yet: {WHY}"));
            return;
        }
        if self.compaction.stuck_warned && !console::is_verbose() {
            return;
        }
        self.compaction.stuck_warned = true;
        let percent = used * 100 / window.max(1);
        self.notice(Tone::Warning, "compaction", &format!("Context is {percent}% full ({} of {}) and compaction cannot shrink it: {WHY}.", short_tokens(used), short_tokens(window)));
    }

    /// Draws the status bar with the run's current numbers: the size of the next request, the
    /// context window, where compaction starts, and the session's compaction count. Does nothing
    /// when the bar is off, so a run without a terminal never asks the provider for its window.
    /// @param phase - what the agent is doing now
    fn show_status(&mut self, phase: Phase) {
        if !console::status_bar_on() {
            return;
        }
        let window = self.context_window();
        let compaction = &self.parts.config.compaction;
        let trigger = compaction.enabled.then(|| budget_for(compaction, window).trigger);
        let summaries = self.compaction.records.iter().filter(|r| r.kind == CompactionKind::Summary).count() as u32;
        let model = self.parts.provider.model_name().to_string();
        let _ = summaries;
        let info = StatusInfo { phase, model, used: self.request_estimate(), window, trigger, cost: self.stats.instance_cost, work_secs: work_clock::elapsed().as_secs(), ..Default::default() };
        console::update_status(info);
    }

    /// Shows an event at the terminal and tells the front end.
    /// @param tone - how much attention it asks for
    /// @param label - what kind of event it is, such as compaction or image
    /// @param text - what happened
    fn notice(&mut self, tone: Tone, label: &str, text: &str) {
        let marker = match label {
            "compaction" => "↻",
            "image" => "+",
            _ => "·",
        };
        show_event(tone, marker, text);
        self.parts.events.notice(label, text);
    }

    /// Tells the front end the size of the next request, the context window and the cost.
    fn report_usage(&mut self) {
        let window = self.context_window();
        let used = self.request_estimate();
        self.parts.events.usage(used, window, self.stats.instance_cost);
    }

    /// Adds a call's tokens and cost to the totals.
    /// @param response - the model reply
    fn record_usage(&mut self, response: &ModelResponse) {
        self.stats.instance_cost += response.cost;
        self.stats.input_tokens += response.usage.input_tokens + response.usage.cache_read_tokens + response.usage.cache_write_tokens;
        self.stats.output_tokens += response.usage.output_tokens;
    }

    /// Reads the actions out of a reply in the run's action format, or finds that it is a reply
    /// to the person with no command.
    /// @param response - the model reply
    fn parse_actions(&self, response: &ModelResponse) -> Result<Parsed, String> {
        let finish = self.parts.config.finish;
        let tools = self.callable_tools();
        match self.parts.config.action_format {
            ActionFormat::Tools => parse_tool_reply(response, &tools, finish),
            ActionFormat::Text => parse_text_reply(response, &self.templates.action_regex, &tools, finish),
        }
    }

    /// A reply with no command: it is kept in the history, shown, and ends the turn. The reply
    /// text is the turn's final output.
    /// @param response - the model reply
    fn reply_ends_turn(&mut self, response: &ModelResponse) -> Step {
        self.format_errors = 0;
        self.messages.push(assistant_message(response, &[]));
        show_assistant(self.stats.api_calls, self.stats.instance_cost, &response.content, &[]);
        self.parts.events.reply(&response.content, &[]);
        Step::Exit(REPLIED_STATUS.into(), response.content.trim().to_string())
    }

    /// Tells the model its reply could not be used. As in mini-swe-agent, the bad reply itself is
    /// not kept; the model sees the error as a user message. Too many in a row ends the run.
    /// @param response - the rejected reply
    /// @param error - what was wrong
    fn format_error(&mut self, response: &ModelResponse, error: &str) -> Result<Step> {
        self.format_errors += 1;
        let text = render_format_error(&self.templates.format_error, error, &response.finish_reason, !response.tool_calls.is_empty())?;
        self.show_format_error(response, error);
        self.messages.push(Message::user(text).with_extra(json!({ "interrupt_type": "FormatError", "model_response": response.content, "cost": response.cost })));
        let max = self.parts.config.max_consecutive_format_errors;
        if max > 0 && self.format_errors >= max {
            return Ok(Step::Exit("RepeatedFormatError".into(), String::new()));
        }
        Ok(Step::Continue)
    }

    /// Shows a reply that could not be used. Its text is shown like any reply, because it is often
    /// a real answer to the person, followed by one dim line saying it was sent back. Verbose
    /// output shows the whole error as well.
    /// @param response - the rejected reply
    /// @param error - what was wrong
    fn show_format_error(&mut self, response: &ModelResponse, error: &str) {
        show_reply_text(&response.content);
        let max = self.parts.config.max_consecutive_format_errors;
        let count = if max > 0 { format!(" ({} of {max})", self.format_errors) } else { String::new() };
        let detail = if console::is_verbose() { error.to_string() } else { first_sentence(error) };
        show_event(Tone::Info, "↺", &format!("{detail}; asked the model again{count}"));
        self.parts.events.notice("format error", &format!("{error}\n(reply was: {})", response.content.chars().take(500).collect::<String>()));
    }

    /// Runs the actions in order. A command that submits ends the run; the actions after it are
    /// reported to the model as not executed, so every tool call still gets a result. A Ctrl+C
    /// stops the running action, skips the rest the same way, and shows the comment prompt.
    /// @param actions - the parsed actions
    fn execute_actions(&mut self, actions: &[Action]) -> Result<Step> {
        let mut outputs = Vec::new();
        let mut submitted = None;
        for (index, action) in actions.iter().enumerate() {
            self.parts.events.action_started(index, action);
            let observation = self.execute(action);
            if matches!(action, Action::Bash { .. }) {
                submitted = submission_from(&observation);
            }
            if submitted.is_none() || console::is_verbose() {
                show_observation(&observation);
            }
            self.parts.events.action_finished(index, action, &observation);
            outputs.push(observation);
            if submitted.is_some() || interrupt::is_pending() {
                break;
            }
        }
        let interrupted = submitted.is_none() && interrupt::take();
        self.push_observations(actions, &outputs, interrupted.then_some(NOT_RUN_INTERRUPTED))?;
        if interrupted {
            return self.interrupted();
        }
        Ok(match submitted { Some(text) => Step::Exit("Submitted".into(), text), None => Step::Continue })
    }

    /// Runs one action in the environment or on its MCP server.
    /// @param action - the action
    fn execute(&mut self, action: &Action) -> Observation {
        let started_at = now_seconds();
        let clock = Instant::now();
        let mut observation = self.execute_untimed(action);
        observation.started_at = Some(started_at);
        observation.duration_ms = Some(clock.elapsed().as_millis() as u64);
        observation
    }

    /// Runs one action without timing it.
    /// @param action - the action
    fn execute_untimed(&mut self, action: &Action) -> Observation {
        match action {
            Action::Bash { command, .. } => self.parts.environment.execute(command),
            Action::Mcp { server, tool, arguments, .. } if is_tool_search(server, tool) => self.run_tool_search(arguments),
            Action::Mcp { server, tool, arguments, .. } => {
                let observation = self.parts.mcp.call(server, tool, arguments.clone());
                // A deferred tool called without a search first is loaded now, so the history
                // never names a tool the request does not send.
                self.load_tools(vec![mcp_name(server, tool)], true);
                observation
            }
        }
    }

    /// Appends one observation message per action. Actions without an output get a "not
    /// executed" result, carrying `reason` when the person rejected them.
    /// @param actions - the actions
    /// @param outputs - the results so far, in order
    /// @param reason - why the actions were not run, if they were rejected
    fn push_observations(&mut self, actions: &[Action], outputs: &[Observation], reason: Option<&str>) -> Result<()> {
        let vars = self.template_vars("");
        for (index, action) in actions.iter().enumerate() {
            let observation = match outputs.get(index) {
                Some(observation) => observation.clone(),
                None => {
                    let reason = reason.unwrap_or("action was not executed");
                    self.parts.events.action_not_run(index, action, reason);
                    Observation::failed("", reason)
                }
            };
            let content = render_observation(&self.templates.observation, &vars, &observation)?;
            let message = match action.tool_call_id() {
                Some(id) => Message::tool(id, content),
                None => Message::user(content),
            };
            self.messages.push(message.with_images(observation.images.clone()).with_extra(json!({ "raw_output": observation.output, "returncode": observation.returncode, "exception_info": observation.exception_info, "timestamp": now_seconds(), "started_at": observation.started_at, "duration_ms": observation.duration_ms })));
        }
        Ok(())
    }

    /// Human mode: the person types a command, Umbra runs it, and both go into the history so
    /// the model sees them when it takes over again.
    fn human_step(&mut self) -> Result<Step> {
        let command = match self.parts.interaction.read_human_command() {
            HumanInput::Command(command) => command,
            HumanInput::SwitchMode(mode) => {
                self.parts.interaction.mode = mode;
                return Ok(Step::Continue);
            }
            HumanInput::Quit => return Ok(Step::Exit("UserQuit".into(), String::new())),
            HumanInput::Interrupted => return self.interrupted(),
        };
        let action = Action::Bash { command: command.clone(), tool_call_id: None };
        self.messages.push(Message::user(format!("User command: \n```bash\n{command}\n```")).with_extra(json!({ "actions": [action] })));
        self.show_status(running_phase(std::slice::from_ref(&action)));
        self.parts.events.action_started(0, &action);
        let observation = self.execute(&action);
        show_observation(&observation);
        self.parts.events.action_finished(0, &action, &observation);
        let submitted = submission_from(&observation);
        self.push_observations(&[action], &[observation], None)?;
        Ok(match submitted { Some(text) => Step::Exit("Submitted".into(), text), None => Step::Continue })
    }

    /// The template variables: the caller's variables plus the task, the counters and the format.
    /// @param task - the task text
    fn template_vars(&self, task: &str) -> Value {
        let mut vars = self.parts.vars.clone();
        vars["task"] = json!(task);
        vars["n_model_calls"] = json!(self.stats.api_calls);
        vars["model_cost"] = json!(self.stats.instance_cost);
        vars["elapsed_seconds"] = json!(self.started.elapsed().as_secs());
        vars["action_format"] = json!(self.parts.config.action_format);
        vars["finish"] = json!(self.parts.config.finish);
        // With deferral the prompt lists the tools by name only; their definitions come from
        // tool_search when the model needs them.
        let described: Vec<&McpTool> = if self.deferred { Vec::new() } else { self.mcp_tools.iter().collect() };
        vars["mcp_tools"] = json!(described.iter().map(|t| json!({ "qualified_name": t.qualified_name(), "description": t.description, "input_schema": t.input_schema })).collect::<Vec<_>>());
        vars["deferred_tools"] = json!(if self.deferred { self.mcp_tools.iter().map(McpTool::qualified_name).collect::<Vec<_>>() } else { Vec::new() });
        vars
    }

    /// Writes the trajectory and the session's index entry. A failure to write is reported but
    /// does not stop the run.
    fn save(&self) {
        let last = self.messages.iter().rev().find(|m| m.role == Role::Exit);
        let exit_status = last.and_then(|m| m.extra["exit_status"].as_str()).unwrap_or("").to_string();
        let submission = last.map(|m| m.content.clone()).unwrap_or_default();
        let mut session = self.parts.session.clone();
        session.mcp_off = self.parts.mcp.off_names();
        if self.parts.interaction.interactive {
            session.work_seconds = work_clock::elapsed().as_secs_f64();
        }
        let info = TrajectoryInfo { model_stats: self.stats.clone(), config: self.parts.config_record.clone(), umbra_version: env!("CARGO_PKG_VERSION").into(), exit_status: exit_status.clone(), submission, compactions: self.compaction.records.clone(), session: session.clone() };
        let trajectory = Trajectory { info, messages: self.messages.clone(), trajectory_format: "mini-swe-agent-1.1".into() };
        if let Err(error) = save_trajectory(&self.parts.trajectory_path, &trajectory) {
            eprintln!("umbra: could not save the trajectory: {error:#}");
        }
        let trajectory_path = absolute_path(&self.parts.trajectory_path).to_string_lossy().replace('\\', "/");
        let entry = SessionIndexEntry { id: session.id, trajectory: trajectory_path, cwd: session.cwd, model: session.model, task: session.task, started_at: session.started_at, updated_at: now_seconds(), exit_status, api_calls: self.stats.api_calls, cost: self.stats.instance_cost, messages: self.messages.len(), title: session.title, summary: session.summary, mcp_off: session.mcp_off, work_seconds: session.work_seconds };
        if let Err(error) = save_index(&entry) {
            eprintln!("umbra: could not save the session index: {error:#}");
        }
    }
}

/// The tools offered in tool mode: `bash`, then one tool per MCP tool.
/// @param mcp_tools - the tools from the connected servers
pub fn tool_specs(mcp_tools: &[McpTool]) -> Vec<ToolSpec> {
    let bash = ToolSpec {
        name: "bash".into(),
        description: "Execute a bash command".into(),
        input_schema: json!({ "type": "object", "properties": { "command": { "type": "string", "description": "The bash command to execute" } }, "required": ["command"] }),
    };
    let mut specs = vec![bash];
    specs.extend(mcp_tools.iter().map(|t| ToolSpec { name: t.qualified_name(), description: format!("[MCP server {}] {}", t.server, t.description), input_schema: t.input_schema.clone() }));
    specs
}

/// The full `mcp__server__tool` name for an MCP action.
/// @param server - the server name
/// @param tool - the tool name
fn mcp_name(server: &str, tool: &str) -> String {
    McpTool { server: server.into(), name: tool.into(), description: String::new(), input_schema: Value::Null }.qualified_name()
}

/// Builds the assistant message for the history. For Anthropic the raw content blocks are kept
/// in `extra` and sent back unchanged on the next call.
/// @param response - the model reply
/// @param actions - its parsed actions
fn assistant_message(response: &ModelResponse, actions: &[Action]) -> Message {
    let mut raw = response.raw.clone();
    let blocks = raw.as_object_mut().and_then(|m| m.remove(RAW_CONTENT_KEY)).unwrap_or(Value::Null);
    let mut message = Message::new(Role::Assistant, response.content.clone());
    message.tool_calls = response.tool_calls.clone();
    let mut extra = json!({ "actions": actions, "response": raw, "cost": response.cost, "usage": response.usage, "finish_reason": response.finish_reason, "timestamp": now_seconds() });
    if !blocks.is_null() {
        extra[RAW_CONTENT_KEY] = blocks;
    }
    message.with_extra(extra)
}

/// The message that gives the model a line typed while it was working.
/// @param text - what the person typed
fn queued_message(text: &str) -> Message {
    Message::user(format!("The user sent this message while you were working: {text}")).with_extra(json!({ "interrupt_type": "UserMessage" }))
}

/// Shows that a typed message was picked up, so the person can see the agent has it.
/// @param text - what the person typed
fn show_queued_message(text: &str) {
    if console::is_quiet() {
        return;
    }
    show_user_text("›", text);
}

/// The message that gives the model a follow up task, or the next task of a resumed session. In
/// the reply style it is the person's words as they typed them, the next message of a
/// conversation; the submit style keeps mini-swe-agent's "new task" framing. The person's own
/// words are kept in `extra.user_text` either way.
/// @param task - the task text
/// @param finish - how a turn ends
fn new_task_message(task: &str, typed: &str, finish: Finish) -> Message {
    let content = match finish {
        Finish::Reply => task.to_string(),
        Finish::Submit => format!("The user added a new task: {task}"),
    };
    Message::user(content).with_extra(json!({ "interrupt_type": "UserNewTask", "user_text": typed }))
}

/// The name of a built in command, as typed.
/// @param command - the command
fn command_name(command: &BuiltinCommand) -> &'static str {
    match command {
        BuiltinCommand::Config => "/config",
        BuiltinCommand::Model(_) => "/model",
        BuiltinCommand::Mcp => "/mcp",
        BuiltinCommand::Mode(_) => "/mode",
        BuiltinCommand::Clear => "/clear",
        BuiltinCommand::Compact(_) => "/compact",
        BuiltinCommand::Resume(_) => "/resume",
        BuiltinCommand::Skills => "/skills",
        BuiltinCommand::Help => "/help",
        BuiltinCommand::Update => "/update",
        BuiltinCommand::Quit => "/quit",
    }
}

/// The first sentence of an error, without its final full stop and starting lowercase, for a
/// one line note.
/// @param error - the error text
fn first_sentence(error: &str) -> String {
    let line = error.lines().next().unwrap_or("").trim();
    let sentence = line.split(". ").next().unwrap_or(line).trim_end_matches('.');
    let mut chars = sentence.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => "the reply could not be used".into(),
    }
}

/// The status bar phase for a set of actions: the program a command starts, or the MCP tool,
/// and how many more actions follow.
/// @param actions - the actions about to run
fn running_phase(actions: &[Action]) -> Phase {
    let Some(first) = actions.first() else { return Phase::Running(String::new()) };
    let name = match first {
        Action::Bash { command, .. } => command.split_whitespace().next().unwrap_or("").rsplit(['/', '\\']).next().unwrap_or("").to_string(),
        Action::Mcp { server, tool, .. } => format!("{server}.{tool}"),
    };
    let name: String = name.chars().take(32).collect();
    let more = if actions.len() > 1 { format!(" +{}", actions.len() - 1) } else { String::new() };
    Phase::Running(format!("{name}{more}"))
}

/// Seconds since the Unix epoch, as a float.
pub fn now_seconds() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}
