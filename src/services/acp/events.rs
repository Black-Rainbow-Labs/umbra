//! The agent events of one ACP session, sent to the editor as `session/update` notifications,
//! and the permission requests confirm mode sends before a command runs.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use regex::Regex;
use serde_json::{json, Value};

use super::content::{text_block, tool_call, tool_call_id, tool_not_run, tool_result};
use super::wire::{wait_for_answer, Waited, Wire};
use crate::models::action::{Action, Observation};
use crate::models::config::Mode;
use crate::services::agent_events::AgentEvents;
use crate::services::interaction_service::Decision;

/// What the editor is told when it refuses a command.
pub const REJECTED: &str = "Commands not executed. The user rejected your commands.";

/// The state of a session that the reader thread and the session thread both use.
pub struct SessionShared {
    pub id: String,
    /// confirm or yolo. The reader thread changes it on `session/set_mode`, also during a turn.
    pub mode: Mutex<Mode>,
    /// Set by `session/cancel`; the session thread's interrupt flag.
    pub cancel: Arc<AtomicBool>,
    /// Programs and MCP tools the person allowed for the rest of the session.
    pub always: Mutex<HashSet<String>>,
    /// The session's `provider/model`.
    pub model: Mutex<String>,
    /// The other models the config offers in the editor's picker.
    pub models: Mutex<Vec<String>>,
}

impl SessionShared {
    /// Builds the shared state of a new session.
    /// @param id - the session id
    /// @param mode - the mode it starts in
    pub fn new(id: &str, mode: Mode) -> Self {
        SessionShared { id: id.to_string(), mode: Mutex::new(mode), cancel: Arc::new(AtomicBool::new(false)), always: Mutex::new(HashSet::new()), model: Mutex::new(String::new()), models: Mutex::new(Vec::new()) }
    }

    /// The current mode.
    pub fn mode(&self) -> Mode {
        *self.mode.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// What the editor said it can show.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClientShows {
    /// `session.notices`: the `notice` update.
    pub notices: bool,
}

/// The events hook of one session.
pub struct AcpEvents {
    wire: Arc<Wire>,
    shared: Arc<SessionShared>,
    shows: ClientShows,
    whitelist: Vec<Regex>,
    cwd: PathBuf,
    /// The tool call ids of the current reply's actions, in order.
    calls: Vec<String>,
    /// Model replies so far, for tool call ids of actions that have none.
    replies: u64,
}

impl AcpEvents {
    /// Builds the hook.
    /// @param wire - stdout and the pending requests
    /// @param shared - the session's shared state
    /// @param shows - what the editor can show
    /// @param whitelist - regexes for commands that run without asking
    /// @param cwd - the session's working folder
    pub fn new(wire: Arc<Wire>, shared: Arc<SessionShared>, shows: ClientShows, whitelist: &[String], cwd: PathBuf) -> Self {
        let whitelist = whitelist.iter().filter_map(|p| Regex::new(&format!("^(?:{p})")).ok()).collect();
        AcpEvents { wire, shared, shows, whitelist, cwd, calls: Vec::new(), replies: 0 }
    }

    /// Sends one session update.
    /// @param update - the update object
    fn send(&self, update: Value) {
        self.wire.update(&self.shared.id, update);
    }

    /// The tool call id of the current reply's action at `index`.
    /// @param index - the action's position
    fn call_id(&self, index: usize) -> String {
        self.calls.get(index).cloned().unwrap_or_else(|| format!("umbra_{}_{index}", self.replies))
    }

    /// Returns true when an action may run without asking: it is on the whitelist, or the person
    /// allowed its program or tool for the session.
    /// @param action - the action
    fn allowed_without_asking(&self, action: &Action) -> bool {
        if let Action::Bash { command, .. } = action {
            if self.whitelist.iter().any(|r| r.is_match(command)) {
                return true;
            }
        }
        self.shared.always.lock().unwrap_or_else(|e| e.into_inner()).contains(&always_key(action))
    }

    /// Asks the editor whether one action may run.
    /// @param index - its position in the reply
    /// @param action - the action
    fn ask(&self, index: usize, action: &Action) -> Permission {
        let mut call = tool_call(&self.call_id(index), action, &self.cwd);
        call.as_object_mut().map(|m| m.remove("sessionUpdate"));
        let options = json!([
            { "optionId": "allow", "name": "Allow", "kind": "allow_once" },
            { "optionId": "allow-always", "name": format!("Always allow {}", always_label(action)), "kind": "allow_always" },
            { "optionId": "reject", "name": "Reject", "kind": "reject_once" },
        ]);
        let receiver = self.wire.request("session/request_permission", json!({ "sessionId": self.shared.id, "toolCall": call, "options": options }));
        match wait_for_answer(&receiver) {
            Waited::Answered(Ok(result)) => read_permission(&result),
            Waited::Answered(Err(error)) => {
                eprintln!("umbra: the editor answered the permission request with an error: {}", error.message);
                Permission::Reject
            }
            Waited::Cancelled => Permission::Cancelled,
        }
    }
}

/// The person's answer to a permission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Allow,
    AllowAlways,
    Reject,
    Cancelled,
}

