//! The JSON-RPC side of the ACP server: writing one message per line on stdout, numbering the
//! requests Umbra sends to the editor, and handing each answer to the thread that waits for it.

use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;

use serde_json::{json, Value};

use crate::utils::interrupt;

/// JSON-RPC: the line was not JSON.
pub const PARSE_ERROR: i64 = -32700;
/// JSON-RPC: not a request, or a request other than `initialize` before `initialize`.
pub const INVALID_REQUEST: i64 = -32600;
/// JSON-RPC: no such method, or one this agent did not offer.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// JSON-RPC: the parameters are wrong.
pub const INVALID_PARAMS: i64 = -32602;
/// JSON-RPC: the agent failed.
pub const INTERNAL_ERROR: i64 = -32603;
/// ACP: the person has to sign in before a session can start.
pub const AUTH_REQUIRED: i64 = -32000;
/// ACP: no such session.
pub const RESOURCE_NOT_FOUND: i64 = -32002;

/// An error answer to a request.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcError {
    /// Builds an error with no data.
    /// @param code - the JSON-RPC error code
    /// @param message - what went wrong, for the person
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        RpcError { code, message: message.into(), data: None }
    }

    /// The error for a session id this process does not know.
    /// @param session_id - the id the editor sent
    pub fn no_session(session_id: &str) -> Self {
        RpcError { code: RESOURCE_NOT_FOUND, message: format!("no session {session_id}"), data: Some(json!({ "sessionId": session_id })) }
    }
}

/// What an answer from the editor to one of Umbra's requests held.
pub type Answer = Result<Value, RpcError>;

/// Stdout, shared by every thread, and the requests waiting for an answer.
pub struct Wire {
    out: Mutex<Box<dyn Write + Send>>,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, Sender<Answer>>>,
}

impl Wire {
    /// Builds the wire over a writer, normally stdout.
    /// @param out - where messages are written
    pub fn new(out: Box<dyn Write + Send>) -> Self {
        Wire { out: Mutex::new(out), next_id: AtomicU64::new(1), pending: Mutex::new(HashMap::new()) }
    }

    /// Writes one message as one line and flushes it. The lock keeps two threads' lines apart.
    /// @param message - the JSON-RPC message
    pub fn send(&self, message: &Value) {
        let line = format!("{message}\n");
        let mut out = self.out.lock().unwrap_or_else(|e| e.into_inner());
        if out.write_all(line.as_bytes()).and_then(|_| out.flush()).is_err() {
            eprintln!("umbra: could not write to the editor");
        }
    }

    /// Answers a request with a result.
    /// @param id - the request's id
    /// @param result - the result
    pub fn respond(&self, id: &Value, result: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }

    /// Answers a request with an error.
    /// @param id - the request's id, or null when it could not be read
    /// @param error - the error
    pub fn respond_error(&self, id: &Value, error: &RpcError) {
        let mut body = json!({ "code": error.code, "message": error.message });
        if let Some(data) = &error.data {
            body["data"] = data.clone();
        }
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "error": body }));
    }

    /// Answers a request with whichever of a result or an error it got.
    /// @param id - the request's id
    /// @param answer - the result or the error
    pub fn reply(&self, id: &Value, answer: Answer) {
        match answer {
            Ok(result) => self.respond(id, result),
            Err(error) => self.respond_error(id, &error),
        }
    }

    /// Sends a notification.
    /// @param method - the method name
    /// @param params - its parameters
    pub fn notify(&self, method: &str, params: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// Sends a `session/update` notification.
    /// @param session_id - the session
    /// @param update - the update object, with its `sessionUpdate` field
    pub fn update(&self, session_id: &str, update: Value) {
        self.notify("session/update", json!({ "sessionId": session_id, "update": update }));
    }

    /// Sends a request to the editor and returns the channel its answer arrives on.
    /// @param method - the method name
    /// @param params - its parameters
    pub fn request(&self, method: &str, params: Value) -> Receiver<Answer> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = mpsc::channel();
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(id, sender);
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        receiver
    }

    /// Hands an answer from the editor to the thread waiting for it. An answer to nothing that is
    /// waiting is dropped.
    /// @param id - the id of Umbra's request
    /// @param answer - what the editor answered
    pub fn deliver(&self, id: &Value, answer: Answer) {
        let Some(id) = id.as_u64() else { return };
        if let Some(sender) = self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id) {
            let _ = sender.send(answer);
        }
    }

    /// Drops every waiting request, so the threads waiting on them stop waiting. Used when the
    /// editor closes stdin.
    pub fn abandon_pending(&self) {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

/// How a wait for the editor's answer ended.
pub enum Waited {
    Answered(Answer),
    /// The session was cancelled, or the editor went away, before it answered.
    Cancelled,
}

/// Waits for the editor's answer, giving up when this thread's session is cancelled.
/// @param receiver - the channel `Wire::request` returned
pub fn wait_for_answer(receiver: &Receiver<Answer>) -> Waited {
    loop {
        if interrupt::is_pending() {
            return Waited::Cancelled;
        }
        match receiver.recv_timeout(interrupt::POLL) {
            Ok(answer) => return Waited::Answered(answer),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return Waited::Cancelled,
        }
    }
}

/// Reads the error object of a JSON-RPC answer.
/// @param error - the `error` member
pub fn error_from(error: &Value) -> RpcError {
    RpcError { code: error["code"].as_i64().unwrap_or(INTERNAL_ERROR), message: error["message"].as_str().unwrap_or("").to_string(), data: error.get("data").cloned() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// A writer that keeps what was written, for reading back in a test.
    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl Write for Captured {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_request_gets_the_answer_with_its_id() {
        let captured = Captured::default();
        let wire = Wire::new(Box::new(captured.clone()));
        let receiver = wire.request("session/request_permission", json!({ "a": 1 }));
        let written: Value = serde_json::from_slice(&captured.0.lock().unwrap()).unwrap();
        assert_eq!(written["method"], "session/request_permission");
        wire.deliver(&written["id"], Ok(json!({ "ok": true })));
        assert_eq!(receiver.recv().unwrap(), Ok(json!({ "ok": true })));
    }
}
