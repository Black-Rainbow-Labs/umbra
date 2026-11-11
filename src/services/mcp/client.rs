//! One connected MCP server: the handshake, the tool list and tool calls, in both protocol eras.

use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use serde_json::{json, Value};

use super::http::{SseTransport, StreamableHttpTransport};
use super::stdio::StdioTransport;
use super::transport::{RpcError, Transport, LEGACY_PROTOCOL_VERSION, MODERN_PROTOCOL_VERSION};
use crate::models::action::Observation;
use crate::models::config::ImagesConfig;
use crate::models::context::{McpServerDefinition, McpTool, McpTransport};
use crate::models::image::ImageData;
use crate::services::image_service::image_from_mcp;
use crate::utils::interrupt;

/// A server Umbra has connected to.
pub struct McpClient {
    pub name: String,
    pub protocol_version: String,
    /// The `instructions` text the server sent, if any.
    pub instructions: String,
    pub tools: Vec<McpTool>,
    transport: Box<dyn Transport>,
    call_timeout: Duration,
}

impl McpClient {
    /// Connects, negotiates the protocol and lists the tools.
    /// @param definition - the server from the config files
    /// @param startup_timeout - how long the handshake and tool listing may take
    /// @param call_timeout - how long one tool call may take
    /// @param log_dir - where stdio servers' stderr is written
    pub fn connect(definition: &McpServerDefinition, startup_timeout: Duration, call_timeout: Duration, log_dir: &Path) -> Result<Self> {
        let transport = open_transport(definition, startup_timeout, log_dir)?;
        let mut client = McpClient { name: definition.name.clone(), protocol_version: String::new(), instructions: String::new(), tools: Vec::new(), transport, call_timeout };
        client.handshake(startup_timeout)?;
        client.tools = client.list_tools(startup_timeout)?;
        Ok(client)
    }

