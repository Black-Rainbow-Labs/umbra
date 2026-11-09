//! `umbra acp`: Umbra as an Agent Client Protocol server, for editors such as Zed, the JetBrains
//! IDEs and Unluminous. The editor starts this process and sends JSON-RPC on stdin, one message a
//! line; Umbra answers on stdout and writes its logs to stderr.
//!
//! This thread reads stdin. It answers what needs no agent itself (`initialize`, `authenticate`,
//! `session/list`, `session/set_mode`, `session/close`), sets a session's cancel flag on
//! `session/cancel`, and hands an answer from the editor to the thread waiting for it. Everything
//! that needs a session's agent goes to that session's own thread. It never waits on a session,
//! so a cancel is read while a turn runs. `tasks/umbra-acp-distribution-tdd.md` is the design.

pub mod content;
pub mod events;
pub mod session;
pub mod wire;

use std::io::BufRead;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde_json::{json, Value};

use crate::cli::AcpArgs;
use crate::repositories::session_repository::{list_sessions, new_session_id, sessions_in};
use crate::utils::console;
use content::iso_time;
use events::{ClientShows, SessionShared};
use session::{check_ready, config_options, set_mode, spawn_session, start_mode, Command, ServerShared};
use wire::{error_from, RpcError, Wire, INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND, PARSE_ERROR};

/// The ACP version Umbra speaks.
pub const PROTOCOL_VERSION: u64 = 1;

/// How long the sessions get to stop once the editor closes stdin.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(10);

/// Serves ACP on stdin and stdout until the editor closes stdin. Returns the exit code.
/// @param args - the `umbra acp` options
pub fn serve(args: &AcpArgs) -> Result<i32> {
    console::set_quiet(true);
    console::use_stderr();
    console::enable_status_bar(false);
    start_mode(args, crate::models::config::Mode::Confirm).map_err(anyhow::Error::msg)?;
    let wire = Arc::new(Wire::new(Box::new(std::io::stdout())));
    let mut server = Server { wire, args: args.clone(), shared: None };
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if !line.trim().is_empty() {
            server.handle_line(&line);
        }
    }
    server.shutdown();
    Ok(0)
}

/// The reader thread's state.
struct Server {
    wire: Arc<Wire>,
    args: AcpArgs,
    /// Set by `initialize`.
    shared: Option<Arc<ServerShared>>,
}

impl Server {
    /// Reads one line from the editor and acts on it.
    /// @param line - one JSON-RPC message
    fn handle_line(&mut self, line: &str) {
        let message: Value = match serde_json::from_str(line) {
            Ok(message) => message,
            Err(error) => return self.wire.respond_error(&Value::Null, &RpcError::new(PARSE_ERROR, format!("not JSON: {error}"))),
        };
        let id = message.get("id").cloned();
        match (message["method"].as_str(), id) {
            (Some(method), Some(id)) => {
                let answer = self.request(&id, method, &message["params"]);
                if let Some(answer) = answer {
                    self.wire.reply(&id, answer);
                }
            }
            (Some(method), None) => self.notification(method, &message["params"]),
            (None, Some(id)) if message.get("error").is_some() => self.wire.deliver(&id, Err(error_from(&message["error"]))),
            (None, Some(id)) => self.wire.deliver(&id, Ok(message["result"].clone())),
            (None, None) => self.wire.respond_error(&Value::Null, &RpcError::new(INVALID_REQUEST, "not a JSON-RPC message")),
        }
    }

