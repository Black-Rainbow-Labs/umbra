//! The MCP client side of Umbra: start the configured servers in the background, keep the state of
//! each one, collect the tools of the ones that are ready, and route calls. A run with a terminal
//! never waits for a server: its tools join from the next model call once it is ready. A run with
//! no terminal waits until every server has answered or the start timeout has passed.

pub mod client;
pub mod http;
pub mod stdio;
pub mod transport;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::models::action::Observation;
use crate::models::config::{ImagesConfig, McpConfig};
use crate::models::context::{McpServerDefinition, McpTool};
use crate::utils::interrupt;
use client::McpClient;

/// A server that could not be started, and why.
#[derive(Debug, Clone)]
pub struct McpFailure {
    pub name: String,
    pub error: String,
}

/// Where one server is.
#[derive(Debug, Clone)]
pub enum ServerState {
    /// Turned off for this session, or left out by `only` or `disabled` in the config.
    Off,
    Starting { since: Instant },
    Ready { tools: Vec<McpTool>, protocol: String, instructions: String },
    Failed { error: String, log_path: PathBuf, at: Instant },
}

/// One registered server.
struct Server {
    definition: McpServerDefinition,
    state: ServerState,
    /// The connection while the server is ready. Calls lock it, so reading the state never waits
    /// for a call.
    client: Option<Arc<Mutex<McpClient>>>,
    /// Counts the starts, so a start that finishes after the server was turned off or restarted
    /// is thrown away.
    attempt: u64,
}

/// The session's MCP servers.
pub struct McpManager {
    servers: Mutex<BTreeMap<String, Server>>,
    settings: McpConfig,
    log_dir: PathBuf,
    images: Mutex<ImagesConfig>,
    /// Goes up whenever a server's state changes.
    version: AtomicU64,
}

impl Default for McpManager {
    /// A manager with no servers.
    fn default() -> Self {
        McpManager { servers: Mutex::new(BTreeMap::new()), settings: McpConfig::default(), log_dir: PathBuf::new(), images: Mutex::new(ImagesConfig::default()), version: AtomicU64::new(0) }
    }
}

impl McpManager {
    /// Starts every selected server on its own thread and returns at once. Servers left out by the
    /// config are registered as Off, so the panel can turn them on.
    /// @param definitions - the servers found in config files
    /// @param settings - the MCP settings
    /// @param log_dir - where stdio servers' stderr goes
    /// @param off - names turned off for this session, from a resumed session
    pub fn start(definitions: &[McpServerDefinition], settings: &McpConfig, log_dir: &Path, off: &[String]) -> Arc<McpManager> {
        let manager = Arc::new(McpManager { settings: settings.clone(), log_dir: log_dir.to_path_buf(), ..Default::default() });
        let mut names = Vec::new();
        {
            let mut servers = manager.lock();
            for definition in definitions {
                servers.insert(definition.name.clone(), Server { definition: definition.clone(), state: ServerState::Off, client: None, attempt: 0 });
                if is_selected(&definition.name, settings) && !off.contains(&definition.name) {
                    names.push(definition.name.clone());
                }
            }
        }
        for name in names {
            manager.launch(&name);
        }
        manager
    }

    /// Starts every selected server and waits until each has answered or the start timeout passed.
    /// For `umbra mcp --connect`, which has nothing else to do meanwhile.
    /// @param definitions - the servers found in config files
    /// @param settings - the MCP settings
    /// @param log_dir - where stdio servers' stderr goes
    pub fn connect_all(definitions: &[McpServerDefinition], settings: &McpConfig, log_dir: &Path) -> Arc<McpManager> {
        let manager = McpManager::start(definitions, settings, log_dir, &[]);
        manager.wait_settled(Instant::now() + Duration::from_secs(settings.startup_timeout_seconds.max(1) + 2));
        manager
    }

