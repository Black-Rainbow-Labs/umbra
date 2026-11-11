//! State the fullscreen view reads at every frame and that other threads change: what the header
//! shows, the session's MCP servers and config, the commands the popup lists, and the requests the
//! view sends to the agent, which the agent applies between steps.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::models::config::{Mode, UmbraConfig};
use crate::services::mcp::McpManager;

/// What the header shows, apart from the MCP state, which is read from the manager.
#[derive(Debug, Clone, Default)]
pub struct HeaderInfo {
    pub version: String,
    /// The `provider/model` string, or the model's name from `[models]`.
    pub model: String,
    pub folder: String,
    /// The working folder itself, for the session picker.
    pub cwd: std::path::PathBuf,
    pub mode: String,
    pub skills: usize,
    /// False when `mcp.enabled = false` or `--no-mcp`.
    pub mcp_enabled: bool,
    /// When the logo's stripe reveal started, or None to show the finished logo.
    pub reveal_from: Option<Instant>,
}

/// A command the popup lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEntry {
    pub name: String,
    /// `command`, `custom` or `skill`.
    pub kind: &'static str,
    pub description: String,
}

/// Something the view asks the agent to do at its next safe point.
#[derive(Debug, Clone)]
pub enum UiRequest {
    /// Use this `provider/model` from the next model call.
    SetModel(String),
    /// Switch the mode.
    SetMode(Mode),
    /// End the session.
    Quit,
    /// The configuration modal saved changes: apply the ones that take effect now.
    ApplyConfig(Box<UmbraConfig>),
}

static HEADER: Mutex<Option<HeaderInfo>> = Mutex::new(None);
static MCP: Mutex<Option<Arc<McpManager>>> = Mutex::new(None);
static CONFIG: Mutex<Option<UmbraConfig>> = Mutex::new(None);
static COMMANDS: Mutex<Vec<CommandEntry>> = Mutex::new(Vec::new());
static REQUESTS: Mutex<Vec<UiRequest>> = Mutex::new(Vec::new());
static SESSION: Mutex<String> = Mutex::new(String::new());
static MCP_ORIGINS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Keeps where each MCP server was defined, for the configuration modal.
/// @param origins - each server's name and the file it came from
pub fn set_mcp_origins(origins: Vec<(String, String)>) {
    *MCP_ORIGINS.lock().unwrap_or_else(|e| e.into_inner()) = origins;
}

/// Where each MCP server was defined.
pub fn mcp_origins() -> Vec<(String, String)> {
    MCP_ORIGINS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Sets what the header shows.
/// @param info - the header's values
pub fn set_header(info: HeaderInfo) {
    *HEADER.lock().unwrap_or_else(|e| e.into_inner()) = Some(info);
}

/// What the header shows, or None before the run set it.
pub fn header() -> Option<HeaderInfo> {
    HEADER.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Changes one field of the header and asks for a redraw. It takes no lock of the view, so the
/// view's own key handling may call it.
/// @param change - the change
pub fn update_header(change: impl FnOnce(&mut HeaderInfo)) {
    if let Some(info) = HEADER.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        change(info);
    }
    crate::utils::screen::request_redraw();
}

/// Sets the session's MCP servers, which the header, the status bar and the panel read.
/// @param manager - the manager, or None
pub fn set_mcp(manager: Option<Arc<McpManager>>) {
    *MCP.lock().unwrap_or_else(|e| e.into_inner()) = manager;
}

/// The session's MCP servers.
pub fn mcp() -> Option<Arc<McpManager>> {
    MCP.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Keeps a copy of the session's config, for the pickers and the config modal.
/// @param config - the merged config
pub fn set_config(config: UmbraConfig) {
    *CONFIG.lock().unwrap_or_else(|e| e.into_inner()) = Some(config);
}

/// The session's config, when the run set it.
pub fn config() -> Option<UmbraConfig> {
    CONFIG.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Sets the commands the popup offers.
/// @param commands - built in commands, custom commands and skills
pub fn set_commands(commands: Vec<CommandEntry>) {
    *COMMANDS.lock().unwrap_or_else(|e| e.into_inner()) = commands;
}

/// The commands the popup offers.
pub fn commands() -> Vec<CommandEntry> {
    COMMANDS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Asks the agent to do something at its next safe point.
/// @param request - what to do
pub fn request(request: UiRequest) {
    REQUESTS.lock().unwrap_or_else(|e| e.into_inner()).push(request);
}

/// Takes every request the view made since the last call.
pub fn take_requests() -> Vec<UiRequest> {
    std::mem::take(&mut *REQUESTS.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Sets the id of the session the view shows.
/// @param id - the session id
pub fn set_session(id: &str) {
    *SESSION.lock().unwrap_or_else(|e| e.into_inner()) = id.to_string();
}

/// The id of the session the view shows.
pub fn session() -> String {
    SESSION.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The spinner frames, one every 100 milliseconds.
const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// The spinner character for a moment, computed from the time so nothing has to step it.
/// @param now - the frame's time
pub fn spinner_at(now: Instant) -> char {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = *START.get_or_init(Instant::now);
    SPINNER[(now.saturating_duration_since(start).as_millis() / 100) as usize % SPINNER.len()]
}
