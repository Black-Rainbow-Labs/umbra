//! Turns a model reply into actions, or into an error message for the model to fix.

use regex::Regex;
use serde_json::Value;

use super::prompt_service::DEFAULT_MCP_REGEX;
use crate::models::action::Action;
use crate::models::context::McpTool;
use crate::models::model_response::ModelResponse;

/// Reads the actions from a tool mode reply. Every call must be `bash` with a `command` string,
/// or a known `mcp__server__tool`. One bad call rejects the whole reply, as in mini-swe-agent.
/// @param response - the model reply
/// @param mcp_tools - the MCP tools that were offered
pub fn parse_tool_actions(response: &ModelResponse, mcp_tools: &[McpTool]) -> Result<Vec<Action>, String> {
    if response.tool_calls.is_empty() {
        return Err("No tool calls found in the response. Every response MUST include at least one tool call.".into());
    }
    let mut actions = Vec::new();
    let mut errors = Vec::new();
    for call in &response.tool_calls {
        let id = Some(call.id.clone());
        if let Value::String(raw) = &call.arguments {
            errors.push(format!("Error parsing tool call arguments: {raw:?} is not a JSON object."));
            continue;
        }
        if call.name == "bash" {
            match call.arguments.get("command").and_then(Value::as_str) {
                Some(command) => actions.push(Action::Bash { command: command.to_string(), tool_call_id: id }),
                None => errors.push("Missing 'command' argument in bash tool call.".into()),
            }
        } else if let Some(tool) = mcp_tools.iter().find(|t| t.qualified_name() == call.name) {
            actions.push(Action::Mcp { server: tool.server.clone(), tool: tool.name.clone(), arguments: call.arguments.clone(), tool_call_id: id });
        } else {
            errors.push(format!("Unknown tool '{}'.", call.name));
        }
    }
    if errors.is_empty() { Ok(actions) } else { Err(errors.join("\n")) }
}

/// Reads the single action from a text mode reply: exactly one bash block or MCP block.
/// @param response - the model reply
/// @param bash_regex - the regex with one capture group for the command
/// @param mcp_tools - the MCP tools that were offered
pub fn parse_text_action(response: &ModelResponse, bash_regex: &str, mcp_tools: &[McpTool]) -> Result<Vec<Action>, String> {
    let bash = Regex::new(bash_regex).map_err(|e| format!("action_regex is invalid: {e}"))?;
    let mcp = Regex::new(DEFAULT_MCP_REGEX).expect("valid regex");
    let commands: Vec<String> = bash.captures_iter(&response.content).map(|c| c[1].trim().to_string()).collect();
    let calls: Vec<String> = if mcp_tools.is_empty() { Vec::new() } else { mcp.captures_iter(&response.content).map(|c| c[1].trim().to_string()).collect() };
    let found = commands.len() + calls.len();
    if found != 1 {
        return Err(format!("Expected exactly 1 action, found {found}."));
    }
    if let Some(command) = commands.into_iter().next() {
        return Ok(vec![Action::Bash { command, tool_call_id: None }]);
    }
    parse_mcp_block(&calls[0], mcp_tools).map(|a| vec![a])
}

/// Reads a text mode MCP block: `{"tool": "mcp__server__tool", "arguments": {...}}`, or with
/// separate `server` and `tool` fields.
/// @param block - the JSON inside the block
/// @param mcp_tools - the MCP tools that were offered
fn parse_mcp_block(block: &str, mcp_tools: &[McpTool]) -> Result<Action, String> {
    let value: Value = serde_json::from_str(block).map_err(|e| format!("The umbra_mcp block is not valid JSON: {e}"))?;
    let tool_name = value["tool"].as_str().unwrap_or("");
    let server = value["server"].as_str();
    let tool = mcp_tools.iter().find(|t| t.qualified_name() == tool_name || (Some(t.server.as_str()) == server && t.name == tool_name));
    let Some(tool) = tool else { return Err(format!("Unknown MCP tool '{tool_name}'.")) };
    let arguments = value.get("arguments").cloned().unwrap_or(Value::Object(Default::default()));
    Ok(Action::Mcp { server: tool.server.clone(), tool: tool.name.clone(), arguments, tool_call_id: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::message::ToolCall;
    use crate::services::prompt_service::DEFAULT_BASH_REGEX;
    use serde_json::json;

    fn tool() -> McpTool {
        McpTool { server: "docs".into(), name: "search".into(), description: String::new(), input_schema: json!({}) }
    }

    #[test]
    fn text_mode_needs_exactly_one_block() {
        let one = ModelResponse { content: "THOUGHT\n```umbra_bash\nls -la\n```".into(), ..Default::default() };
        assert_eq!(parse_text_action(&one, DEFAULT_BASH_REGEX, &[]).unwrap(), vec![Action::Bash { command: "ls -la".into(), tool_call_id: None }]);
        let two = ModelResponse { content: "```umbra_bash\nls\n```\n```umbra_bash\npwd\n```".into(), ..Default::default() };
        assert_eq!(parse_text_action(&two, DEFAULT_BASH_REGEX, &[]).unwrap_err(), "Expected exactly 1 action, found 2.");
        let mcp = ModelResponse { content: "```umbra_mcp\n{\"tool\": \"mcp__docs__search\", \"arguments\": {\"q\": \"x\"}}\n```".into(), ..Default::default() };
        assert!(matches!(&parse_text_action(&mcp, DEFAULT_BASH_REGEX, &[tool()]).unwrap()[0], Action::Mcp { tool, .. } if tool == "search"));
    }

    #[test]
    fn tool_mode_rejects_unknown_tools_and_missing_commands() {
        let response = ModelResponse { tool_calls: vec![ToolCall { id: "1".into(), name: "edit".into(), arguments: json!({}) }], ..Default::default() };
        assert_eq!(parse_tool_actions(&response, &[]).unwrap_err(), "Unknown tool 'edit'.");
        let response = ModelResponse { tool_calls: vec![ToolCall { id: "1".into(), name: "bash".into(), arguments: json!({"cmd": "ls"}) }], ..Default::default() };
        assert!(parse_tool_actions(&response, &[]).unwrap_err().contains("Missing 'command'"));
        let response = ModelResponse { tool_calls: vec![ToolCall { id: "1".into(), name: "mcp__docs__search".into(), arguments: json!({"q": 1}) }], ..Default::default() };
        assert_eq!(parse_tool_actions(&response, &[tool()]).unwrap().len(), 1);
    }
}
