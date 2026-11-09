//! The agent loop. It follows mini-swe-agent's DefaultAgent: one linear list of messages, a
//! model call per step, the actions run in order, the observations appended, and the run ends
//! when a command prints COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT or a limit is reached.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use serde_json::{json, Value};

use super::action_parser::{parse_text_action, parse_tool_actions};
use super::agent_events::AgentEvents;
use super::compaction_service::{apply_mask, budget_for, estimate_request_tokens, estimate_tools_tokens, mask_record, plan_mask, resolve_window, summarize, summary_record, ReportedSize, SummaryInputs};
use super::environment::{submission_from, Environment};
use super::image_service::images_in_text;
use super::interaction_service::{show_assistant, show_notice, show_observation, Comment, Decision, HumanInput, Interaction};
use super::session_service::ResumedSession;
use super::mcp::McpManager;
use super::prompt_service::{render, render_format_error, render_observation, Templates};
use super::providers::anthropic::RAW_CONTENT_KEY;
use super::providers::{Provider, ProviderError, ToolSpec};
use crate::models::action::{Action, Observation};
use crate::models::compaction::{CompactionKind, CompactionReason, CompactionRecord};
use crate::models::config::{ActionFormat, Mode, UmbraConfig};
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
use crate::utils::status_bar::{Phase, StatusInfo};

/// How a run ended.
#[derive(Debug, Clone)]
pub struct RunResult {
    /// `Submitted`, `LimitsExceeded`, `TimeExceeded`, `RepeatedFormatError`, `ModelError`, `UserQuit` or `Interrupted`.
    pub exit_status: String,
    pub submission: String,
    pub trajectory_path: PathBuf,
    pub stats: ModelStats,
    pub session_id: String,
}

/// How a run starts: a new task, or a saved session with the next task.
pub enum Opening {
    New { task: String, images: Vec<ImageData> },
    Resume { session: Box<ResumedSession>, task: String, images: Vec<ImageData> },
}

/// The exit status of a run the person ended with Ctrl+C.
pub const INTERRUPTED_STATUS: &str = "Interrupted";

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
    pub mcp: McpManager,
    pub interaction: Interaction,
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
    mcp_tools: Vec<McpTool>,
    messages: Vec<Message>,
    stats: ModelStats,
    format_errors: u32,
    started: Instant,
    exit: (String, String),
    compaction: CompactionState,
    /// The model calls and cost already spent when this run started; the limits count from here.
    baseline: (u32, f64),
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
}

