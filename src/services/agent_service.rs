//! The agent loop. It follows mini-swe-agent's DefaultAgent: one linear list of messages, a
//! model call per step, the actions run in order, the observations appended, and the run ends
//! when a command prints COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT or a limit is reached.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use serde_json::{json, Value};

use super::action_parser::{parse_text_action, parse_tool_actions};
use super::environment::{submission_from, Environment};
use super::interaction_service::{show_assistant, show_notice, show_observation, Decision, HumanInput, Interaction};
use super::mcp::McpManager;
use super::prompt_service::{render, render_format_error, render_observation, Templates};
use super::providers::anthropic::RAW_CONTENT_KEY;
use super::providers::{Provider, ToolSpec};
use crate::models::action::{Action, Observation};
use crate::models::config::{ActionFormat, Mode, UmbraConfig};
use crate::models::context::McpTool;
use crate::models::message::{Message, Role};
use crate::models::model_response::ModelResponse;
use crate::models::trajectory::{ModelStats, Trajectory, TrajectoryInfo};
use crate::repositories::trajectory_repository::save_trajectory;

/// How a run ended.
#[derive(Debug, Clone)]
pub struct RunResult {
    /// `Submitted`, `LimitsExceeded`, `TimeExceeded`, `RepeatedFormatError`, `ModelError` or `UserQuit`.
    pub exit_status: String,
    pub submission: String,
    pub trajectory_path: PathBuf,
    pub stats: ModelStats,
}

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
}

impl Agent {
    /// Prepares a run: picks the templates and builds the tool list for tool mode.
    /// @param parts - the provider, environment, MCP servers and settings
    pub fn new(parts: AgentParts) -> Self {
        let templates = Templates::resolve(&parts.config.templates, parts.config.action_format);
        let mcp_tools = parts.mcp.tools();
        let tools = if parts.config.action_format == ActionFormat::Tools { tool_specs(&mcp_tools) } else { Vec::new() };
        Agent { parts, templates, tools, mcp_tools, messages: Vec::new(), stats: ModelStats::default(), format_errors: 0, started: Instant::now(), exit: (String::new(), String::new()) }
    }

    /// Renders the system prompt for a task without running anything, for `umbra prompt`.
    /// @param task - the task text
    pub fn render_prompts(&self, task: &str) -> Result<(String, String)> {
        let vars = self.template_vars(task);
        Ok((render(&self.templates.system, &vars)?, render(&self.templates.instance, &vars)?))
    }

    /// Runs the task to the end and returns how it ended. The trajectory is saved after every step.
    /// @param task - the task text
    pub fn run(&mut self, task: &str) -> Result<RunResult> {
        let (system, instance) = self.render_prompts(task)?;
        self.messages = vec![Message::system(system), Message::user(instance)];
        loop {
            let step = self.step();
            let ended = match step {
                Ok(Step::Continue) => false,
                Ok(Step::Exit(status, submission)) => self.finish_or_continue(status, submission),
                Err(error) => self.finish_or_continue("ModelError".into(), format!("{error:#}")),
            };
            self.save();
            if ended {
                break;
            }
        }
        self.parts.environment.close();
        self.parts.mcp.close();
        Ok(RunResult { exit_status: self.exit.0.clone(), submission: self.exit.1.clone(), trajectory_path: self.parts.trajectory_path.clone(), stats: self.stats.clone() })
    }

    /// Handles a step that wants to end the run. After a submission the person may give a
    /// follow up task, which continues the same conversation. Returns true when the run is over.
    /// @param status - the exit status
    /// @param submission - the final output
    fn finish_or_continue(&mut self, status: String, submission: String) -> bool {
        if status == "Submitted" {
            if let Some(task) = self.parts.interaction.follow_up_task() {
                self.messages.push(Message::user(format!("The user added a new task: {task}")).with_extra(json!({ "interrupt_type": "UserNewTask" })));
                return false;
            }
        }
        self.messages.push(Message::exit(&status, &submission));
        self.exit = (status, submission);
        true
    }

