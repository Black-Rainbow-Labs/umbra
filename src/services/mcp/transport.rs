//! The JSON-RPC plumbing shared by every MCP transport.

use std::time::Duration;

use anyhow::Result;
use serde_json::{json, Value};

/// The MCP revision Umbra asks for in the `initialize` handshake.
pub const LEGACY_PROTOCOL_VERSION: &str = "2025-11-25";
/// The revision that has no handshake and sends its metadata in every request.
pub const MODERN_PROTOCOL_VERSION: &str = "2026-07-28";

/// A JSON-RPC error returned by a server. Kept apart from transport failures so the client can
/// tell "the server answered no" from "the server did not answer".
#[derive(Debug, Clone)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl std::fmt::Display for RpcError {
    /// Formats the error as `code: message`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MCP error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for RpcError {}

/// One way of exchanging JSON-RPC messages with a server.
pub trait Transport: Send {
    /// Sends a request and waits for its response. A JSON-RPC error comes back as an `RpcError`
    /// inside the `anyhow::Error`, so callers can downcast to it.
    /// @param method - the JSON-RPC method
    /// @param params - the params object
    /// @param timeout - how long to wait for the response
    fn request(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value>;

    /// Sends a notification, which has no response.
    /// @param method - the JSON-RPC method
    /// @param params - the params object
    fn notify(&mut self, method: &str, params: Value) -> Result<()>;

    /// Tells the transport which protocol revision is in use, for HTTP headers.
    /// @param version - the negotiated revision
    fn set_protocol_version(&mut self, version: &str);

    /// Shuts the connection down.
    fn close(&mut self);
}

/// Builds a JSON-RPC request object.
/// @param id - the request id
/// @param method - the method name
/// @param params - the params object
pub fn request_message(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

/// Builds a JSON-RPC notification object.
/// @param method - the method name
/// @param params - the params object
pub fn notification_message(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

/// Turns a JSON-RPC response into its result, or an `RpcError`.
/// @param response - the response object
pub fn response_result(response: &Value) -> Result<Value> {
    if let Some(error) = response.get("error") {
        let rpc = RpcError { code: error["code"].as_i64().unwrap_or(0), message: error["message"].as_str().unwrap_or("").to_string() };
        return Err(rpc.into());
    }
    Ok(response.get("result").cloned().unwrap_or(Value::Null))
}

/// Builds the reply to a request the server sent to Umbra. Umbra declares no client
/// capabilities, so it answers `ping` and refuses everything else with method not found.
/// @param request - the server's request
pub fn reply_to_server_request(request: &Value) -> Value {
    let id = request["id"].clone();
    if request["method"] == "ping" {
        return json!({ "jsonrpc": "2.0", "id": id, "result": {} });
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "Method not found" } })
}

/// Returns true for a message that is a request from the server (it has a method and an id).
/// @param message - a parsed JSON-RPC message
pub fn is_server_request(message: &Value) -> bool {
    message.get("method").is_some() && message.get("id").is_some_and(|id| !id.is_null())
}

/// Parses server sent events and returns the data of each event, in order.
/// @param text - the event stream text
pub fn sse_data_events(text: &str) -> Vec<(String, String)> {
    let mut events = Vec::new();
    let mut event = String::new();
    let mut data: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            if !data.is_empty() {
                events.push((std::mem::take(&mut event), data.join("\n")));
                data.clear();
            }
            event.clear();
        } else if let Some(rest) = line.strip_prefix("data:") {
            data.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
        } else if let Some(rest) = line.strip_prefix("event:") {
            event = rest.trim().to_string();
        }
    }
    if !data.is_empty() {
        events.push((event, data.join("\n")));
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multi_line_sse_events() {
        let events = sse_data_events("event: message\ndata: {\"a\":\ndata: 1}\n\ndata: second\n");
        assert_eq!(events, vec![("message".to_string(), "{\"a\":\n1}".to_string()), (String::new(), "second".to_string())]);
    }

    #[test]
    fn refuses_unknown_server_requests_and_answers_ping() {
        assert_eq!(reply_to_server_request(&json!({"id": 3, "method": "ping"}))["result"], json!({}));
        assert_eq!(reply_to_server_request(&json!({"id": 4, "method": "sampling/createMessage"}))["error"]["code"], -32601);
    }
}