impl Agent {
    /// Prepares a run: picks the templates and builds the tool list for tool mode.
    /// @param parts - the provider, environment, MCP servers and settings
    pub fn new(parts: AgentParts) -> Self {
        let templates = Templates::resolve(&parts.config.templates, parts.config.action_format);
        let mcp_tools = parts.mcp.tools();
        let tools = if parts.config.action_format == ActionFormat::Tools { tool_specs(&mcp_tools) } else { Vec::new() };
        let compaction = CompactionState { tools_tokens: estimate_tools_tokens(&tools), ..Default::default() };
        Agent { parts, templates, tools, mcp_tools, messages: Vec::new(), stats: ModelStats::default(), format_errors: 0, started: Instant::now(), exit: (String::new(), String::new()), compaction, baseline: (0, 0.0) }
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
            Opening::New { task, images } => self.open_new(&task, images)?,
            Opening::Resume { session, task, images } => {
                self.open_resumed(*session);
                self.add_task(&task, images);
            }
        }
        self.save();
        self.show_status(Phase::Starting);
        // Read the terminal from the start, so a line typed while the first steps run is queued
        // for the next model call rather than left waiting until the run ends.
        if self.parts.interaction.interactive {
            console::start_reading_stdin();
        }
        loop {
            let end = self.run_turn();
            if end.status == "Submitted" && self.follow_up() {
                continue;
            }
            self.end_turn(&end);
            break;
        }
        self.close();
        Ok(RunResult { exit_status: self.exit.0.clone(), submission: self.exit.1.clone(), trajectory_path: self.parts.trajectory_path.clone(), stats: self.stats.clone(), session_id: self.parts.session.id.clone() })
    }

    /// Starts a new session: renders the system prompt and the first message. The person's own
    /// words are kept in `extra.user_text`, so a front end can show them again later.
    /// @param task - the task text
    /// @param images - the images attached to the task
    pub fn open_new(&mut self, task: &str, images: Vec<ImageData>) -> Result<()> {
        let (system, instance) = self.render_prompts(task)?;
        self.messages = vec![Message::system(system), Message::user(instance).with_images(images).with_extra(json!({ "user_text": task }))];
        Ok(())
    }

    /// Continues a saved session from its history and totals.
    /// @param session - the saved session
    pub fn open_resumed(&mut self, session: ResumedSession) {
        self.messages = session.messages;
        self.stats = session.stats;
        self.compaction.records = session.compactions;
        self.baseline = (self.stats.api_calls, self.stats.instance_cost);
    }

    /// Gives the model its next task, the way a follow up task is given. An exit marker left by
    /// the previous turn is removed first, so the history stays one conversation.
    /// @param task - the task text
    /// @param images - the images attached to it
    pub fn add_task(&mut self, task: &str, images: Vec<ImageData>) {
        while self.messages.last().is_some_and(|m| m.role == Role::Exit) {
            self.messages.pop();
        }
        self.messages.push(new_task_message(task).with_images(images));
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

    /// Stops the environment and the MCP servers.
    pub fn close(&mut self) {
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
        while let Some(task) = self.parts.interaction.follow_up_task() {
            if let Some(focus) = compact_command(&task) {
                self.compact_by_summary(CompactionReason::Manual, focus);
                continue;
            }
            let images = self.typed_images(&task);
            self.add_task(&task, images);
            return true;
        }
        false
    }

    /// One step: a model call (or a typed command in human mode), then the actions. A Ctrl+C
    /// at any point stops the step and shows the comment prompt.
    fn step(&mut self) -> Result<Step> {
        if interrupt::take() {
            return self.interrupted();
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
            Ok(actions) => actions,
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
            Decision::Run => self.execute_actions(&actions),
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
        for text in console::take_queued_lines() {
            if let Some(focus) = compact_command(&text) {
                self.compact_by_summary(CompactionReason::Manual, focus);
                continue;
            }
            show_queued_message(&text);
            let images = self.typed_images(&text);
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
            self.notice("image", &notice);
        }
        for image in &images {
            self.notice("image", &format!("attached {} {}", image.source, image.describe()));
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
                self.notice("compaction", &format!("the model refused the request as too long ({}), so the session is being summarized and the call sent again", error.message.lines().next().unwrap_or("")));
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
        if self.request_estimate() > budget.target {
            self.compact_by_summary(CompactionReason::Threshold, "");
        }
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
        self.notice("compaction", &format!("removed {} old command outputs: about {before} to {after} tokens of a {window} token window", indexes.len()));
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
        self.show_status(Phase::Compacting);
        let vars = self.template_vars("");
        let inputs = SummaryInputs { provider: self.parts.provider.as_mut(), tools: &self.tools, vars: &vars, summary_template: &self.templates.compact_summary, resume_template: &self.templates.compact_resume, focus };
        let outcome = match summarize(&self.messages, keep_recent, inputs) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.notice("compaction", &format!("could not summarize the session: {error:#}"));
                return false;
            }
        };
        self.record_usage(&outcome.response);
        self.messages = outcome.history.clone();
        self.compaction.reported = None;
        let after = self.request_estimate();
        self.notice("compaction", &format!("summarized {} messages: about {before} to {after} tokens of a {window} token window", outcome.removed.len()));
        self.compaction.records.push(summary_record(reason, self.stats.api_calls, (before, after, window), &outcome));
        self.stats.compactions += 1;
        self.show_status(Phase::Compacting);
        true
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
        let info = StatusInfo { phase, model, used: self.request_estimate(), window, trigger, compactions: self.stats.compactions, summaries, step: self.stats.api_calls, cost: self.stats.instance_cost, elapsed_secs: self.started.elapsed().as_secs() };
        console::update_status(info);
    }

    /// Shows a notice at the terminal and tells the front end.
    /// @param label - the heading
    /// @param text - the notice
    fn notice(&mut self, label: &str, text: &str) {
        show_notice(label, text);
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

    /// Reads the actions out of a reply in the run's action format.
    /// @param response - the model reply
    fn parse_actions(&self, response: &ModelResponse) -> Result<Vec<Action>, String> {
        match self.parts.config.action_format {
            ActionFormat::Tools => parse_tool_actions(response, &self.mcp_tools),
            ActionFormat::Text => parse_text_action(response, &self.templates.action_regex, &self.mcp_tools),
        }
    }

    /// Tells the model its reply could not be used. As in mini-swe-agent, the bad reply itself is
    /// not kept; the model sees the error as a user message. Too many in a row ends the run.
    /// @param response - the rejected reply
    /// @param error - what was wrong
    fn format_error(&mut self, response: &ModelResponse, error: &str) -> Result<Step> {
        self.format_errors += 1;
        let text = render_format_error(&self.templates.format_error, error, &response.finish_reason, !response.tool_calls.is_empty())?;
        self.notice("format error", &format!("{error}\n(reply was: {})", response.content.chars().take(500).collect::<String>()));
        self.messages.push(Message::user(text).with_extra(json!({ "interrupt_type": "FormatError", "model_response": response.content, "cost": response.cost })));
        let max = self.parts.config.max_consecutive_format_errors;
        if max > 0 && self.format_errors >= max {
            return Ok(Step::Exit("RepeatedFormatError".into(), String::new()));
        }
        Ok(Step::Continue)
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
            show_observation(&observation);
            self.parts.events.action_finished(index, action, &observation);
            if matches!(action, Action::Bash { .. }) {
                submitted = submission_from(&observation);
            }
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
        match action {
            Action::Bash { command, .. } => self.parts.environment.execute(command),
            Action::Mcp { server, tool, arguments, .. } => self.parts.mcp.call(server, tool, arguments.clone()),
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
            self.messages.push(message.with_images(observation.images.clone()).with_extra(json!({ "raw_output": observation.output, "returncode": observation.returncode, "exception_info": observation.exception_info, "timestamp": now_seconds() })));
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
        let observation = self.execute(&action);
        show_observation(&observation);
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
        vars["mcp_tools"] = json!(self.mcp_tools.iter().map(|t| json!({ "qualified_name": t.qualified_name(), "description": t.description, "input_schema": t.input_schema })).collect::<Vec<_>>());
        vars
    }

    /// Writes the trajectory and the session's index entry. A failure to write is reported but
    /// does not stop the run.
    fn save(&self) {
        let last = self.messages.iter().rev().find(|m| m.role == Role::Exit);
        let exit_status = last.and_then(|m| m.extra["exit_status"].as_str()).unwrap_or("").to_string();
        let submission = last.map(|m| m.content.clone()).unwrap_or_default();
        let session = self.parts.session.clone();
        let info = TrajectoryInfo { model_stats: self.stats.clone(), config: self.parts.config_record.clone(), umbra_version: env!("CARGO_PKG_VERSION").into(), exit_status: exit_status.clone(), submission, compactions: self.compaction.records.clone(), session: session.clone() };
        let trajectory = Trajectory { info, messages: self.messages.clone(), trajectory_format: "mini-swe-agent-1.1".into() };
        if let Err(error) = save_trajectory(&self.parts.trajectory_path, &trajectory) {
            eprintln!("umbra: could not save the trajectory: {error:#}");
        }
        let trajectory_path = absolute_path(&self.parts.trajectory_path).to_string_lossy().replace('\\', "/");
        let entry = SessionIndexEntry { id: session.id, trajectory: trajectory_path, cwd: session.cwd, model: session.model, task: session.task, started_at: session.started_at, updated_at: now_seconds(), exit_status, api_calls: self.stats.api_calls, cost: self.stats.instance_cost, messages: self.messages.len() };
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
    console::heading("message from the user", "1;35");
    console::emit(text);
}

/// The message that gives the model a follow up task, or the next task of a resumed session. The
/// person's own words are kept in `extra.user_text`.
/// @param task - the task text
fn new_task_message(task: &str) -> Message {
    Message::user(format!("The user added a new task: {task}")).with_extra(json!({ "interrupt_type": "UserNewTask", "user_text": task }))
}

/// Reads `/compact` or `/compact <focus>` typed at the follow up prompt. Returns the focus text,
/// empty when there is none, or None for any other text.
/// @param text - what the person typed
fn compact_command(text: &str) -> Option<&str> {
    let rest = text.trim().strip_prefix("/compact")?;
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then(|| rest.trim())
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

#[cfg(test)]
mod tests {
    use super::compact_command;

    #[test]
    fn reads_the_compact_command_and_its_focus() {
        assert_eq!(compact_command("/compact"), Some(""));
        assert_eq!(compact_command("  /compact keep the test names  "), Some("keep the test names"));
        assert_eq!(compact_command("/compaction is broken"), None);
        assert_eq!(compact_command("fix the build"), None);
    }
}
