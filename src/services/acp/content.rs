//! Converting between ACP's shapes and Umbra's: the content blocks of a prompt, the MCP servers an
//! editor passes, a tool call for each action, and a saved history sent back as session updates.

use std::path::{Path, PathBuf};

use base64::Engine;
use serde_json::{json, Value};

use crate::models::action::{Action, Observation};
use crate::models::config::ImagesConfig;
use crate::models::context::{McpServerDefinition, McpTransport, Scope};
use crate::models::image::ImageData;
use crate::models::message::{Message, Role};
use crate::services::environment::submission_from;
use crate::services::image_service::image_from_bytes;
use crate::utils::console::clip;

/// The most characters of one action's output sent to the editor.
pub const LARGEST_OUTPUT: usize = 20_000;

/// The text of a prompt and the images in it.
pub struct PromptInput {
    pub text: String,
    pub images: Vec<ImageData>,
    /// Images that could not be used, and why.
    pub notices: Vec<String>,
}

/// Turns a prompt's content blocks into a task. Text blocks are joined. An embedded file is added
/// as a fenced block under its URI, a linked file as its path, and an image is decoded and scaled
/// the way `-i` images are.
/// @param blocks - the `prompt` array of `session/prompt`
/// @param settings - the image settings
pub fn prompt_input(blocks: &[Value], settings: &ImagesConfig) -> PromptInput {
    let mut parts: Vec<String> = Vec::new();
    let mut images = Vec::new();
    let mut notices = Vec::new();
    for block in blocks {
        match block["type"].as_str().unwrap_or("") {
            "text" => parts.push(block["text"].as_str().unwrap_or("").to_string()),
            "resource_link" => parts.push(format!("File: {}", path_of_uri(block["uri"].as_str().unwrap_or("")))),
            "resource" => parts.push(embedded_file(&block["resource"])),
            "image" => match image_block(block, settings) {
                Ok(image) => images.push(image),
                Err(error) => notices.push(format!("an image in the prompt was not used: {error:#}")),
            },
            other => notices.push(format!("a {other} block in the prompt was not used")),
        }
    }
    PromptInput { text: parts.join("\n\n").trim().to_string(), images, notices }
}

/// An embedded file as text for the task: its path, then its text in a fence. A binary file is
/// named and left out.
/// @param resource - the `resource` member of a `resource` block
fn embedded_file(resource: &Value) -> String {
    let path = path_of_uri(resource["uri"].as_str().unwrap_or(""));
    match resource["text"].as_str() {
        Some(text) => format!("File: {path}\n```\n{text}\n```"),
        None => format!("File: {path} (binary, not included)"),
    }
}

/// Decodes an image block.
/// @param block - the `image` block
/// @param settings - the image settings
fn image_block(block: &Value, settings: &ImagesConfig) -> anyhow::Result<ImageData> {
    let data = block["data"].as_str().unwrap_or("");
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim())?;
    let source = block["uri"].as_str().map(path_of_uri).unwrap_or_else(|| "image from the editor".into());
    image_from_bytes(bytes, &source, settings)
}

/// The path in a `file://` URI, or the URI itself when it is not one.
/// @param uri - the URI
pub fn path_of_uri(uri: &str) -> String {
    let Some(rest) = uri.strip_prefix("file://") else { return uri.to_string() };
    let decoded = percent_decode(rest);
    // file:///C:/x is a Windows path; drop the slash before the drive letter.
    let bytes = decoded.as_bytes();
    if bytes.len() > 2 && bytes[0] == b'/' && bytes[2] == b':' {
        return decoded[1..].to_string();
    }
    decoded
}

