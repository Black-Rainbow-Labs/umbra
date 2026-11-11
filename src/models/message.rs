//! The conversation history. Umbra keeps one linear list of messages, like mini-swe-agent.
//! Providers convert this neutral form into their own wire format.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::image::ImageData;

/// Who wrote a message. `Exit` is a local marker that ends the run and is never sent to a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
    Exit,
}

/// One tool call requested by the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// One entry in the conversation. `extra` holds data for the trajectory file only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Images shown with the text: attached by the person, or returned by an MCP tool.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageData>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub extra: Value,
}

impl Message {
    /// Builds a message with a role and text and nothing else.
    /// @param role - who wrote the message
    /// @param content - the message text
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Message { role, content: content.into(), tool_calls: Vec::new(), tool_call_id: None, images: Vec::new(), extra: Value::Null }
    }

    /// Builds a system message.
    /// @param content - the system prompt text
    pub fn system(content: impl Into<String>) -> Self {
        Message::new(Role::System, content)
    }

    /// Builds a user message.
    /// @param content - the user text
    pub fn user(content: impl Into<String>) -> Self {
        Message::new(Role::User, content)
    }

    /// Builds the result message for one tool call.
    /// @param tool_call_id - the id of the call this answers
    /// @param content - the rendered observation
    pub fn tool(tool_call_id: &str, content: impl Into<String>) -> Self {
        let mut message = Message::new(Role::Tool, content);
        message.tool_call_id = Some(tool_call_id.to_string());
        message
    }

    /// Builds the exit marker that ends the agent loop.
    /// @param status - the exit status, such as Submitted or LimitsExceeded
    /// @param submission - the final output of the run
    pub fn exit(status: &str, submission: &str) -> Self {
        let mut message = Message::new(Role::Exit, submission);
        message.extra = serde_json::json!({ "exit_status": status, "submission": submission });
        message
    }

    /// Returns a copy of this message with the given images attached.
    /// @param images - the images to show with the text
    pub fn with_images(mut self, images: Vec<ImageData>) -> Self {
        self.images = images;
        self
    }

    /// Returns the reasoning text an OpenAI compatible server streamed with this assistant reply,
    /// with the field name the server used (`reasoning_content` or `reasoning`). It is read from
    /// the reply kept in `extra.response`, so sessions saved by earlier versions have it too.
    pub fn reasoning(&self) -> Option<(&'static str, &str)> {
        let reply = &self.extra["response"]["choices"][0]["message"];
        ["reasoning_content", "reasoning"].into_iter().find_map(|key| reply[key].as_str().filter(|text| !text.is_empty()).map(|text| (key, text)))
    }

    /// Returns a copy of this message with the given `extra` data attached.
    /// @param extra - the trajectory data to attach
    pub fn with_extra(mut self, extra: Value) -> Self {
        self.extra = extra;
        self
    }
}