    /// Runs the `initialize` handshake. When the server refuses it with a JSON-RPC error, it is
    /// treated as a 2026-07-28 server, which has no handshake.
    /// @param timeout - how long to wait
    fn handshake(&mut self, timeout: Duration) -> Result<()> {
        let params = json!({ "protocolVersion": LEGACY_PROTOCOL_VERSION, "capabilities": {}, "clientInfo": client_info() });
        match self.transport.request("initialize", params, timeout) {
            Ok(result) => {
                self.protocol_version = result["protocolVersion"].as_str().unwrap_or(LEGACY_PROTOCOL_VERSION).to_string();
                self.instructions = result["instructions"].as_str().unwrap_or("").to_string();
                self.transport.set_protocol_version(&self.protocol_version);
                self.transport.notify("notifications/initialized", json!({}))
            }
            Err(error) if error.downcast_ref::<RpcError>().is_some() => {
                self.protocol_version = MODERN_PROTOCOL_VERSION.to_string();
                self.transport.set_protocol_version(MODERN_PROTOCOL_VERSION);
                let _ = self.request("server/discover", json!({}), timeout);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Sends a request, adding the `_meta` block the 2026-07-28 revision requires.
    /// @param method - the method name
    /// @param params - the params object
    /// @param timeout - how long to wait
    fn request(&mut self, method: &str, mut params: Value, timeout: Duration) -> Result<Value> {
        if self.protocol_version == MODERN_PROTOCOL_VERSION {
            params["_meta"] = modern_meta();
        }
        self.transport.request(method, params, timeout)
    }

    /// Lists every tool, following `nextCursor` pages.
    /// @param timeout - how long each page may take
    fn list_tools(&mut self, timeout: Duration) -> Result<Vec<McpTool>> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let params = match &cursor { Some(c) => json!({ "cursor": c }), None => json!({}) };
            let result = self.request("tools/list", params, timeout)?;
            for tool in result["tools"].as_array().cloned().unwrap_or_default() {
                tools.push(McpTool { server: self.name.clone(), name: tool["name"].as_str().unwrap_or("").to_string(), description: tool["description"].as_str().unwrap_or("").to_string(), input_schema: normalize_schema(&tool["inputSchema"]) });
            }
            cursor = result["nextCursor"].as_str().map(String::from);
            if cursor.is_none() {
                return Ok(tools);
            }
        }
    }

    /// Calls one tool and turns the result into an observation. A tool that reports `isError`
    /// gets return code 1, so the model sees it failed and can try again. Images in the result
    /// are attached to the observation.
    /// @param tool - the tool name on this server
    /// @param arguments - the arguments object
    /// @param settings - the image scaling and size limits
    pub fn call_tool(&mut self, tool: &str, arguments: Value, settings: &ImagesConfig) -> Observation {
        let arguments = if arguments.is_object() { arguments } else { json!({}) };
        match self.request("tools/call", json!({ "name": tool, "arguments": arguments }), self.call_timeout) {
            Ok(result) => {
                let failed = result["isError"].as_bool().unwrap_or(false);
                let (images, notes) = tool_result_images(&result, &format!("mcp:{}.{tool}", self.name), settings);
                let mut output = render_tool_result(&result);
                for note in notes.iter().chain((!images.is_empty()).then(|| format!("[{} image(s) attached to this result]", images.len())).iter()) {
                    output.push('\n');
                    output.push_str(note);
                }
                Observation { output, returncode: if failed { 1 } else { 0 }, exception_info: String::new(), images, ..Default::default() }
            }
            Err(error) if interrupt::is_pending() => Observation::failed("", format!("{INTERRUPTED_MCP_CALL} ({error})")),
            Err(error) => Observation::failed("", format!("MCP call {}.{tool} failed: {error}", self.name)),
        }
    }

    /// Shuts the server down.
    pub fn close(&mut self) {
        self.transport.close();
    }
}

/// Opens the transport the definition asks for.
/// @param definition - the server definition
/// @param timeout - the connection timeout, for SSE
/// @param log_dir - where stdio servers' stderr goes
fn open_transport(definition: &McpServerDefinition, timeout: Duration, log_dir: &Path) -> Result<Box<dyn Transport>> {
    Ok(match &definition.transport {
        McpTransport::Stdio { command, args, env, cwd } => {
            let log = log_dir.join(format!("{}.log", sanitize(&definition.name)));
            Box::new(StdioTransport::start(command, args, env, cwd.as_deref(), &log)?)
        }
        McpTransport::Http { url, headers } => Box::new(StreamableHttpTransport::new(url, headers)),
        McpTransport::Sse { url, headers } => Box::new(SseTransport::connect(url, headers, timeout)?),
    })
}

/// Umbra's name and version as an MCP client.
fn client_info() -> Value {
    json!({ "name": "umbra", "version": env!("CARGO_PKG_VERSION") })
}

/// The `_meta` block every 2026-07-28 request carries.
fn modern_meta() -> Value {
    json!({ "io.modelcontextprotocol/protocolVersion": MODERN_PROTOCOL_VERSION, "io.modelcontextprotocol/clientInfo": client_info(), "io.modelcontextprotocol/clientCapabilities": {} })
}

/// Makes sure a tool schema is an object schema, which every provider requires.
/// @param schema - the tool's inputSchema
fn normalize_schema(schema: &Value) -> Value {
    if schema.is_object() && schema.get("type").is_some() {
        return schema.clone();
    }
    json!({ "type": "object", "properties": {} })
}

/// The error an MCP call gets when Ctrl+C stops it.
pub const INTERRUPTED_MCP_CALL: &str = "The user interrupted this MCP call (Ctrl+C)";

/// Reads the images out of a tool result. An image that cannot be used gives a note instead.
/// @param result - the tools/call result
/// @param source - the tool, recorded with each image
/// @param settings - the image settings
pub fn tool_result_images(result: &Value, source: &str, settings: &ImagesConfig) -> (Vec<ImageData>, Vec<String>) {
    let mut images = Vec::new();
    let mut notes = Vec::new();
    for item in result["content"].as_array().into_iter().flatten().filter(|i| i["type"] == "image") {
        match image_from_mcp(item, source, settings) {
            Ok(image) => images.push(image),
            Err(error) => notes.push(format!("[an image in this result could not be shown to you: {error:#}]")),
        }
    }
    (images, notes)
}

/// Joins a tool result's content items into text. Images and audio are described, not inlined.
/// @param result - the tools/call result
pub fn render_tool_result(result: &Value) -> String {
    let mut parts = Vec::new();
    for item in result["content"].as_array().cloned().unwrap_or_default() {
        let part = match item["type"].as_str().unwrap_or("") {
            "text" => item["text"].as_str().unwrap_or("").to_string(),
            "image" | "audio" => format!("[{} content, {}, {} base64 characters]", item["type"].as_str().unwrap_or(""), item["mimeType"].as_str().unwrap_or("unknown type"), item["data"].as_str().map(str::len).unwrap_or(0)),
            "resource_link" => format!("[resource link: {} {}]", item["name"].as_str().unwrap_or(""), item["uri"].as_str().unwrap_or("")),
            "resource" => item["resource"]["text"].as_str().map(String::from).unwrap_or_else(|| format!("[resource: {}]", item["resource"]["uri"].as_str().unwrap_or(""))),
            _ => item.to_string(),
        };
        parts.push(part);
    }
    if parts.is_empty() {
        if let Some(structured) = result.get("structuredContent") {
            return structured.to_string();
        }
    }
    parts.join("\n")
}

/// Makes a server name safe to use as a file name.
/// @param name - the server name
pub fn sanitize(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_text_images_and_resources() {
        let result = json!({"content": [{"type": "text", "text": "hello"}, {"type": "image", "mimeType": "image/png", "data": "abcd"}, {"type": "resource", "resource": {"uri": "file://x", "text": "body"}}]});
        assert_eq!(render_tool_result(&result), "hello\n[image content, image/png, 4 base64 characters]\nbody");
    }
}