    /// Answers a request, or hands it to a session thread and returns None; that thread answers.
    /// @param id - the request id
    /// @param method - the method
    /// @param params - its parameters
    fn request(&mut self, id: &Value, method: &str, params: &Value) -> Option<Result<Value, RpcError>> {
        if method == "initialize" {
            return Some(Ok(self.initialize(params)));
        }
        let Some(shared) = self.shared.clone() else {
            return Some(Err(RpcError::new(INVALID_REQUEST, "initialize first")));
        };
        match method {
            "authenticate" => Some(authenticate(&self.args, params)),
            "session/new" => self.open(&shared, id, method, params, None),
            "session/load" | "session/resume" => {
                let session_id = params["sessionId"].as_str().unwrap_or("").to_string();
                self.open(&shared, id, method, params, Some(session_id))
            }
            "session/prompt" => forward(&shared, id, method, params),
            "session/set_config_option" if params["configId"] == "mode" => Some(with_session(&shared, params, |s| set_mode(s, params["value"].as_str().unwrap_or("")).map(|_| json!({ "configOptions": config_options(s) })))),
            "session/set_config_option" => forward(&shared, id, method, params),
            "session/set_mode" => Some(with_session(&shared, params, |s| set_mode(s, params["modeId"].as_str().unwrap_or("")).map(|_| json!({})))),
            "session/list" => Some(Ok(list(params))),
            "session/close" => Some(close(&shared, params)),
            other => Some(Err(RpcError::new(METHOD_NOT_FOUND, format!("Umbra has no method {other}")))),
        }
    }

