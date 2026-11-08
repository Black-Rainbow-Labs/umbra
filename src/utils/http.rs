//! A small blocking HTTP client wrapper. Status codes are returned, never turned into errors,
//! so callers decide what to retry.

use std::time::Duration;

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
