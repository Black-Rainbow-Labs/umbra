//! One ACP session: its thread, which builds and owns the session's `Agent`, opens or continues
//! the session, runs each prompt as one turn, and answers the requests that need the agent.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use serde_json::{json, Value};

use super::content::{client_mcp_servers, history_updates, merge_mcp_servers, prompt_input, stop_reason, text_block};
use super::events::{AcpEvents, ClientShows, SessionShared};
use super::wire::{RpcError, Wire, AUTH_REQUIRED, INTERNAL_ERROR, INVALID_PARAMS};
use crate::cli::AcpArgs;
use crate::models::config::{Mode, UmbraConfig};
use crate::models::session::{ResumeRecord, SessionInfo};
use crate::repositories::session_repository::find_session;
use crate::repositories::trajectory_repository::trajectory_path;
use crate::services::agent_service::{now_seconds, Agent};
use crate::services::context_service::{discovery_context, expand_slash_command, load_run_context, RunContext};
use crate::services::image_service::images_in_text;
use crate::services::interaction_service::Interaction;
use crate::services::providers::{check_credentials, create_provider};
use crate::services::run_service::{build_agent, prepare_config, AgentSession};
use crate::services::session_service::{load_resumed, ResumedSession};
use crate::utils::interrupt;

/// What the reader thread hands a session thread.
pub enum Command {
    /// A request the session thread answers itself.
    Request { id: Value, method: String, params: Value },
    /// Stop the environment and the MCP servers and end the thread.
    Close,
}

/// A running session, as the reader thread holds it.
pub struct SessionHandle {
    pub shared: Arc<SessionShared>,
    pub sender: Sender<Command>,
    pub thread: JoinHandle<()>,
}

/// What every session thread shares with the server.
pub struct ServerShared {
    pub wire: Arc<Wire>,
    pub args: AcpArgs,
    pub shows: ClientShows,
    pub sessions: Mutex<HashMap<String, SessionHandle>>,
}

/// Starts a session thread and hands it the request that opens the session: `session/new`,
/// `session/load` or `session/resume`.
/// @param server - the server's shared state
/// @param shared - the new session's shared state
/// @param first - the request that opens it
pub fn spawn_session(server: Arc<ServerShared>, shared: Arc<SessionShared>, first: Command) -> SessionHandle {
    let (sender, receiver) = mpsc::channel();
    let thread_shared = shared.clone();
    let thread = std::thread::spawn(move || session_thread(server, thread_shared, first, receiver));
    SessionHandle { shared, sender, thread }
}

/// The session thread: opens the session, then answers requests in order until it is closed.
/// @param server - the server's shared state
/// @param shared - this session's shared state
/// @param first - the request that opens it
/// @param receiver - later requests
fn session_thread(server: Arc<ServerShared>, shared: Arc<SessionShared>, first: Command, receiver: Receiver<Command>) {
    interrupt::set_thread_flag(shared.cancel.clone());
    let Command::Request { id, method, params } = first else { return };
    let mut session = match Session::open(&server, &shared, &method, &params) {
        Ok(session) => session,
        Err(error) => {
            server.wire.respond_error(&id, &error);
            server.sessions.lock().unwrap_or_else(|e| e.into_inner()).remove(&shared.id);
            return;
        }
    };
    session.answer_open(&id, &method, true);
    for command in receiver {
        match command {
            Command::Request { id, method, params } => session.handle(&id, &method, &params),
            Command::Close => break,
        }
    }
    session.agent.close();
}

/// A session's agent and what it was built from.
struct Session {
    server: Arc<ServerShared>,
    shared: Arc<SessionShared>,
    agent: Agent,
    config: UmbraConfig,
    context: RunContext,
    cwd: PathBuf,
    /// False until the first prompt renders the system prompt. A resumed session starts true.
    opened: bool,
}