    /// The servers, locked.
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Server>> {
        self.servers.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Records a change: the version goes up and the screen is drawn again.
    fn changed(&self) {
        self.version.fetch_add(1, Ordering::SeqCst);
        crate::utils::screen::poke();
        crate::utils::screen::redraw();
    }

    /// Starts one server on a thread, with a second thread that fails it when the start timeout
    /// passes first. A server that missed the deadline and connects later is closed.
    /// @param name - the server name
    fn launch(self: &Arc<Self>, name: &str) {
        let startup = Duration::from_secs(self.settings.startup_timeout_seconds.max(1));
        let call = Duration::from_secs(self.settings.call_timeout_seconds.max(1));
        let (definition, attempt) = {
            let mut servers = self.lock();
            let Some(server) = servers.get_mut(name) else { return };
            server.attempt += 1;
            server.state = ServerState::Starting { since: Instant::now() };
            server.client = None;
            (server.definition.clone(), server.attempt)
        };
        self.changed();
        let (manager, log_dir) = (self.clone(), self.log_dir.clone());
        std::thread::spawn(move || {
            let result = McpClient::connect(&definition, startup, call, &log_dir).map_err(|e| format!("{e:#}"));
            manager.finish_start(&definition.name, attempt, result);
        });
        let manager = self.clone();
        let name = name.to_string();
        std::thread::spawn(move || {
            std::thread::sleep(startup + Duration::from_millis(500));
            manager.finish_start(&name, attempt, Err(format!("did not finish starting within {} seconds", startup.as_secs())));
        });
    }

    /// Records how a start ended, when it is still the server's current start and the server is
    /// still starting. A connection that arrives too late is closed.
    /// @param name - the server name
    /// @param attempt - which start this was
    /// @param result - the connected client, or the error
    fn finish_start(&self, name: &str, attempt: u64, result: Result<McpClient, String>) {
        let mut late = None;
        {
            let mut servers = self.lock();
            let Some(server) = servers.get_mut(name) else { return };
            let current = server.attempt == attempt && matches!(server.state, ServerState::Starting { .. });
            match (current, result) {
                (true, Ok(client)) => {
                    server.state = ServerState::Ready { tools: client.tools.clone(), protocol: client.protocol_version.clone(), instructions: client.instructions.clone() };
                    server.client = Some(Arc::new(Mutex::new(client)));
                }
                (true, Err(error)) => {
                    server.state = ServerState::Failed { error, log_path: log_path_for(&self.log_dir, &server.definition), at: Instant::now() };
                }
                (false, Ok(client)) => late = Some(client),
                (false, Err(_)) => return,
            }
        }
        if let Some(mut client) = late {
            client.close();
            return;
        }
        self.changed();
    }

    /// One entry per registered server, in name order.
    pub fn snapshot(&self) -> Vec<(String, ServerState)> {
        self.lock().iter().map(|(name, server)| (name.clone(), server.state.clone())).collect()
    }

    /// A number that changes whenever a server's state changes.
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }

    /// True while any server is starting.
    pub fn any_starting(&self) -> bool {
        self.lock().values().any(|s| matches!(s.state, ServerState::Starting { .. }))
    }

    /// The tools of every server that is ready, read before each model call.
    pub fn ready_tools(&self) -> Vec<McpTool> {
        self.lock().values().flat_map(|s| match &s.state { ServerState::Ready { tools, .. } => tools.clone(), _ => Vec::new() }).collect()
    }

    /// Every tool from every ready server. The same as `ready_tools`.
    pub fn tools(&self) -> Vec<McpTool> {
        self.ready_tools()
    }

    /// The ready servers, with their protocol versions and tool counts.
    pub fn connected(&self) -> Vec<(String, String, usize)> {
        self.lock().iter().filter_map(|(name, s)| match &s.state { ServerState::Ready { tools, protocol, .. } => Some((name.clone(), protocol.clone(), tools.len())), _ => None }).collect()
    }

    /// The servers that failed to start, with their errors.
    pub fn failures(&self) -> Vec<McpFailure> {
        self.lock().iter().filter_map(|(name, s)| match &s.state { ServerState::Failed { error, .. } => Some(McpFailure { name: name.clone(), error: error.clone() }), _ => None }).collect()
    }

    /// Server instructions to add to the system prompt, by server name.
    pub fn instructions(&self) -> Vec<(String, String)> {
        self.lock().iter().filter_map(|(name, s)| match &s.state { ServerState::Ready { instructions, .. } if !instructions.trim().is_empty() => Some((name.clone(), instructions.clone())), _ => None }).collect()
    }

    /// The servers turned off for this session, which a resumed session turns off again.
    pub fn off_names(&self) -> Vec<String> {
        self.lock().iter().filter(|(name, s)| matches!(s.state, ServerState::Off) && is_selected(name, &self.settings)).map(|(name, _)| name.clone()).collect()
    }

    /// Turns one server on or off for this session. Off closes it, so its process ends and its
    /// tools are absent from the next model call. On starts it again.
    /// @param name - the server name
    /// @param on - true to turn it on
    pub fn set_enabled(self: &Arc<Self>, name: &str, on: bool) {
        if on {
            let off = self.lock().get(name).is_some_and(|s| matches!(s.state, ServerState::Off | ServerState::Failed { .. }));
            if off {
                self.launch(name);
            }
            return;
        }
        let client = {
            let mut servers = self.lock();
            let Some(server) = servers.get_mut(name) else { return };
            server.attempt += 1;
            server.state = ServerState::Off;
            server.client.take()
        };
        if let Some(client) = client {
            std::thread::spawn(move || client.lock().unwrap_or_else(|e| e.into_inner()).close());
        }
        self.changed();
    }

    /// Starts a failed server again.
    /// @param name - the server name
    pub fn restart(self: &Arc<Self>, name: &str) {
        let failed = self.lock().get(name).is_some_and(|s| matches!(s.state, ServerState::Failed { .. }));
        if failed {
            self.launch(name);
        }
    }