/// Reads a `session/request_permission` result.
/// @param result - the editor's result
pub fn read_permission(result: &Value) -> Permission {
    let outcome = &result["outcome"];
    if outcome["outcome"] != "selected" {
        return Permission::Cancelled;
    }
    match outcome["optionId"].as_str().unwrap_or("") {
        "allow" => Permission::Allow,
        "allow-always" => Permission::AllowAlways,
        _ => Permission::Reject,
    }
}

/// What "always allow" remembers for an action: the program a command starts, or the MCP tool.
/// @param action - the action
pub fn always_key(action: &Action) -> String {
    match action {
        Action::Bash { command, .. } => format!("bash:{}", program_of(command)),
        Action::Mcp { server, tool, .. } => format!("mcp:{server}.{tool}"),
    }
}

/// The words for "always allow" in the editor's button.
/// @param action - the action
fn always_label(action: &Action) -> String {
    match action {
        Action::Bash { command, .. } => format!("`{}` commands", program_of(command)),
        Action::Mcp { server, tool, .. } => format!("{server}.{tool}"),
    }
}

/// The program a command starts: its first word, with any `cd somewhere &&` before it skipped.
/// @param command - the command text
fn program_of(command: &str) -> String {
    let mut words = command.split_whitespace().peekable();
    let mut first = words.next().unwrap_or("");
    while first == "cd" {
        let _folder = words.next();
        match words.next() {
            Some("&&") | Some(";") => first = words.next().unwrap_or(""),
            _ => break,
        }
    }
    first.rsplit(['/', '\\']).next().unwrap_or(first).to_string()
}

impl AgentEvents for AcpEvents {
    /// Sends the reply's text and a pending tool call for each action.
    fn reply(&mut self, content: &str, actions: &[Action]) {
        self.replies += 1;
        if !content.trim().is_empty() {
            self.send(json!({ "sessionUpdate": "agent_message_chunk", "content": text_block(content.trim()) }));
        }
        self.calls = actions.iter().enumerate().map(|(i, a)| tool_call_id(a, self.replies, i)).collect();
        for (index, action) in actions.iter().enumerate() {
            self.send(tool_call(&self.call_id(index), action, &self.cwd));
        }
    }

    /// Marks the tool call as running.
    fn action_started(&mut self, index: usize, _action: &Action) {
        self.send(json!({ "sessionUpdate": "tool_call_update", "toolCallId": self.call_id(index), "status": "in_progress" }));
    }

    /// Sends the tool call's result.
    fn action_finished(&mut self, index: usize, _action: &Action, observation: &Observation) {
        self.send(tool_result(&self.call_id(index), observation));
    }

    /// Marks the tool call as failed, with the reason it did not run.
    fn action_not_run(&mut self, index: usize, _action: &Action, reason: &str) {
        self.send(tool_not_run(&self.call_id(index), reason));
    }

    /// Sends a notice, or a thought for an editor that cannot show notices.
    fn notice(&mut self, label: &str, text: &str) {
        if self.shows.notices {
            self.send(json!({ "sessionUpdate": "notice", "severity": "info", "title": label, "description": text }));
        } else {
            self.send(json!({ "sessionUpdate": "agent_thought_chunk", "content": text_block(&format!("{label}: {text}")) }));
        }
    }

    /// Sends the context use and the cost.
    fn usage(&mut self, used: u64, window: u64, cost: f64) {
        self.send(json!({ "sessionUpdate": "usage_update", "used": used, "size": window, "cost": { "amount": cost, "currency": "USD" } }));
    }

    /// In confirm mode, asks the editor about each action that may not run without asking.
    fn confirm(&mut self, actions: &[Action]) -> Option<Decision> {
        if self.shared.mode() != Mode::Confirm {
            return Some(Decision::Run);
        }
        for (index, action) in actions.iter().enumerate() {
            if self.allowed_without_asking(action) {
                continue;
            }
            match self.ask(index, action) {
                Permission::Allow => {}
                Permission::AllowAlways => {
                    self.shared.always.lock().unwrap_or_else(|e| e.into_inner()).insert(always_key(action));
                }
                Permission::Reject => return Some(Decision::Reject(REJECTED.into())),
                Permission::Cancelled => return Some(Decision::Interrupted),
            }
        }
        Some(Decision::Run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_allow_remembers_the_program() {
        let bash = |c: &str| Action::Bash { command: c.into(), tool_call_id: None, timeout: None };
        assert_eq!(always_key(&bash("cargo test --all")), "bash:cargo");
        assert_eq!(always_key(&bash("cd src && /usr/bin/make -j4")), "bash:make");
        assert_eq!(always_key(&bash("cd src")), "bash:cd");
        let mcp = Action::Mcp { server: "fs".into(), tool: "read".into(), arguments: json!({}), tool_call_id: None };
        assert_eq!(always_key(&mcp), "mcp:fs.read");
    }

    #[test]
    fn reads_permission_answers() {
        assert_eq!(read_permission(&json!({ "outcome": { "outcome": "selected", "optionId": "allow-always" } })), Permission::AllowAlways);
        assert_eq!(read_permission(&json!({ "outcome": { "outcome": "selected", "optionId": "reject" } })), Permission::Reject);
        assert_eq!(read_permission(&json!({ "outcome": { "outcome": "cancelled" } })), Permission::Cancelled);
    }
}
