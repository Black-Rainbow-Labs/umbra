//! Starts a fake MCP server over stdio and checks the handshake, tool listing and tool calls in
//! both protocol eras. The fake server is this test binary itself, started with
//! `UMBRA_FAKE_MCP` set, so the tests need nothing installed.

mod common;

use std::io::{BufRead, Write};

use std::time::{Duration, Instant};

use common::{read_trajectory, recorded_requests, replay_model, run_umbra, run_umbra_with, slash, text, Running, Scratch};
use serde_json::{json, Value};

/// When `UMBRA_FAKE_MCP` is set, this test becomes an MCP server on stdin and stdout and never
/// returns. `legacy` speaks the initialize handshake; `modern` refuses it and requires `_meta`;
/// `images` is legacy with two more tools: `shot` returns a 64x48 PNG and `slow` never answers.
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
        if message["params"]["name"] == "slow" {
            continue;
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
        "tools/list" if era == "images" => json!({ "tools": [echo_tool(), { "name": "shot", "description": "Take a screenshot", "inputSchema": { "type": "object" } }, { "name": "slow", "description": "Never answer", "inputSchema": { "type": "object" } }] }),
        "tools/list" => json!({ "tools": [echo_tool()] }),
        "tools/call" if message["params"]["name"] == "shot" => json!({ "content": [{ "type": "text", "text": "the screen" }, { "type": "image", "mimeType": "image/png", "data": png_base64(64, 48) }] }),
        "tools/call" => json!({ "content": [{ "type": "text", "text": format!("echo: {}", message["params"]["arguments"]["text"].as_str().unwrap_or("")) }], "isError": false }),
        _ => return error(-32601, "Method not found"),
    };
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// The fake server's echo tool.
fn echo_tool() -> Value {
    json!({ "name": "echo", "description": "Echo the text back", "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] } })
}

/// A plain PNG of the given size as base64.
/// @param width - the width in pixels
/// @param height - the height in pixels
fn png_base64(width: u32, height: u32) -> String {
    use base64::Engine;
    let mut bytes = Vec::new();
    image::DynamicImage::new_rgb8(width, height).write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    base64::engine::general_purpose::STANDARD.encode(bytes)
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

#[test]
fn an_mcp_image_reaches_the_model_after_the_tool_result() {
    let s = Scratch::new("mcp-image");
    let config = fake_server_config(&s, "images");
    let model = replay_model(&s, "script.json", &json!([
        { "content": "", "tool_calls": [{ "name": "mcp__fake__shot", "arguments": {} }] },
        { "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT" } }] }
    ]));
    let (traj, record) = (s.path("run.traj.json"), s.path("record.jsonl"));
    let output = run_umbra_with(&s.path("home"), &["-m", &model, "-y", "-c", &config, "--cwd", &slash(&s.root), "-o", &slash(&traj), "task"], &[("UMBRA_REPLAY_RECORD", &slash(&record))], "");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let requests = recorded_requests(&record);
    let result = requests[1]["messages"].as_array().unwrap().iter().find(|m| m["role"] == "tool").expect("a tool result");
    assert_eq!(result["images"][0]["media_type"], "image/png");
    assert_eq!((result["images"][0]["width"].as_u64(), result["images"][0]["height"].as_u64()), (Some(64), Some(48)));
    assert!(result["content"].as_str().unwrap().contains("[1 image(s) attached to this result]"), "{result}");
    let trajectory = read_trajectory(&traj);
    assert!(trajectory["messages"][3]["images"][0]["data"].as_str().unwrap().len() > 50);
    assert_eq!(trajectory["messages"][3]["images"][0]["source"], "mcp:fake.shot");
}