    /// Answers `initialize`: the protocol version, what Umbra can do, and the sign in methods.
    /// @param params - the editor's capabilities and information
    fn initialize(&mut self, params: &Value) -> Value {
        let capabilities = &params["clientCapabilities"];
        let shows = ClientShows { notices: capabilities["session"]["notices"].is_object() };
        self.shared = Some(Arc::new(ServerShared { wire: self.wire.clone(), args: self.args.clone(), shows, sessions: Mutex::new(Default::default()) }));
        let mut methods = vec![json!({ "id": "umbra-config", "name": "Use Umbra's configured model", "description": "Uses the model in ~/.umbra/config.toml or the -m option, with ANTHROPIC_API_KEY, your Claude Code login, another provider's API key, or a local model server", "type": "agent" })];
        if capabilities["auth"]["terminal"].as_bool().unwrap_or(false) {
            methods.push(json!({ "id": "umbra-setup", "name": "Set up Umbra in a terminal", "description": "Choose a model and save it in ~/.umbra/config.toml", "type": "terminal", "args": ["--setup"] }));
        }
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "agentInfo": { "name": "umbra", "title": "Umbra", "version": env!("CARGO_PKG_VERSION") },
            "agentCapabilities": {
                "loadSession": true,
                "promptCapabilities": { "image": true, "audio": false, "embeddedContext": true },
                "mcpCapabilities": { "http": true, "sse": true },
                "sessionCapabilities": { "list": {}, "resume": {}, "close": {} },
            },
            "authMethods": methods,
        })
    }

    /// Opens a session on a new thread, or hands load and resume of an open one to its thread.
    /// @param shared - the server's shared state
    /// @param id - the request id
    /// @param method - `session/new`, `session/load` or `session/resume`
    /// @param params - its parameters
    /// @param existing - the session id, for load and resume
    fn open(&mut self, shared: &Arc<ServerShared>, id: &Value, method: &str, params: &Value, existing: Option<String>) -> Option<Result<Value, RpcError>> {
        let mut sessions = shared.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let session_id = match existing {
            Some(session_id) if session_id.is_empty() => return Some(Err(RpcError::new(INVALID_PARAMS, "sessionId is required"))),
            Some(session_id) => {
                if let Some(handle) = sessions.get(&session_id) {
                    let _ = handle.sender.send(Command::Request { id: id.clone(), method: method.into(), params: params.clone() });
                    return None;
                }
                session_id
            }
            None => unique_session_id(&sessions),
        };
        let mode = start_mode(&self.args, crate::models::config::Mode::Confirm).unwrap_or(crate::models::config::Mode::Confirm);
        let session = Arc::new(SessionShared::new(&session_id, mode));
        let first = Command::Request { id: id.clone(), method: method.into(), params: params.clone() };
        let handle = spawn_session(shared.clone(), session, first);
        sessions.insert(session_id, handle);
        None
    }

    /// Acts on a notification. `session/cancel` sets the session's cancel flag; the rest are ignored.
    /// @param method - the method
    /// @param params - its parameters
    fn notification(&mut self, method: &str, params: &Value) {
        if method != "session/cancel" {
            return;
        }
        let Some(shared) = &self.shared else { return };
        let sessions = shared.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(handle) = sessions.get(params["sessionId"].as_str().unwrap_or("")) {
            handle.shared.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// After stdin closes: cancels and closes every session and waits a while for them to stop
    /// their commands, MCP servers and containers.
    fn shutdown(&mut self) {
        self.wire.abandon_pending();
        let Some(shared) = &self.shared else { return };
        let handles: Vec<_> = shared.sessions.lock().unwrap_or_else(|e| e.into_inner()).drain().map(|(_, h)| h).collect();
        for handle in &handles {
            handle.shared.cancel.store(true, Ordering::SeqCst);
            let _ = handle.sender.send(Command::Close);
        }
        let deadline = Instant::now() + SHUTDOWN_WAIT;
        while handles.iter().any(|h| !h.thread.is_finished()) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// A new session id that no open session has. Ids are the time in milliseconds.
/// @param sessions - the open sessions
fn unique_session_id(sessions: &std::collections::HashMap<String, session::SessionHandle>) -> String {
    loop {
        let id = new_session_id();
        if !sessions.contains_key(&id) && !crate::repositories::session_repository::sessions_dir().join(format!("{id}.json")).exists() {
            return id;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Hands a request to its session's thread, or answers that there is no such session.
/// @param shared - the server's shared state
/// @param id - the request id
/// @param method - the method
/// @param params - its parameters
fn forward(shared: &ServerShared, id: &Value, method: &str, params: &Value) -> Option<Result<Value, RpcError>> {
    let session_id = params["sessionId"].as_str().unwrap_or("");
    let sessions = shared.sessions.lock().unwrap_or_else(|e| e.into_inner());
    let Some(handle) = sessions.get(session_id) else { return Some(Err(RpcError::no_session(session_id))) };
    let _ = handle.sender.send(Command::Request { id: id.clone(), method: method.into(), params: params.clone() });
    None
}

/// Runs `action` on a session's shared state, or answers that there is no such session.
/// @param shared - the server's shared state
/// @param params - the request parameters, with `sessionId`
/// @param action - what to do with the session
fn with_session(shared: &ServerShared, params: &Value, action: impl FnOnce(&SessionShared) -> Result<Value, RpcError>) -> Result<Value, RpcError> {
    let session_id = params["sessionId"].as_str().unwrap_or("");
    let sessions = shared.sessions.lock().unwrap_or_else(|e| e.into_inner());
    match sessions.get(session_id) {
        Some(handle) => action(&handle.shared),
        None => Err(RpcError::no_session(session_id)),
    }
}

/// Answers `authenticate`: Umbra has no sign in of its own, so this checks that the configured
/// model has a credential and says what to set when it does not.
/// @param args - the `umbra acp` options
/// @param params - the method id
fn authenticate(args: &AcpArgs, params: &Value) -> Result<Value, RpcError> {
    let method = params["methodId"].as_str().unwrap_or("");
    if method != "umbra-config" && method != "umbra-setup" {
        return Err(RpcError::new(INVALID_PARAMS, format!("no sign in method {method}")));
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    check_ready(args, &cwd).map(|_| json!({}))
}

/// Answers `session/list` from the session index: the folder's sessions, or every session.
/// @param params - optional `cwd`
fn list(params: &Value) -> Value {
    let sessions = match params["cwd"].as_str() {
        Some(cwd) => sessions_in(Path::new(cwd)),
        None => list_sessions(),
    };
    let listed: Vec<Value> = sessions.iter().map(|s| json!({ "sessionId": s.id, "cwd": s.cwd, "title": s.task.lines().next().unwrap_or(""), "updatedAt": iso_time(s.updated_at) })).collect();
    json!({ "sessions": listed })
}

/// Answers `session/close`: cancels the session's turn, and its thread stops the environment and
/// the MCP servers and ends.
/// @param shared - the server's shared state
/// @param params - the session id
fn close(shared: &ServerShared, params: &Value) -> Result<Value, RpcError> {
    let session_id = params["sessionId"].as_str().unwrap_or("");
    if let Some(handle) = shared.sessions.lock().unwrap_or_else(|e| e.into_inner()).remove(session_id) {
        handle.shared.cancel.store(true, Ordering::SeqCst);
        let _ = handle.sender.send(Command::Close);
    }
    Ok(json!({}))
}