/// Decodes `%20` and the like.
/// @param text - the percent encoded text
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(&text[index + 1..index + 3], 16) {
                out.push(value);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The MCP servers an editor passes in `session/new`, as Umbra's definitions. An entry that cannot
/// be read is left out with the reason.
/// @param servers - the `mcpServers` array
pub fn client_mcp_servers(servers: &[Value]) -> (Vec<McpServerDefinition>, Vec<String>) {
    let mut found = Vec::new();
    let mut problems = Vec::new();
    for server in servers {
        let name = server["name"].as_str().unwrap_or("").to_string();
        let transport = match server["type"].as_str().unwrap_or("stdio") {
            "http" => McpTransport::Http { url: server["url"].as_str().unwrap_or("").into(), headers: pairs(&server["headers"]) },
            "sse" => McpTransport::Sse { url: server["url"].as_str().unwrap_or("").into(), headers: pairs(&server["headers"]) },
            _ => McpTransport::Stdio { command: server["command"].as_str().unwrap_or("").into(), args: strings(&server["args"]), env: pairs(&server["env"]), cwd: None },
        };
        let empty = match &transport { McpTransport::Stdio { command, .. } => command.is_empty(), McpTransport::Http { url, .. } | McpTransport::Sse { url, .. } => url.is_empty() };
        if name.is_empty() || empty {
            problems.push(format!("an MCP server from the editor has no name, command or url: {server}"));
            continue;
        }
        found.push(McpServerDefinition { name, transport, origin: PathBuf::from("the editor"), source: "acp".into(), scope: Scope::Project });
    }
    (found, problems)
}

/// Adds the editor's MCP servers to the discovered ones. The editor's server wins over a
/// discovered one of the same name.
/// @param discovered - the servers found in config files
/// @param from_client - the servers the editor passed
pub fn merge_mcp_servers(discovered: Vec<McpServerDefinition>, from_client: Vec<McpServerDefinition>) -> Vec<McpServerDefinition> {
    let mut merged: Vec<McpServerDefinition> = discovered.into_iter().filter(|d| !from_client.iter().any(|c| c.name == d.name)).collect();
    merged.extend(from_client);
    merged
}

/// Reads ACP's `[{name, value}]` list.
/// @param value - the list
fn pairs(value: &Value) -> Vec<(String, String)> {
    value.as_array().into_iter().flatten().filter_map(|p| Some((p["name"].as_str()?.to_string(), p["value"].as_str().unwrap_or("").to_string()))).collect()
}

/// Reads a list of strings.
/// @param value - the list
fn strings(value: &Value) -> Vec<String> {
    value.as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect()
}

/// A text content block.
/// @param text - the text
pub fn text_block(text: &str) -> Value {
    json!({ "type": "text", "text": text })
}

/// The `tool_call` update for an action, before it runs.
/// @param id - the tool call id
/// @param action - the action
/// @param cwd - the working folder, for the location
pub fn tool_call(id: &str, action: &Action, cwd: &Path) -> Value {
    let (title, kind, raw_input) = match action {
        Action::Bash { command, .. } => (clip(first_line(command), 200), "execute", json!({ "command": command })),
        Action::Mcp { server, tool, arguments, .. } => (format!("{server}.{tool}"), "other", arguments.clone()),
    };
    let mut call = json!({ "sessionUpdate": "tool_call", "toolCallId": id, "title": title, "kind": kind, "status": "pending", "rawInput": raw_input });
    if let Action::Bash { command, .. } = action {
        call["content"] = json!([{ "type": "content", "content": text_block(&format!("```bash\n{command}\n```")) }]);
        call["locations"] = json!([{ "path": cwd.to_string_lossy() }]);
    }
    call
}

/// The `tool_call_update` for an action that finished: completed when it exited 0, failed
/// otherwise, with its output.
/// @param id - the tool call id
/// @param observation - what the action returned
pub fn tool_result(id: &str, observation: &Observation) -> Value {
    let status = if observation.returncode == 0 && observation.exception_info.is_empty() { "completed" } else { "failed" };
    let mut text = observation.output.trim_end().to_string();
    if !observation.exception_info.is_empty() {
        text = if text.is_empty() { observation.exception_info.clone() } else { format!("{text}\n{}", observation.exception_info) };
    }
    let shown = if text.is_empty() { format!("(no output, exit {})", observation.returncode) } else { format!("```\n{}\n```", clip(&text, LARGEST_OUTPUT)) };
    json!({ "sessionUpdate": "tool_call_update", "toolCallId": id, "status": status, "content": [{ "type": "content", "content": text_block(&shown) }], "rawOutput": { "output": clip(&observation.output, LARGEST_OUTPUT), "returncode": observation.returncode, "exception_info": observation.exception_info } })
}

/// The `tool_call_update` for an action that did not run.
/// @param id - the tool call id
/// @param reason - why it did not run
pub fn tool_not_run(id: &str, reason: &str) -> Value {
    json!({ "sessionUpdate": "tool_call_update", "toolCallId": id, "status": "failed", "content": [{ "type": "content", "content": text_block(reason) }] })
}

/// The first line of a command, for a tool call's title.
/// @param command - the command text
fn first_line(command: &str) -> &str {
    command.trim().lines().next().unwrap_or("")
}

/// The tool call id for an action: the model's own id, or one made from the step and position.
/// @param action - the action
/// @param step - the model call number
/// @param index - its position in the reply
pub fn tool_call_id(action: &Action, step: u64, index: usize) -> String {
    action.tool_call_id().map(str::to_string).unwrap_or_else(|| format!("umbra_{step}_{index}"))
}

/// A saved history as the session updates `session/load` sends: the person's words, the model's
/// text, each action with its result, and each final answer. The final answer is read from the
/// command that submitted it, because a turn's exit marker is removed when the next task is added.
/// @param messages - the history
/// @param cwd - the working folder
pub fn history_updates(messages: &[Message], cwd: &Path) -> Vec<Value> {
    let mut updates = Vec::new();
    let mut open_calls: Vec<String> = Vec::new();
    for (number, message) in messages.iter().enumerate() {
        match message.role {
            Role::User | Role::Tool if message.extra.get("raw_output").is_some() => {
                let id = message.tool_call_id.clone().or_else(|| open_calls.first().cloned());
                let observation = observation_of(message);
                if let Some(id) = id {
                    open_calls.retain(|c| *c != id);
                    updates.push(tool_result(&id, &observation));
                }
                if let Some(answer) = submission_from(&observation).filter(|a| !a.trim().is_empty()) {
                    updates.push(json!({ "sessionUpdate": "agent_message_chunk", "content": text_block(answer.trim()) }));
                }
            }
            Role::User => {
                if let Some(text) = message.extra["user_text"].as_str() {
                    updates.push(json!({ "sessionUpdate": "user_message_chunk", "content": text_block(text) }));
                }
            }
            Role::Assistant => {
                if !message.content.trim().is_empty() {
                    updates.push(json!({ "sessionUpdate": "agent_message_chunk", "content": text_block(message.content.trim()) }));
                }
                let actions: Vec<Action> = serde_json::from_value(message.extra["actions"].clone()).unwrap_or_default();
                // Saved actions without a model id get ids no live turn uses.
                open_calls = actions.iter().enumerate().map(|(i, a)| a.tool_call_id().map(str::to_string).unwrap_or_else(|| format!("umbra_saved_{number}_{i}"))).collect();
                for (action, id) in actions.iter().zip(&open_calls) {
                    updates.push(tool_call(id, action, cwd));
                }
            }
            _ => {}
        }
    }
    updates
}

/// The observation saved in an observation message's `extra`.
/// @param message - a tool result or text mode observation message
fn observation_of(message: &Message) -> Observation {
    let extra = &message.extra;
    Observation { output: extra["raw_output"].as_str().unwrap_or("").to_string(), returncode: extra["returncode"].as_i64().unwrap_or(0) as i32, exception_info: extra["exception_info"].as_str().unwrap_or("").to_string(), images: Vec::new() }
}

/// The ACP stop reason for how a turn ended.
/// @param status - Umbra's exit status
pub fn stop_reason(status: &str) -> &'static str {
    match status {
        "LimitsExceeded" | "TimeExceeded" => "max_turn_requests",
        "Interrupted" | "UserQuit" => "cancelled",
        _ => "end_turn",
    }
}

