//! Starts a fake MCP server over stdio and checks the handshake, tool listing and tool calls in
//! both protocol eras. The fake server is this test binary itself, started with
//! `UMBRA_FAKE_MCP` set, so the tests need nothing installed.

mod common;

use std::io::{BufRead, Write};

use common::{read_trajectory, run_umbra, slash, text, Scratch};
use serde_json::{json, Value};

/// When `UMBRA_FAKE_MCP` is set, this test becomes an MCP server on stdin and stdout and never
/// returns. `legacy` speaks the initialize handshake; `modern` refuses it and requires `_meta`.
#[test]
fn fake_mcp_server() {
    let Ok(era) = std::env::var("UMBRA_FAKE_MCP") else { return };
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    writeln!(out).unwrap();
    for line in stdin.lock().lines().map_while(|l| l.ok()) {
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        if message.get("id").is_none() || message.get("method").is_none() {
            continue;
        }
        if message["method"] == "tools/call" {
            writeln!(out, "{}", json!({ "jsonrpc": "2.0", "id": "server-ping", "method": "ping" })).unwrap();
        }
        let reply = fake_reply(&era, &message);
        writeln!(out, "{reply}").unwrap();
        out.flush().unwrap();
    }
    std::process::exit(0);
}

/// Builds the fake server's reply to one request.
/// @param era - legacy or modern
/// @param message - the request
fn fake_reply(era: &str, message: &Value) -> Value {
    let id = message["id"].clone();
    let method = message["method"].as_str().unwrap_or("");
    let has_meta = message["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"] == "2026-07-28";
    let error = |code: i64, text: &str| json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": text } });
    if era == "modern" && method == "initialize" {
        return error(-32601, "Method not found");
    }
    if era == "modern" && !has_meta {
        return error(-32602, "missing _meta");
    }
    let result = match method {
        "initialize" => json!({ "protocolVersion": "2025-06-18", "capabilities": { "tools": {} }, "serverInfo": { "name": "fake" }, "instructions": "Fake server instructions." }),
        "server/discover" => json!({ "supportedVersions": ["2026-07-28"] }),
        "tools/list" => json!({ "tools": [{ "name": "echo", "description": "Echo the text back", "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] } }] }),
        "tools/call" => json!({ "content": [{ "type": "text", "text": format!("echo: {}", message["params"]["arguments"]["text"].as_str().unwrap_or("")) }], "isError": false }),
        _ => return error(-32601, "Method not found"),
    };
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// Writes a config that starts this test binary as the fake server.
/// @param s - the test folder
/// @param era - legacy or modern
fn fake_server_config(s: &Scratch, era: &str) -> String {
    let exe = slash(&std::env::current_exe().unwrap());
    let config = format!("[mcp_servers.fake]\ncommand = \"{exe}\"\nargs = [\"fake_mcp_server\", \"--exact\", \"--nocapture\", \"--test-threads\", \"1\"]\nenv = {{ UMBRA_FAKE_MCP = \"{era}\" }}\n");
    slash(&s.write("mcp.toml", &config))
}

/// Connects to the fake server with `umbra mcp --connect --json`.
/// @param era - legacy or modern
fn connect(era: &str) -> Value {
    let s = Scratch::new(&format!("mcp-{era}"));
    let config = fake_server_config(&s, era);
    let output = run_umbra(&s.path("home"), &["mcp", "--connect", "--json", "-c", &config, "--cwd", &slash(&s.root)]);
    assert!(output.status.success(), "{}", text(&output));
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| panic!("not JSON: {}", text(&output)))
}

#[test]
fn connects_to_a_legacy_server() {
    let result = connect("legacy");
    assert_eq!(result["connected"][0], json!(["fake", "2025-06-18", 1]), "{result}");
    assert_eq!(result["tools"][0]["name"], "echo");
}

#[test]
fn connects_to_a_server_without_the_initialize_handshake() {
    let result = connect("modern");
    assert_eq!(result["connected"][0], json!(["fake", "2026-07-28", 1]), "{result}");
}

#[test]
fn the_agent_calls_an_mcp_tool_and_sees_the_result() {
    let s = Scratch::new("mcp-agent");
    let config = fake_server_config(&s, "legacy");
    let replies = json!([
        { "content": "", "tool_calls": [{ "name": "mcp__fake__echo", "arguments": { "text": "ping" } }] },
        { "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT" } }] }
    ]);
    let script = s.write("script.json", &replies.to_string());
    let traj = s.path("run.traj.json");
    let model = format!("replay/{}", slash(&script));
    let output = run_umbra(&s.path("home"), &["-m", &model, "-y", "-c", &config, "--cwd", &slash(&s.root), "-o", &slash(&traj), "task"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let trajectory = read_trajectory(&traj);
    assert!(trajectory["messages"][0]["content"].as_str().unwrap().contains("Fake server instructions."), "server instructions reach the system prompt");
    assert_eq!(trajectory["messages"][3]["extra"]["raw_output"], "echo: ping");
}