impl Session {
    /// Builds the session for `session/new`, `session/load` or `session/resume`.
    /// @param server - the server's shared state
    /// @param shared - this session's shared state
    /// @param method - the request that opens it
    /// @param params - its parameters
    fn open(server: &Arc<ServerShared>, shared: &Arc<SessionShared>, method: &str, params: &Value) -> Result<Session, RpcError> {
        let cwd = absolute_cwd(params)?;
        let resumed = match method {
            "session/new" => None,
            _ => Some(load_saved(&shared.id)?),
        };
        let mcp: Vec<Value> = params["mcpServers"].as_array().cloned().unwrap_or_default();
        build_session(server, shared, &cwd, &mcp, resumed)
    }

    /// Answers the request that opened the session, and on first opening tells the editor the
    /// slash commands. `session/load` sends the history first.
    /// @param id - the request id
    /// @param method - `session/new`, `session/load` or `session/resume`
    /// @param first - true when this request built the session
    fn answer_open(&mut self, id: &Value, method: &str, first: bool) {
        if method == "session/load" {
            for update in history_updates(self.agent.messages(), &self.cwd) {
                self.server.wire.update(&self.shared.id, update);
            }
        }
        let mut result = json!({ "modes": modes(&self.shared), "configOptions": config_options(&self.shared) });
        if method == "session/new" {
            result["sessionId"] = json!(self.shared.id);
        }
        self.server.wire.respond(id, result);
        if first {
            self.server.wire.update(&self.shared.id, json!({ "sessionUpdate": "available_commands_update", "availableCommands": available_commands(&self.context) }));
        }
    }

    /// Answers one request from the reader thread.
    /// @param id - the request id
    /// @param method - the method
    /// @param params - its parameters
    fn handle(&mut self, id: &Value, method: &str, params: &Value) {
        match method {
            "session/prompt" => {
                let answer = self.prompt(params);
                self.server.wire.reply(id, answer);
            }
            "session/set_config_option" => {
                let answer = self.set_option(params);
                self.server.wire.reply(id, answer);
            }
            "session/load" | "session/resume" => self.answer_open(id, method, false),
            other => self.server.wire.respond_error(id, &RpcError::new(super::wire::METHOD_NOT_FOUND, format!("no method {other}"))),
        }
    }

    /// Runs one prompt as one turn and answers with the stop reason.
    /// @param params - the `session/prompt` parameters
    fn prompt(&mut self, params: &Value) -> Result<Value, RpcError> {
        interrupt::take();
        let blocks = params["prompt"].as_array().cloned().unwrap_or_default();
        let input = prompt_input(&blocks, &self.config.images);
        for notice in &input.notices {
            self.thought(notice);
        }
        if input.text.is_empty() && input.images.is_empty() {
            return Ok(json!({ "stopReason": "end_turn" }));
        }
        let task = expand_slash_command(&input.text, &self.context);
        let images = self.all_images(&task, input.images);
        if self.opened {
            self.agent.add_task(&task, images);
        } else {
            self.agent.open_new(&task, images).map_err(|e| RpcError::new(INTERNAL_ERROR, format!("{e:#}")))?;
            self.agent.name_session(&input.text);
            self.opened = true;
            let title: String = input.text.lines().next().unwrap_or("").chars().take(80).collect();
            self.server.wire.update(&self.shared.id, json!({ "sessionUpdate": "session_info_update", "title": title }));
        }
        self.agent.start_turn_limits();
        let end = self.agent.run_turn();
        self.agent.end_turn(&end);
        self.finish_turn(&end.status, &end.submission)
    }

    /// Sends the turn's final answer and builds the prompt's result.
    /// @param status - how the turn ended
    /// @param submission - the final output
    fn finish_turn(&self, status: &str, submission: &str) -> Result<Value, RpcError> {
        match status {
            "ModelError" => return Err(RpcError::new(INTERNAL_ERROR, submission.to_string())),
            "RepeatedFormatError" => self.message("The model's replies could not be used: it did not ask for a command in the expected format several times in a row."),
            "LimitsExceeded" => self.message("The step or cost limit for this turn was reached. Send another message to carry on."),
            "TimeExceeded" => self.message("The time limit for this turn was reached. Send another message to carry on."),
            _ => {}
        }
        if status == "Submitted" && !submission.trim().is_empty() {
            self.message(submission.trim());
        }
        Ok(json!({ "stopReason": stop_reason(status) }))
    }

