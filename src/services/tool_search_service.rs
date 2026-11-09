//! Deferred MCP tools, the way Claude Code defers them. When the MCP tool definitions would take
//! a large share of the context window, the model is sent only their names and a `tool_search`
//! tool. Calling `tool_search` loads the definitions of the tools it finds, and those tools are
//! sent with every later request. Without this, one "Hello" with every MCP server on this machine
//! was a 214,611 token request, 124,715 of them tool definitions (task-2588).

use serde_json::{json, Value};

use super::providers::ToolSpec;
use crate::models::config::DeferTools;
use crate::models::context::McpTool;

/// The name of the tool that loads deferred tools.
pub const TOOL_SEARCH_NAME: &str = "tool_search";

/// The server name Umbra's own tools carry when they travel as MCP actions.
pub const BUILTIN_SERVER: &str = "umbra";

/// The most tools one keyword search loads when the model does not say.
const DEFAULT_MAX_RESULTS: usize = 5;

/// The `tool_search` tool as an MCP tool, so the parsers and the action display treat it like
/// one. Its server is `umbra`.
pub fn tool_search_tool() -> McpTool {
    McpTool { server: BUILTIN_SERVER.into(), name: TOOL_SEARCH_NAME.into(), description: String::new(), input_schema: json!({}) }
}

/// The `tool_search` tool definition sent to the model.
pub fn tool_search_spec() -> ToolSpec {
    ToolSpec {
        name: TOOL_SEARCH_NAME.into(),
        description: "Loads the definitions of deferred MCP tools so they can be called. The system prompt lists the deferred tools by name. Pass `select:<name>,<name>` to load tools by their exact names, or keywords to find tools whose name or description match. The tools found are loaded and can be called by name from the next reply.".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "`select:<name>,<name>` for exact names, or keywords" },
                "max_results": { "type": "integer", "description": "The most tools a keyword search loads (default 5)" }
            },
            "required": ["query"]
        }),
    }
}

/// Returns true when a call is to Umbra's own tool search.
/// @param server - the action's server
/// @param tool - the action's tool name
pub fn is_tool_search(server: &str, tool: &str) -> bool {
    server == BUILTIN_SERVER && tool == TOOL_SEARCH_NAME
}

/// Decides whether to defer the MCP tool definitions. `auto` defers when they would take more
/// than `ratio` of the context window.
/// @param mode - auto, always or never
/// @param tools_tokens - the estimated size of every MCP tool definition
/// @param window - the context window in tokens
/// @param ratio - the share of the window that turns deferral on in auto mode
pub fn should_defer(mode: DeferTools, tools_tokens: u64, window: u64, ratio: f64) -> bool {
    match mode {
        DeferTools::Never => false,
        DeferTools::Always => true,
        DeferTools::Auto => tools_tokens as f64 > window as f64 * ratio,
    }
}

/// Finds the tools a query names. `select:a,b` matches exact names, either the full
/// `mcp__server__tool` name or the tool's own name. Anything else is keywords: a tool matches when
/// every keyword is in its full name or description, and tools with keywords in their names come
/// first. Returns the indexes of the tools found, at most `max` for a keyword search.
/// @param query - what the model asked for
/// @param tools - every MCP tool
/// @param max - the most tools a keyword search returns
pub fn search_tools(query: &str, tools: &[McpTool], max: usize) -> Vec<usize> {
    let query = query.trim();
    if let Some(names) = query.strip_prefix("select:") {
        let wanted: Vec<&str> = names.split(',').map(str::trim).filter(|n| !n.is_empty()).collect();
        return (0..tools.len()).filter(|&i| wanted.iter().any(|w| tools[i].qualified_name() == *w || tools[i].name == *w)).collect();
    }
    let terms: Vec<String> = query.split_whitespace().map(|t| t.trim_start_matches('+').to_lowercase()).filter(|t| !t.is_empty()).collect();
    if terms.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(usize, usize)> = tools
        .iter()
        .enumerate()
        .filter_map(|(index, tool)| {
            let name = tool.qualified_name().to_lowercase();
            let description = tool.description.to_lowercase();
            let all = terms.iter().all(|t| name.contains(t.as_str()) || description.contains(t.as_str()));
            all.then(|| (index, terms.iter().filter(|t| name.contains(t.as_str())).count()))
        })
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    scored.into_iter().take(max.max(1)).map(|(index, _)| index).collect()
}

/// The most results a `tool_search` call asked for, or the default.
/// @param arguments - the call's arguments
pub fn max_results(arguments: &Value) -> usize {
    arguments.get("max_results").and_then(Value::as_u64).map_or(DEFAULT_MAX_RESULTS, |n| n.clamp(1, 50) as usize)
}

/// The text `tool_search` returns: each tool found with its description and argument schema, or
/// what to try when nothing matched.
/// @param found - the tools found
/// @param query - what the model asked for
pub fn describe_found(found: &[&McpTool], query: &str) -> String {
    if found.is_empty() {
        return format!("No deferred tool matches '{query}'. Use `select:` with names from the deferred tools list in the system prompt, or other keywords.");
    }
    let mut out = format!("Loaded {} {}. Call {} by name from your next reply.\n", found.len(), if found.len() == 1 { "tool" } else { "tools" }, if found.len() == 1 { "it" } else { "them" });
    for tool in found {
        out.push_str(&format!("\n{}: {}\narguments schema: {}\n", tool.qualified_name(), tool.description.trim(), tool.input_schema));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(server: &str, name: &str, description: &str) -> McpTool {
        McpTool { server: server.into(), name: name.into(), description: description.into(), input_schema: json!({ "type": "object" }) }
    }

    fn tools() -> Vec<McpTool> {
        vec![tool("firefox", "navigate_page", "Open a URL in the page"), tool("firefox", "take_snapshot", "Snapshot the page DOM"), tool("chordical", "chordical_track", "Add or change a track"), tool("playwright", "browser_navigate", "Navigate to a URL")]
    }

    #[test]
    fn select_loads_exact_names() {
        assert_eq!(search_tools("select:mcp__firefox__take_snapshot,chordical_track", &tools(), 5), vec![1, 2]);
        assert!(search_tools("select:nope", &tools(), 5).is_empty());
    }

    #[test]
    fn keywords_need_every_term_and_rank_name_matches_first() {
        assert_eq!(search_tools("navigate url", &tools(), 5), vec![0, 3]);
        assert_eq!(search_tools("page", &tools(), 5), vec![0, 1]);
        assert_eq!(search_tools("+firefox snapshot", &tools(), 5), vec![1]);
        assert_eq!(search_tools("navigate", &tools(), 1).len(), 1);
    }

    #[test]
    fn auto_defers_only_past_the_share_of_the_window() {
        assert!(should_defer(DeferTools::Auto, 124_715, 200_000, 0.1));
        assert!(!should_defer(DeferTools::Auto, 5_000, 200_000, 0.1));
        assert!(!should_defer(DeferTools::Never, 124_715, 200_000, 0.1));
        assert!(should_defer(DeferTools::Always, 10, 200_000, 0.1));
    }

    #[test]
    fn the_result_lists_each_tool_with_its_schema() {
        let all = tools();
        let text = describe_found(&[&all[1]], "snapshot");
        assert!(text.starts_with("Loaded 1 tool.") && text.contains("mcp__firefox__take_snapshot: Snapshot the page DOM") && text.contains("arguments schema: {\"type\":\"object\"}"));
        assert!(describe_found(&[], "x").starts_with("No deferred tool matches 'x'"));
    }
}