    /// Waits until no server is starting, the deadline passes, or Ctrl+C is pressed. Returns true
    /// when every server settled.
    /// @param deadline - when to stop waiting
    pub fn wait_settled(&self, deadline: Instant) -> bool {
        loop {
            if !self.any_starting() {
                return true;
            }
            if Instant::now() >= deadline || interrupt::is_pending() {
                return false;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// Calls a tool on a server. A server that is not ready gets an observation saying why.
    /// @param server - the server name
    /// @param tool - the tool name
    /// @param arguments - the arguments object
    pub fn call(&self, server: &str, tool: &str, arguments: Value) -> Observation {
        let (client, state) = match self.lock().get(server) {
            Some(s) => (s.client.clone(), Some(s.state.clone())),
            None => (None, None),
        };
        match client {
            Some(client) => {
                let images = self.images.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let observation = client.lock().unwrap_or_else(|e| e.into_inner()).call_tool(tool, arguments, &images);
                observation
            }
            None => Observation::failed("", format!("MCP server '{server}' is not connected{}", not_ready_reason(state.as_ref()))),
        }
    }

    /// Sets the limits used for images that tools return.
    /// @param settings - the image settings
    pub fn set_image_settings(&self, settings: ImagesConfig) {
        *self.images.lock().unwrap_or_else(|e| e.into_inner()) = settings;
    }

    /// Shuts every server down.
    pub fn close(&self) {
        let clients: Vec<Arc<Mutex<McpClient>>> = {
            let mut servers = self.lock();
            servers.values_mut().filter_map(|s| {
                s.attempt += 1;
                s.client.take()
            }).collect()
        };
        for client in clients {
            client.lock().unwrap_or_else(|e| e.into_inner()).close();
        }
    }
}

/// What an observation adds when a call goes to a server that is not ready.
/// @param state - the server's state, or None for a name that is not registered
fn not_ready_reason(state: Option<&ServerState>) -> String {
    match state {
        Some(ServerState::Starting { .. }) => " (it is still starting)".into(),
        Some(ServerState::Failed { error, .. }) => format!(" (it failed to start: {})", error.lines().next().unwrap_or("")),
        Some(ServerState::Off) => " (it is turned off for this session)".into(),
        _ => String::new(),
    }
}

/// The log file a stdio server's stderr is written to.
/// @param log_dir - the MCP log folder
/// @param definition - the server
fn log_path_for(log_dir: &Path, definition: &McpServerDefinition) -> PathBuf {
    log_dir.join(format!("{}.log", client::sanitize(&definition.name)))
}

/// Applies the `only` and `disabled` lists.
/// @param name - the server name
/// @param config - the MCP settings
pub fn is_selected(name: &str, config: &McpConfig) -> bool {
    if config.disabled.iter().any(|d| d == name) {
        return false;
    }
    config.only.is_empty() || config.only.iter().any(|o| o == name)
}

/// A one line summary of the servers for the header: `⠋ starting 2 of 5`, `5 running`,
/// `4 running · 1 failed`, `3 running · 2 off`, or `none`. Returns the parts and whether each is
/// a failure, which is drawn red.
/// @param states - the snapshot
/// @param spinner - the spinner character for this frame
pub fn summary(states: &[(String, ServerState)], spinner: char) -> Vec<(String, bool)> {
    if states.is_empty() {
        return vec![("none".into(), false)];
    }
    let count = |f: fn(&ServerState) -> bool| states.iter().filter(|(_, s)| f(s)).count();
    let starting = count(|s| matches!(s, ServerState::Starting { .. }));
    let running = count(|s| matches!(s, ServerState::Ready { .. }));
    let failed = count(|s| matches!(s, ServerState::Failed { .. }));
    let off = count(|s| matches!(s, ServerState::Off));
    if starting > 0 {
        return vec![(format!("{spinner} starting {} of {}", running + failed + 1, states.len() - off), false)];
    }
    let mut parts = vec![(format!("{running} running"), false)];
    if failed > 0 {
        parts.push((format!("{failed} failed"), true));
    }
    if off > 0 {
        parts.push((format!("{off} off"), false));
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_names_starting_running_failed_and_off_servers() {
        let failed = ServerState::Failed { error: "x".into(), log_path: PathBuf::new(), at: Instant::now() };
        let ready = ServerState::Ready { tools: Vec::new(), protocol: String::new(), instructions: String::new() };
        let states = vec![("a".to_string(), ready.clone()), ("b".to_string(), ServerState::Starting { since: Instant::now() }), ("c".to_string(), ServerState::Off)];
        assert_eq!(summary(&states, '⠋'), vec![("⠋ starting 2 of 2".to_string(), false)]);
        let states = vec![("a".to_string(), ready.clone()), ("b".to_string(), failed), ("c".to_string(), ServerState::Off)];
        assert_eq!(summary(&states, '⠋'), vec![("1 running".to_string(), false), ("1 failed".to_string(), true), ("1 off".to_string(), false)]);
        assert_eq!(summary(&[], '⠋'), vec![("none".to_string(), false)]);
    }
}
