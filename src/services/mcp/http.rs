//! The two HTTP transports: Streamable HTTP (one endpoint, every message is a POST) and the
//! older HTTP+SSE transport (a GET event stream that names the URL to POST to).

use std::io::{BufRead, BufReader};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde_json::Value;

use super::transport::{is_server_request, notification_message, reply_to_server_request, request_message, response_result, sse_data_events, Transport, MODERN_PROTOCOL_VERSION};
use crate::utils::http::{build_agent, http_agent, read_reply, HttpReply};

/// A Streamable HTTP connection.
pub struct StreamableHttpTransport {
    url: String,
    headers: Vec<(String, String)>,
    session_id: Option<String>,
    protocol_version: Option<String>,
    agent: ureq::Agent,
    next_id: u64,
}

impl StreamableHttpTransport {
    /// Prepares a connection. Nothing is sent until the first request.
    /// @param url - the MCP endpoint
    /// @param headers - headers from the config, such as Authorization
    pub fn new(url: &str, headers: &[(String, String)]) -> Self {
        StreamableHttpTransport { url: url.to_string(), headers: headers.to_vec(), session_id: None, protocol_version: None, agent: http_agent(600), next_id: 1 }
    }

    /// POSTs one message with the headers the spec requires.
    /// @param message - the JSON-RPC message
    fn post(&self, message: &Value) -> Result<HttpReply> {
        let mut request = self.agent.post(&self.url).header("content-type", "application/json").header("accept", "application/json, text/event-stream");
        for (key, value) in &self.headers {
            request = request.header(key.as_str(), value.as_str());
        }
        if let Some(session) = &self.session_id {
            request = request.header("mcp-session-id", session.as_str());
        }
        if let Some(version) = &self.protocol_version {
            request = request.header("mcp-protocol-version", version.as_str());
            if version == MODERN_PROTOCOL_VERSION {
                request = add_modern_headers(request, message);
            }
        }
        let response = request.send(message.to_string()).map_err(|e| anyhow!("MCP HTTP request failed: {e}"))?;
        read_reply(response).map_err(|e| anyhow!(e))
    }

    /// Finds the response to request `id` in a reply that is JSON or an event stream, answering
    /// any requests the server made along the way.
    /// @param reply - the HTTP reply
    /// @param id - the request id
    fn find_response(&self, reply: &HttpReply, id: u64) -> Result<Value> {
        let is_stream = reply.header("content-type").unwrap_or("").contains("text/event-stream");
        let messages: Vec<Value> = if is_stream {
            sse_data_events(&reply.body).into_iter().filter_map(|(_, data)| serde_json::from_str(&data).ok()).collect()
        } else {
            vec![serde_json::from_str(&reply.body).map_err(|e| anyhow!("MCP server sent invalid JSON: {e}"))?]
        };
        for message in messages {
            if is_server_request(&message) {
                let _ = self.post(&reply_to_server_request(&message));
            } else if message["id"].as_u64() == Some(id) {
                return response_result(&message);
            }
        }
        bail!("MCP server reply had no response for request {id}")
    }
}

/// Adds the per request headers of the 2026-07-28 revision: `Mcp-Method` always, and `Mcp-Name`
/// for calls that name a tool, resource or prompt.
/// @param request - the request being built
/// @param message - the JSON-RPC message
fn add_modern_headers(mut request: ureq::RequestBuilder<ureq::typestate::WithBody>, message: &Value) -> ureq::RequestBuilder<ureq::typestate::WithBody> {
    if let Some(method) = message["method"].as_str() {
        request = request.header("mcp-method", method);
        let name = message["params"]["name"].as_str().or_else(|| message["params"]["uri"].as_str());
        if let Some(name) = name.filter(|n| n.is_ascii()) {
            request = request.header("mcp-name", name);
        }
    }
    request
}

impl Transport for StreamableHttpTransport {
    /// Sends a request and reads its response. Remembers the session id the server hands out.
    /// @param method - the method name
    /// @param params - the params object
    /// @param _timeout - the agent's own timeout applies
    fn request(&mut self, method: &str, params: Value, _timeout: Duration) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let reply = self.post(&request_message(id, method, params))?;
        if let Some(session) = reply.header("mcp-session-id") {
            self.session_id = Some(session.to_string());
        }
        if !(200..300).contains(&reply.status) {
            let detail = serde_json::from_str::<Value>(&reply.body).ok().filter(|v| v.get("error").is_some());
            if let Some(body) = detail {
                return response_result(&body);
            }
            bail!("MCP server answered HTTP {} to {method}", reply.status);
        }
        self.find_response(&reply, id)
    }

    /// Sends a notification. The server answers 202 with no body.
    /// @param method - the method name
    /// @param params - the params object
    fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        self.post(&notification_message(method, params)).map(|_| ())
    }

    /// Records the revision so later requests carry the version header.
    /// @param version - the negotiated revision
    fn set_protocol_version(&mut self, version: &str) {
        self.protocol_version = Some(version.to_string());
    }

    /// Ends the session with a DELETE when the server gave one.
    fn close(&mut self) {
        if let Some(session) = &self.session_id {
            let _ = self.agent.delete(&self.url).header("mcp-session-id", session.as_str()).call();
        }
    }
}