    /// One step: a model call (or a typed command in human mode), then the actions.
    fn step(&mut self) -> Result<Step> {
        if self.parts.interaction.mode == Mode::Human {
            return self.human_step();
        }
        if let Some(exit) = self.check_limits() {
            return Ok(exit);
        }
        self.stats.api_calls += 1;
        let response = self.parts.provider.complete(&self.messages, &self.tools).map_err(|e| anyhow::anyhow!("model call failed: {}", e.message))?;
        self.record_usage(&response);
        let actions = match self.parse_actions(&response) {
            Ok(actions) => actions,
            Err(error) => return self.format_error(&response, &error),
        };
        self.format_errors = 0;
        self.messages.push(assistant_message(&response, &actions));
        show_assistant(self.stats.api_calls, self.stats.instance_cost, &response.content, &actions);
        match self.parts.interaction.confirm(&actions) {
            Decision::Run => self.execute_actions(&actions),
            Decision::Reject(reason) => {
                self.push_observations(&actions, &[], Some(&reason))?;
                Ok(Step::Continue)
            }
        }
    }

    /// Checks the step, cost and wall time limits before a model call. In a terminal the person
    /// may raise the step and cost limits and carry on.
    fn check_limits(&mut self) -> Option<Step> {
        let config = &self.parts.config;
        let limit = config.wall_time_limit_seconds;
        if limit > 0 && self.started.elapsed().as_secs() >= limit {
            return Some(Step::Exit("TimeExceeded".into(), String::new()));
        }
        let steps_hit = config.step_limit > 0 && self.stats.api_calls >= config.step_limit;
        let cost_hit = config.cost_limit > 0.0 && self.stats.instance_cost >= config.cost_limit;
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
        show_notice("format error", &format!("{error}\n(reply was: {})", response.content.chars().take(500).collect::<String>()));
        self.messages.push(Message::user(text).with_extra(json!({ "interrupt_type": "FormatError", "model_response": response.content, "cost": response.cost })));
        let max = self.parts.config.max_consecutive_format_errors;
        if max > 0 && self.format_errors >= max {
            return Ok(Step::Exit("RepeatedFormatError".into(), String::new()));
        }
        Ok(Step::Continue)
    }

    /// Runs the actions in order. A command that submits ends the run; the actions after it are
    /// reported to the model as not executed, so every tool call still gets a result.
    /// @param actions - the parsed actions
    fn execute_actions(&mut self, actions: &[Action]) -> Result<Step> {
        let mut outputs = Vec::new();
        let mut submitted = None;
        for action in actions {
            let observation = self.execute(action);
            show_observation(&observation);
            if matches!(action, Action::Bash { .. }) {
                submitted = submission_from(&observation);
            }
            outputs.push(observation);
            if submitted.is_some() {
                break;
            }
        }
        self.push_observations(actions, &outputs, None)?;
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
            let observation = outputs.get(index).cloned().unwrap_or_else(|| Observation::failed("", reason.unwrap_or("action was not executed")));
            let content = render_observation(&self.templates.observation, &vars, &observation)?;
            let message = match action.tool_call_id() {
                Some(id) => Message::tool(id, content),
                None => Message::user(content),
            };
            self.messages.push(message.with_extra(json!({ "raw_output": observation.output, "returncode": observation.returncode, "exception_info": observation.exception_info, "timestamp": now_seconds() })));
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
        };
        let action = Action::Bash { command: command.clone(), tool_call_id: None };
        self.messages.push(Message::user(format!("User command: \n```bash\n{command}\n```")).with_extra(json!({ "actions": [action] })));
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

    /// Writes the trajectory. A failure to write is reported but does not stop the run.
    fn save(&self) {
        let last = self.messages.iter().rev().find(|m| m.role == Role::Exit);
        let exit_status = last.and_then(|m| m.extra["exit_status"].as_str()).unwrap_or("").to_string();
        let submission = last.map(|m| m.content.clone()).unwrap_or_default();
        let info = TrajectoryInfo { model_stats: self.stats.clone(), config: self.parts.config_record.clone(), umbra_version: env!("CARGO_PKG_VERSION").into(), exit_status, submission };
        let trajectory = Trajectory { info, messages: self.messages.clone(), trajectory_format: "mini-swe-agent-1.1".into() };
        if let Err(error) = save_trajectory(&self.parts.trajectory_path, &trajectory) {
            eprintln!("umbra: could not save the trajectory: {error:#}");
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

/// Seconds since the Unix epoch, as a float.
fn now_seconds() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}