#[test]
fn ctrl_c_stops_a_slow_mcp_call_and_the_server_keeps_working() {
    let s = Scratch::new("mcp-interrupt");
    let config = fake_server_config(&s, "images");
    let model = replay_model(&s, "script.json", &json!([
        { "content": "", "tool_calls": [{ "name": "mcp__fake__slow", "arguments": {} }] },
        { "content": "", "tool_calls": [{ "name": "mcp__fake__echo", "arguments": { "text": "still here" } }] },
        { "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT" } }] }
    ]));
    let traj = s.path("run.traj.json");
    let args = ["-m", &model, "-y", "-c", &config, "--cwd", &slash(&s.root), "-o", &slash(&traj), "--exit-immediately", "task"];
    let mut running = Running::start(&s.path("home"), &args, &[("UMBRA_FORCE_INTERACTIVE", "1")]);
    std::thread::sleep(Duration::from_secs(4));
    if !running.ctrl_c() {
        eprintln!("skipped: this machine cannot send a console event");
        return;
    }
    let sent = Instant::now();
    std::thread::sleep(Duration::from_millis(500));
    running.type_line("go on");
    let output = running.finish(Duration::from_secs(20));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(sent.elapsed() < Duration::from_secs(20));
    let trajectory = read_trajectory(&traj);
    let messages = trajectory["messages"].as_array().unwrap();
    assert!(messages[3]["extra"]["exception_info"].as_str().unwrap().starts_with("The user interrupted this MCP call"), "{}", messages[3]);
    assert_eq!(messages[4]["content"], "Interrupted by user: go on");
    assert_eq!(messages[6]["extra"]["raw_output"], "echo: still here", "the MCP server survived the Ctrl+C");
}

#[test]
fn deferred_tools_are_sent_by_name_and_loaded_with_tool_search() {
    let s = Scratch::new("mcp-deferred");
    let config = fake_server_config(&s, "images");
    let defer = slash(&s.write("defer.toml", "[mcp]\ndefer_tools = \"always\"\n"));
    let model = replay_model(&s, "script.json", &json!([
        { "content": "", "tool_calls": [{ "name": "tool_search", "arguments": { "query": "select:mcp__fake__echo" } }] },
        { "content": "", "tool_calls": [{ "name": "mcp__fake__echo", "arguments": { "text": "ping" } }] },
        "The echo tool answered."
    ]));
    let (traj, record) = (s.path("run.traj.json"), s.path("record.jsonl"));
    let output = run_umbra_with(&s.path("home"), &["-m", &model, "-y", "-c", &config, "-c", &defer, "--cwd", &slash(&s.root), "-o", &slash(&traj), "task"], &[("UMBRA_REPLAY_RECORD", &slash(&record))], "");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let requests = recorded_requests(&record);
    assert_eq!(requests[0]["tools"], json!(["bash", "tool_search"]), "no MCP definitions before a search");
    let system = requests[0]["messages"][0]["content"].as_str().unwrap();
    assert!(system.contains("<deferred_tools>") && system.contains("mcp__fake__shot"), "the names are listed");
    assert!(!system.contains("Echo the text back"), "but not the descriptions");
    assert_eq!(requests[1]["tools"], json!(["bash", "tool_search", "mcp__fake__echo"]), "the search loaded echo");
    let trajectory = read_trajectory(&traj);
    let messages = trajectory["messages"].as_array().unwrap();
    assert!(messages[3]["extra"]["raw_output"].as_str().unwrap().contains("mcp__fake__echo: Echo the text back"), "{}", messages[3]);
    assert_eq!(messages[5]["extra"]["raw_output"], "echo: ping");
    assert_eq!(trajectory["info"]["exit_status"], "Replied");

    // A resumed session loads the same tools again before its first call.
    let model = replay_model(&s, "again.json", &json!(["Still here."]));
    let record2 = s.path("record2.jsonl");
    let output = run_umbra_with(&s.path("home"), &["-m", &model, "-y", "-c", &config, "-c", &defer, "--cwd", &slash(&s.root), "--resume", &slash(&traj), "and now?"], &[("UMBRA_REPLAY_RECORD", &slash(&record2))], "");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(recorded_requests(&record2)[0]["tools"], json!(["bash", "tool_search", "mcp__fake__echo"]));
}

#[test]
fn small_tool_sets_are_not_deferred() {
    let s = Scratch::new("mcp-not-deferred");
    let config = fake_server_config(&s, "legacy");
    let model = replay_model(&s, "script.json", &json!(["Hi."]));
    let record = s.path("record.jsonl");
    let output = run_umbra_with(&s.path("home"), &["-m", &model, "-y", "-c", &config, "--cwd", &slash(&s.root), "task"], &[("UMBRA_REPLAY_RECORD", &slash(&record))], "");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(recorded_requests(&record)[0]["tools"], json!(["bash", "mcp__fake__echo"]));
}