    /// The prompt's images, then the image paths named in its text.
    /// @param task - the task text
    /// @param images - the images the editor sent
    fn all_images(&self, task: &str, mut images: Vec<crate::models::image::ImageData>) -> Vec<crate::models::image::ImageData> {
        let (found, notices) = images_in_text(task, &self.cwd, &self.config.images);
        for notice in notices {
            self.thought(&notice);
        }
        for image in found {
            if !images.iter().any(|i| i.source == image.source) {
                images.push(image);
            }
        }
        images
    }

    /// Changes the model or the mode.
    /// @param params - the `session/set_config_option` parameters
    fn set_option(&mut self, params: &Value) -> Result<Value, RpcError> {
        let value = params["value"].as_str().unwrap_or("").to_string();
        match params["configId"].as_str().unwrap_or("") {
            "mode" => set_mode(&self.shared, &value)?,
            "model" => self.set_model(&value)?,
            other => return Err(RpcError::new(INVALID_PARAMS, format!("no config option {other}"))),
        }
        Ok(json!({ "configOptions": config_options(&self.shared) }))
    }

    /// Switches the session to another model from the next model call on.
    /// @param model - the `provider/model` string
    fn set_model(&mut self, model: &str) -> Result<(), RpcError> {
        check_credentials(&self.config, model).map_err(|e| RpcError::new(INVALID_PARAMS, format!("{e:#}")))?;
        let provider = create_provider(&self.config, model).map_err(|e| RpcError::new(INVALID_PARAMS, format!("{e:#}")))?;
        self.agent.set_provider(provider, model);
        self.config.model = model.to_string();
        *self.shared.model.lock().unwrap_or_else(|e| e.into_inner()) = model.to_string();
        Ok(())
    }

    /// Sends text as the agent's message.
    /// @param text - the text
    fn message(&self, text: &str) {
        self.server.wire.update(&self.shared.id, json!({ "sessionUpdate": "agent_message_chunk", "content": text_block(text) }));
    }

    /// Sends text as the agent's thought, for notices outside the agent loop.
    /// @param text - the text
    fn thought(&self, text: &str) {
        self.server.wire.update(&self.shared.id, json!({ "sessionUpdate": "agent_thought_chunk", "content": text_block(text) }));
    }
}

/// Reads the absolute working folder of a session request.
/// @param params - the request parameters
fn absolute_cwd(params: &Value) -> Result<PathBuf, RpcError> {
    let Some(cwd) = params["cwd"].as_str() else { return Err(RpcError::new(INVALID_PARAMS, "cwd is required")) };
    let path = PathBuf::from(cwd);
    if !path.is_absolute() {
        return Err(RpcError::new(INVALID_PARAMS, format!("cwd must be an absolute path, not {cwd}")));
    }
    if !path.is_dir() {
        return Err(RpcError::new(INVALID_PARAMS, format!("no folder {cwd}")));
    }
    Ok(path)
}

/// Loads a saved session by id, for `session/load` and `session/resume`.
/// @param id - the session id
fn load_saved(id: &str) -> Result<ResumedSession, RpcError> {
    let entry = find_session(id).map_err(|_| RpcError::no_session(id))?;
    let resumed = load_resumed(Path::new(&entry.trajectory)).map_err(|e| RpcError::new(INTERNAL_ERROR, format!("{e:#}")))?;
    if resumed.session.id != id {
        return Err(RpcError::no_session(id));
    }
    Ok(resumed)
}

