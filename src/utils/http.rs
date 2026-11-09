//! A small blocking HTTP client wrapper. Status codes are returned, never turned into errors,
//! so callers decide what to retry.

use std::sync::mpsc;
use std::time::Duration;

use super::interrupt;

/// The outcome of one HTTP request that reached a server.
pub struct HttpReply {
    pub status: u16,
    pub body: String,
    pub headers: Vec<(String, String)>,
}

impl HttpReply {
    /// Returns a response header by name, ignoring letter case.
    /// @param name - the header name
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// Builds an HTTP agent that reports every status code as a normal reply.
/// @param timeout_seconds - the whole request's time limit
pub fn http_agent(timeout_seconds: u64) -> ureq::Agent {
    build_agent(Some(Duration::from_secs(timeout_seconds)))
}

/// Builds an HTTP agent that uses the operating system's TLS library, with an optional overall
/// timeout. A long lived event stream passes None.
/// @param timeout - the whole request's time limit, or None for no limit
pub fn build_agent(timeout: Option<Duration>) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
    let config = ureq::Agent::config_builder().http_status_as_error(false).tls_config(tls).timeout_global(timeout).build();
    ureq::Agent::new_with_config(config)
}

/// Sends a JSON body with POST and reads the whole reply as text.
/// @param agent - the HTTP agent
/// @param url - the endpoint
/// @param headers - request headers
/// @param body - the JSON body
pub fn post_json(agent: &ureq::Agent, url: &str, headers: &[(String, String)], body: &serde_json::Value) -> Result<HttpReply, String> {
    let mut request = agent.post(url).header("content-type", "application/json");
    for (key, value) in headers {
        request = request.header(key.as_str(), value.as_str());
    }
    let response = request.send(body.to_string()).map_err(|e| e.to_string())?;
    read_reply(response)
}

/// Sends a GET request and reads the whole reply as text.
/// @param agent - the HTTP agent
/// @param url - the endpoint
/// @param headers - request headers
pub fn get_text(agent: &ureq::Agent, url: &str, headers: &[(String, String)]) -> Result<HttpReply, String> {
    let mut request = agent.get(url);
    for (key, value) in headers {
        request = request.header(key.as_str(), value.as_str());
    }
    let response = request.call().map_err(|e| e.to_string())?;
    read_reply(response)
}

/// Converts a ureq response into an `HttpReply`, reading the body as text.
/// @param response - the response from ureq
pub fn read_reply(mut response: ureq::http::Response<ureq::Body>) -> Result<HttpReply, String> {
    let status = response.status().as_u16();
    let headers = response.headers().iter().map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string())).collect();
    let body = response.body_mut().with_config().limit(64 * 1024 * 1024).read_to_string().map_err(|e| e.to_string())?;
    Ok(HttpReply { status, body, headers })
}

/// Why an interruptible request gave no reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestError {
    /// Ctrl+C was pressed. The worker thread is left to finish on its own.
    Interrupted,
    /// The request never reached the server, or its reply could not be read.
    Failed(String),
}

/// Waits for a value from a worker thread, checking for Ctrl+C every 100 ms.
/// @param receiver - the worker's channel
fn wait_for<T>(receiver: &mpsc::Receiver<T>) -> Result<T, RequestError> {
    loop {
        if interrupt::is_pending() {
            return Err(RequestError::Interrupted);
        }
        match receiver.recv_timeout(interrupt::POLL) {
            Ok(value) => return Ok(value),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(RequestError::Failed("the request thread stopped without a reply".into())),
        }
    }
}

/// Sends a JSON body with POST on a worker thread and waits for the whole reply, giving up when
/// Ctrl+C is pressed. The abandoned request keeps going until the server answers.
/// @param agent - the HTTP agent
/// @param url - the endpoint
/// @param headers - request headers
/// @param body - the JSON body
pub fn post_json_interruptible(agent: &ureq::Agent, url: &str, headers: &[(String, String)], body: &serde_json::Value) -> Result<HttpReply, RequestError> {
    let (sender, receiver) = mpsc::channel();
    let (agent, url, headers, body) = (agent.clone(), url.to_string(), headers.to_vec(), body.clone());
    std::thread::spawn(move || {
        let _ = sender.send(post_json(&agent, &url, &headers, &body));
    });
    wait_for(&receiver)?.map_err(RequestError::Failed)
}

/// One thing read from a streamed reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// The status and headers arrived.
    Start(u16),
    /// The whole body of a reply that was not 200.
    ErrorBody(String),
    /// One line of a 200 reply, without its line ending.
    Line(String),
    /// The reply ended.
    End,
    /// Reading the reply failed.
    Failed(String),
}

/// A streamed reply being read on a worker thread. Dropping it makes the worker stop at the next
/// line it reads, which closes the connection, so a server stops generating.
pub struct EventStream {
    receiver: mpsc::Receiver<StreamEvent>,
}

impl EventStream {
    /// Waits for the next event, giving up when Ctrl+C is pressed.
    pub fn next(&self) -> Result<StreamEvent, RequestError> {
        wait_for(&self.receiver)
    }
}

/// Sends a JSON body with POST and reads the reply line by line on a worker thread.
/// @param agent - the HTTP agent
/// @param url - the endpoint
/// @param headers - request headers
/// @param body - the JSON body
pub fn post_stream(agent: &ureq::Agent, url: &str, headers: &[(String, String)], body: &serde_json::Value) -> EventStream {
    let (sender, receiver) = mpsc::channel();
    let (agent, url, headers, body) = (agent.clone(), url.to_string(), headers.to_vec(), body.clone());
    std::thread::spawn(move || stream_worker(&agent, &url, &headers, &body, &sender));
    EventStream { receiver }
}

/// The worker for `post_stream`: sends the request, then each line, stopping as soon as nobody
/// is listening.
/// @param agent - the HTTP agent
/// @param url - the endpoint
/// @param headers - request headers
/// @param body - the JSON body
/// @param sender - the channel to the waiting caller
fn stream_worker(agent: &ureq::Agent, url: &str, headers: &[(String, String)], body: &serde_json::Value, sender: &mpsc::Sender<StreamEvent>) {
    let mut request = agent.post(url).header("content-type", "application/json").header("accept", "text/event-stream");
    for (key, value) in headers {
        request = request.header(key.as_str(), value.as_str());
    }
    let mut response = match request.send(body.to_string()) {
        Ok(response) => response,
        Err(error) => {
            let _ = sender.send(StreamEvent::Failed(error.to_string()));
            return;
        }
    };
    let status = response.status().as_u16();
    if sender.send(StreamEvent::Start(status)).is_err() {
        return;
    }
    if status != 200 {
        let text = response.body_mut().read_to_string().unwrap_or_default();
        let _ = sender.send(StreamEvent::ErrorBody(text));
        return;
    }
    let reader = std::io::BufReader::new(response.into_body().into_reader());
    for line in std::io::BufRead::lines(reader) {
        let event = match line {
            Ok(line) => StreamEvent::Line(line),
            Err(error) => StreamEvent::Failed(error.to_string()),
        };
        let failed = matches!(event, StreamEvent::Failed(_));
        if sender.send(event).is_err() || failed {
            return;
        }
    }
    let _ = sender.send(StreamEvent::End);
}