/// Seconds since the Unix epoch as an ISO 8601 time in UTC, such as `2026-11-09T14:22:15Z`.
/// @param seconds - seconds since the Unix epoch
pub fn iso_time(seconds: f64) -> String {
    let total = seconds.max(0.0) as i64;
    let (days, rest) = (total.div_euclid(86_400), total.rem_euclid(86_400));
    // Howard Hinnant's days-to-civil algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_text_files_and_links_from_a_prompt() {
        let blocks = vec![json!({ "type": "text", "text": "fix it" }), json!({ "type": "resource", "resource": { "uri": "file:///C:/src/a%20b.rs", "text": "fn main() {}" } }), json!({ "type": "resource_link", "uri": "file:///home/x/notes.md", "name": "notes.md" })];
        let input = prompt_input(&blocks, &ImagesConfig::default());
        assert_eq!(input.text, "fix it\n\nFile: C:/src/a b.rs\n```\nfn main() {}\n```\n\nFile: /home/x/notes.md");
        assert!(input.images.is_empty() && input.notices.is_empty());
    }

    #[test]
    fn writes_iso_times() {
        assert_eq!(iso_time(0.0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_time(1_794_185_509.0), "2026-11-09T00:51:49Z");
    }

    #[test]
    fn the_editors_mcp_server_replaces_a_discovered_one_of_the_same_name() {
        let (client, problems) = client_mcp_servers(&[json!({ "name": "fs", "command": "node", "args": ["a.js"], "env": [{ "name": "K", "value": "v" }] }), json!({ "name": "web", "type": "http", "url": "http://x/mcp", "headers": [] }), json!({ "name": "" })]);
        assert_eq!(problems.len(), 1);
        assert_eq!(client[0].transport, McpTransport::Stdio { command: "node".into(), args: vec!["a.js".into()], env: vec![("K".into(), "v".into())], cwd: None });
        let discovered = client_mcp_servers(&[json!({ "name": "fs", "command": "old" }), json!({ "name": "other", "command": "x" })]).0;
        let merged = merge_mcp_servers(discovered, client);
        let names: Vec<(&str, bool)> = merged.iter().map(|d| (d.name.as_str(), d.source == "acp")).collect();
        assert_eq!(names, vec![("other", true), ("fs", true), ("web", true)]);
    }
}