/// Builds the agent of a session: the config for its folder, the context, the editor's MCP servers,
/// the model, and the history of a saved session.
/// @param server - the server's shared state
/// @param shared - this session's shared state
/// @param cwd - the session's working folder
/// @param mcp - the editor's `mcpServers`
/// @param resumed - the saved session, for load and resume
fn build_session(server: &Arc<ServerShared>, shared: &Arc<SessionShared>, cwd: &Path, mcp: &[Value], resumed: Option<ResumedSession>) -> Result<Session, RpcError> {
    let config = session_config(&server.args, cwd, resumed.as_ref(), shared)?;
    let ctx = discovery_context(&config, cwd);
    let mut context = load_run_context(&config, &ctx);
    let (from_client, problems) = client_mcp_servers(mcp);
    for problem in problems {
        eprintln!("umbra: {problem}");
    }
    if config.mcp.enabled {
        context.mcp_servers = merge_mcp_servers(std::mem::take(&mut context.mcp_servers), from_client);
    }
    let (session, path) = session_record(&config, &shared.id, cwd, resumed.as_ref());
    let interaction = Interaction::detached(Mode::Yolo);
    let events = AcpEvents::new(server.wire.clone(), shared.clone(), server.shows, &config.whitelist_actions, cwd.to_path_buf());
    let run = AgentSession { session, trajectory_path: path, need_provider: true, events: Box::new(events) };
    let mut agent = build_agent(config.clone(), cwd, &context, interaction, run).map_err(|e| RpcError::new(INTERNAL_ERROR, format!("{e:#}")))?;
    let opened = resumed.is_some();
    if let Some(resumed) = resumed {
        agent.open_resumed(resumed);
    }
    Ok(Session { server: server.clone(), shared: shared.clone(), agent, config, context, cwd: cwd.to_path_buf(), opened })
}

/// The config of a session: the files for its folder, the `umbra acp` options, and a check that
/// the model can be used. A saved session keeps its own model unless `-m` says otherwise.
/// @param args - the `umbra acp` options
/// @param cwd - the session's working folder
/// @param resumed - the saved session, if any
/// @param shared - this session's shared state, which gets the mode and the model
fn session_config(args: &AcpArgs, cwd: &Path, resumed: Option<&ResumedSession>, shared: &SessionShared) -> Result<UmbraConfig, RpcError> {
    let mut common = args.common.clone();
    common.cwd = Some(cwd.to_path_buf());
    if let Some(resumed) = resumed {
        if common.model.is_none() && !resumed.session.model.is_empty() {
            common.model = Some(resumed.session.model.clone());
        }
    }
    let (mut config, _) = prepare_config(&common).map_err(|e| RpcError::new(INVALID_PARAMS, format!("{e:#}")))?;
    config.status_bar = false;
    config.confirm_exit = false;
    if let Some(limit) = args.cost_limit {
        config.cost_limit = limit;
    }
    if let Some(limit) = args.step_limit {
        config.step_limit = limit;
    }
    if let Err(error) = check_credentials(&config, &config.model) {
        return Err(RpcError { code: AUTH_REQUIRED, message: format!("{error:#}. Run `umbra setup` in a terminal, or pick the \"Set up Umbra in a terminal\" sign in"), data: None });
    }
    let mode = start_mode(args, config.mode).map_err(|e| RpcError::new(INVALID_PARAMS, e))?;
    *shared.mode.lock().unwrap_or_else(|e| e.into_inner()) = mode;
    *shared.model.lock().unwrap_or_else(|e| e.into_inner()) = config.model.clone();
    *shared.models.lock().unwrap_or_else(|e| e.into_inner()) = config.models.clone();
    Ok(config)
}

/// The mode a session starts in: `--yolo` or `--mode`, else the config's mode. Human mode is not
/// offered to an editor, so it starts in confirm mode.
/// @param args - the `umbra acp` options
/// @param configured - the mode in the config files
pub fn start_mode(args: &AcpArgs, configured: Mode) -> Result<Mode, String> {
    if args.yolo {
        return Ok(Mode::Yolo);
    }
    match args.mode.as_deref() {
        Some("yolo") => Ok(Mode::Yolo),
        Some("confirm") => Ok(Mode::Confirm),
        Some(other) => Err(format!("unknown mode '{other}'. Use confirm or yolo")),
        None if configured == Mode::Yolo => Ok(Mode::Yolo),
        None => Ok(Mode::Confirm),
    }
}