/// A connection over the older HTTP+SSE transport.
pub struct SseTransport {
    post_url: String,
    headers: Vec<(String, String)>,
    messages: Receiver<Value>,
    agent: ureq::Agent,
    next_id: u64,
}

impl SseTransport {
    /// Opens the event stream and waits for the `endpoint` event that names the POST URL.
    /// @param url - the SSE URL
    /// @param headers - headers from the config
    /// @param timeout - how long to wait for the endpoint event
    pub fn connect(url: &str, headers: &[(String, String)], timeout: Duration) -> Result<Self> {
        let stream_agent = build_agent(None);
        let mut request = stream_agent.get(url).header("accept", "text/event-stream");
        for (key, value) in headers {
            request = request.header(key.as_str(), value.as_str());
        }
        let response = request.call().map_err(|e| anyhow!("could not open SSE stream {url}: {e}"))?;
        if response.status().as_u16() != 200 {
            bail!("SSE stream {url} answered HTTP {}", response.status().as_u16());
        }
        let (endpoint_tx, endpoint_rx) = mpsc::channel();
        let (message_tx, messages) = mpsc::channel();
        spawn_sse_reader(response.into_body(), endpoint_tx, message_tx);
        let endpoint: String = endpoint_rx.recv_timeout(timeout).map_err(|_| anyhow!("SSE stream {url} sent no endpoint event"))?;
        let post_url = resolve_url(url, &endpoint);
        Ok(SseTransport { post_url, headers: headers.to_vec(), messages, agent: http_agent(600), next_id: 1 })
    }

    /// POSTs one message to the endpoint the stream named.
    /// @param message - the JSON-RPC message
    fn post(&self, message: &Value) -> Result<()> {
        let mut request = self.agent.post(&self.post_url).header("content-type", "application/json");
        for (key, value) in &self.headers {
            request = request.header(key.as_str(), value.as_str());
        }
        request.send(message.to_string()).map_err(|e| anyhow!("SSE POST failed: {e}"))?;
        Ok(())
    }
}

impl Transport for SseTransport {
    /// Sends a request and waits for its response on the event stream.
    /// @param method - the method name
    /// @param params - the params object
    /// @param timeout - how long to wait
    fn request(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.post(&request_message(id, method, params))?;
        let deadline = Instant::now() + timeout;
        loop {
            match self.messages.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(message) if is_server_request(&message) => self.post(&reply_to_server_request(&message))?,
                Ok(message) if message["id"].as_u64() == Some(id) => return response_result(&message),
                Ok(_) => continue,
                Err(RecvTimeoutError::Timeout) => bail!("MCP server did not answer {method} within {} seconds", timeout.as_secs()),
                Err(RecvTimeoutError::Disconnected) => bail!("SSE stream closed while Umbra waited for {method}"),
            }
        }
    }

    /// Sends a notification.
    /// @param method - the method name
    /// @param params - the params object
    fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        self.post(&notification_message(method, params))
    }

    /// The old transport has no version header.
    fn set_protocol_version(&mut self, _version: &str) {}

    /// The stream closes when the reader thread's receiver is dropped.
    fn close(&mut self) {}
}

/// Reads the event stream on its own thread: the first `endpoint` event goes to one channel and
/// every JSON message goes to the other.
/// @param body - the GET response body
/// @param endpoint_tx - receives the POST URL
/// @param message_tx - receives JSON-RPC messages
fn spawn_sse_reader(body: ureq::Body, endpoint_tx: mpsc::Sender<String>, message_tx: mpsc::Sender<Value>) {
    std::thread::spawn(move || {
        let reader = BufReader::new(body.into_reader());
        let (mut event, mut data) = (String::new(), String::new());
        for line in reader.lines().map_while(|l| l.ok()) {
            let line = line.trim_end_matches('\r').to_string();
            if let Some(rest) = line.strip_prefix("event:") {
                event = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("data:") {
                data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
            } else if line.is_empty() && !data.is_empty() {
                if event == "endpoint" {
                    let _ = endpoint_tx.send(std::mem::take(&mut data));
                } else if let Ok(message) = serde_json::from_str::<Value>(&data) {
                    if message_tx.send(message).is_err() {
                        break;
                    }
                }
                data.clear();
                event.clear();
            }
        }
    });
}

/// Resolves the endpoint event's URL, which may be relative to the SSE URL.
/// @param base - the SSE URL
/// @param endpoint - the URL or path from the endpoint event
fn resolve_url(base: &str, endpoint: &str) -> String {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return endpoint.to_string();
    }
    let origin_end = base.find("://").map(|i| i + 3).and_then(|start| base[start..].find('/').map(|p| start + p)).unwrap_or(base.len());
    let origin = &base[..origin_end];
    if endpoint.starts_with('/') { format!("{origin}{endpoint}") } else { format!("{}/{endpoint}", base.trim_end_matches('/')) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_endpoint_urls() {
        assert_eq!(resolve_url("http://localhost:9000/sse", "/messages?s=1"), "http://localhost:9000/messages?s=1");
        assert_eq!(resolve_url("http://h/sse", "https://other/x"), "https://other/x");
    }
}
