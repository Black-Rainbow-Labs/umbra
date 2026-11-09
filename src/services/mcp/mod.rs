//! The MCP client side of Umbra: start the configured servers, collect their tools, route calls.

pub mod client;
pub mod http;
pub mod stdio;
pub mod transport;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::models::action::Observation;
use crate::models::config::{ImagesConfig, McpConfig};
use crate::models::context::{McpServerDefinition, McpTool};
use client::McpClient;

/// A server that could not be started, and why.
#[derive(Debug, Clone)]
pub struct McpFailure {
    pub name: String,
    pub error: String,
}

/// Every connected server, by name.
#[derive(Default)]
pub struct McpManager {
    clients: BTreeMap<String, McpClient>,
    pub failures: Vec<McpFailure>,
    images: ImagesConfig,
}

impl McpManager {
    /// Starts every selected server at once and waits up to the startup timeout for all of them.
    /// A server that fails or is too slow is recorded in `failures` and left out; the run goes on.
    /// @param definitions - the servers found in config files
    /// @param config - the MCP settings
    /// @param log_dir - where stdio servers' stderr goes
    pub fn connect_all(definitions: &[McpServerDefinition], config: &McpConfig, log_dir: &Path) -> Self {
        let startup = Duration::from_secs(config.startup_timeout_seconds.max(1));
        let call = Duration::from_secs(config.call_timeout_seconds.max(1));
        let (sender, receiver) = mpsc::channel();
        let selected: Vec<McpServerDefinition> = definitions.iter().filter(|d| is_selected(&d.name, config)).cloned().collect();
        for definition in &selected {
            let (sender, definition, log_dir) = (sender.clone(), definition.clone(), log_dir.to_path_buf());
            std::thread::spawn(move || {
                let result = McpClient::connect(&definition, startup, call, &log_dir).map_err(|e| format!("{e:#}"));
                let _ = sender.send((definition.name.clone(), result));
            });
        }
        drop(sender);
        collect_clients(receiver, &selected, startup)
    }

    /// Every tool from every connected server.
    pub fn tools(&self) -> Vec<McpTool> {
        self.clients.values().flat_map(|c| c.tools.iter().cloned()).collect()
    }

    /// The servers that connected, with their protocol versions and tool counts.
    pub fn connected(&self) -> Vec<(String, String, usize)> {
        self.clients.values().map(|c| (c.name.clone(), c.protocol_version.clone(), c.tools.len())).collect()
    }

    /// Server instructions to add to the system prompt, by server name.
    pub fn instructions(&self) -> Vec<(String, String)> {
        self.clients.values().filter(|c| !c.instructions.trim().is_empty()).map(|c| (c.name.clone(), c.instructions.clone())).collect()
    }

    /// Calls a tool on a server.
    /// @param server - the server name
    /// @param tool - the tool name
    /// @param arguments - the arguments object
    pub fn call(&mut self, server: &str, tool: &str, arguments: Value) -> Observation {
        match self.clients.get_mut(server) {
            Some(client) => client.call_tool(tool, arguments, &self.images),
            None => Observation::failed("", format!("MCP server '{server}' is not connected")),
        }
    }

    /// Sets the limits used for images that tools return.
    /// @param settings - the image settings
    pub fn set_image_settings(&mut self, settings: ImagesConfig) {
        self.images = settings;
    }

    /// Shuts every server down.
    pub fn close(&mut self) {
        for client in self.clients.values_mut() {
            client.close();
        }
    }
}

/// Applies the `only` and `disabled` lists.
/// @param name - the server name
/// @param config - the MCP settings
fn is_selected(name: &str, config: &McpConfig) -> bool {
    if config.disabled.iter().any(|d| d == name) {
        return false;
    }
    config.only.is_empty() || config.only.iter().any(|o| o == name)
}

/// Gathers the results of the connection threads until all have answered or the time is up.
/// @param receiver - results from the connection threads
/// @param selected - the servers that were started
/// @param startup - the startup timeout
fn collect_clients(receiver: mpsc::Receiver<(String, Result<McpClient, String>)>, selected: &[McpServerDefinition], startup: Duration) -> McpManager {
    let mut manager = McpManager::default();
    let deadline = Instant::now() + startup + Duration::from_secs(2);
    let mut answered = Vec::new();
    while answered.len() < selected.len() {
        let Ok((name, result)) = receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) else { break };
        answered.push(name.clone());
        match result {
            Ok(client) => {
                manager.clients.insert(name, client);
            }
            Err(error) => manager.failures.push(McpFailure { name, error }),
        }
    }
    for definition in selected.iter().filter(|d| !answered.contains(&d.name)) {
        manager.failures.push(McpFailure { name: definition.name.clone(), error: format!("did not finish starting within {} seconds", startup.as_secs()) });
    }
    manager
}