/// The session block and trajectory file. A new session gets this id. A saved one keeps its file
/// and records the resume, the way `umbra --resume` does.
/// @param config - the session's config
/// @param id - the session id
/// @param cwd - the working folder
/// @param resumed - the saved session, if any
fn session_record(config: &UmbraConfig, id: &str, cwd: &Path, resumed: Option<&ResumedSession>) -> (SessionInfo, PathBuf) {
    let cwd_text = cwd.to_string_lossy().replace('\\', "/");
    let Some(resumed) = resumed else {
        let session = SessionInfo { id: id.to_string(), cwd: cwd_text, model: config.model.clone(), task: String::new(), started_at: now_seconds(), resumes: Vec::new() };
        return (session, trajectory_path(&config.output, id));
    };
    let mut session = resumed.session.clone();
    session.resumes.push(ResumeRecord { at: now_seconds(), model: config.model.clone(), messages_before: resumed.messages.len(), forked_from: String::new() });
    session.model = config.model.clone();
    session.cwd = cwd_text;
    (session, resumed.trajectory_path.clone())
}

/// Changes a session's mode.
/// @param shared - the session's shared state
/// @param mode - `confirm` or `yolo`
pub fn set_mode(shared: &SessionShared, mode: &str) -> Result<(), RpcError> {
    let mode = match mode {
        "confirm" => Mode::Confirm,
        "yolo" => Mode::Yolo,
        other => return Err(RpcError::new(INVALID_PARAMS, format!("no mode {other}. Use confirm or yolo"))),
    };
    *shared.mode.lock().unwrap_or_else(|e| e.into_inner()) = mode;
    Ok(())
}

/// The id ACP uses for a mode.
/// @param mode - the mode
fn mode_id(mode: Mode) -> &'static str {
    if mode == Mode::Yolo { "yolo" } else { "confirm" }
}

/// The session's modes, for the older `session/set_mode`.
/// @param shared - the session's shared state
pub fn modes(shared: &SessionShared) -> Value {
    json!({ "currentModeId": mode_id(shared.mode()), "availableModes": [
        { "id": "confirm", "name": "Confirm", "description": "Ask before running each command" },
        { "id": "yolo", "name": "Yolo", "description": "Run commands without asking" },
    ] })
}

/// The session's config options: the mode and the model.
/// @param shared - the session's shared state
pub fn config_options(shared: &SessionShared) -> Value {
    let current = shared.model.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut models = vec![current.clone()];
    for model in shared.models.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        if !models.contains(model) {
            models.push(model.clone());
        }
    }
    let choices: Vec<Value> = models.iter().map(|m| json!({ "value": m, "name": m })).collect();
    json!([
        { "id": "mode", "name": "Mode", "description": "Whether Umbra asks before running a command", "category": "mode", "type": "select", "currentValue": mode_id(shared.mode()), "options": [
            { "value": "confirm", "name": "Confirm", "description": "Ask before running each command" },
            { "value": "yolo", "name": "Yolo", "description": "Run commands without asking" },
        ] },
        { "id": "model", "name": "Model", "description": "The model Umbra asks, as provider/model", "category": "model", "type": "select", "currentValue": current, "options": choices },
    ])
}

/// The slash commands and skills, for `available_commands_update`.
/// @param context - the session's loaded commands and skills
fn available_commands(context: &RunContext) -> Vec<Value> {
    let mut commands: Vec<Value> = context.commands.iter().map(|c| json!({ "name": c.name, "description": short(&c.description, &format!("Run {}", c.path.display())), "input": { "hint": "arguments" } })).collect();
    for skill in &context.skills {
        if !context.commands.iter().any(|c| c.name == skill.name) {
            commands.push(json!({ "name": skill.name, "description": short(&skill.description, "A skill"), "input": { "hint": "what to do" } }));
        }
    }
    commands
}

/// A description shortened to one line of at most 200 characters, or the fallback when empty.
/// @param text - the description
/// @param fallback - used when there is none
fn short(text: &str, fallback: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty() { fallback.to_string() } else { line.chars().take(200).collect() }
}

/// Checks that Umbra could start a session in a folder: a model is set and it has a credential.
/// @param args - the `umbra acp` options
/// @param cwd - the folder
pub fn check_ready(args: &AcpArgs, cwd: &Path) -> Result<(), RpcError> {
    let shared = SessionShared::new("check", Mode::Confirm);
    session_config(args, cwd, None, &shared).map(|_| ())
}
