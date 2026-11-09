//! Drives `umbra acp` the way an editor does: starts the real binary, writes JSON-RPC lines on its
//! stdin and reads its stdout. Every stdout line must be a JSON-RPC message, which every test
//! checks by parsing each one. The model is the replay provider, so nothing needs a network.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use common::{recorded_requests, replay_model, slash, umbra_command, Scratch};
use serde_json::{json, Value};

/// When `UMBRA_FAKE_MCP` is set, this test becomes a small MCP server with one tool, `echo`.
#[test]
fn fake_mcp_server() {
    if std::env::var("UMBRA_FAKE_MCP").is_err() {
        return;
    }
    let mut out = std::io::stdout();
    for line in std::io::stdin().lock().lines().map_while(|l| l.ok()) {
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(id) = message.get("id").cloned() else { continue };
        let result = match message["method"].as_str().unwrap_or("") {
            "initialize" => json!({ "protocolVersion": "2025-06-18", "capabilities": { "tools": {} }, "serverInfo": { "name": "fake" } }),
            "tools/list" => json!({ "tools": [{ "name": "echo", "description": "Echo the text back", "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } } } }] }),
            "tools/call" => json!({ "content": [{ "type": "text", "text": format!("echo: {}", message["params"]["arguments"]["text"].as_str().unwrap_or("")) }] }),
            _ => json!({}),
        };
        writeln!(out, "{}", json!({ "jsonrpc": "2.0", "id": id, "result": result })).unwrap();
        out.flush().unwrap();
    }
    std::process::exit(0);
}

/// A running `umbra acp` and what it has said so far.
struct Editor {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    next_id: u64,
    /// Every notification received, in order.
    updates: Vec<Value>,
    /// Every request Umbra sent to the editor, in order.
    asked: Vec<Value>,
}

impl Editor {
    /// Starts `umbra acp` with a fake home folder.
    /// @param home - the fake home folder
    /// @param args - the arguments after `acp`
    /// @param envs - extra environment variables
    fn start(home: &Path, args: &[&str], envs: &[(&str, &str)]) -> Self {
        let mut all = vec!["acp"];
        all.extend_from_slice(args);
        let mut command = umbra_command(home, &all);
        let stderr = if std::env::var_os("UMBRA_ACP_TEST_STDERR").is_some() { Stdio::inherit() } else { Stdio::null() };
        command.envs(envs.iter().copied()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(stderr);
        let mut child = command.spawn().expect("umbra runs");
        let stdout = child.stdout.take().unwrap();
        let (sender, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(|l| l.ok()) {
                if sender.send(line).is_err() {
                    return;
                }
            }
        });
        Editor { stdin: child.stdin.take(), child, lines, next_id: 1, updates: Vec::new(), asked: Vec::new() }
    }

    /// Writes one message.
    /// @param message - the JSON-RPC message
    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    /// Sends a request and returns its id.
    /// @param method - the method
    /// @param params - its parameters
    fn ask(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        id
    }

    /// Reads the next message, which must be JSON-RPC.
    /// @param limit - how long to wait
    fn read(&mut self, limit: Duration) -> Value {
        let line = self.lines.recv_timeout(limit).unwrap_or_else(|_| panic!("umbra sent nothing for {limit:?}. Updates so far: {:#?}", self.updates));
        let message: Value = serde_json::from_str(&line).unwrap_or_else(|_| panic!("stdout held a line that is not JSON: {line}"));
        assert_eq!(message["jsonrpc"], "2.0", "not JSON-RPC: {line}");
        message
    }

    /// Waits for the answer to request `id`, keeping the notifications and answering Umbra's own
    /// requests with `answer`.
    /// @param id - the request id
    /// @param answer - the result for each request Umbra sends
    fn answer_of(&mut self, id: u64, mut answer: impl FnMut(&Value) -> Value) -> Value {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let message = self.read(deadline.saturating_duration_since(Instant::now()));
            if message["id"] == id && message.get("method").is_none() {
                return message;
            }
            match (message.get("method"), message.get("id")) {
                (Some(_), Some(their_id)) => {
                    let result = answer(&message);
                    let their_id = their_id.clone();
                    self.asked.push(message);
                    self.send(&json!({ "jsonrpc": "2.0", "id": their_id, "result": result }));
                }
                (Some(_), None) => self.updates.push(message),
                _ => panic!("an answer to nothing: {message}"),
            }
        }
    }

    /// Sends a request and waits for its answer, refusing every request Umbra sends.
    /// @param method - the method
    /// @param params - its parameters
    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.ask(method, params);
        self.answer_of(id, |_| json!({ "outcome": { "outcome": "cancelled" } }))
    }

    /// `initialize` with the usual capabilities.
    fn initialize(&mut self) -> Value {
        self.call("initialize", json!({ "protocolVersion": 1, "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false }, "terminal": false }, "clientInfo": { "name": "test", "version": "1" } }))
    }

    /// Opens a session in a folder and returns its id.
    /// @param cwd - the working folder
    fn new_session(&mut self, cwd: &Path) -> String {
        let answer = self.call("session/new", json!({ "cwd": slash(cwd), "mcpServers": [] }));
        answer["result"]["sessionId"].as_str().unwrap_or_else(|| panic!("no session: {answer}")).to_string()
    }

    /// The `update` objects of the notifications so far, of one kind.
    /// @param kind - the `sessionUpdate` value
    fn updates_of(&self, kind: &str) -> Vec<Value> {
        self.updates.iter().map(|u| u["params"]["update"].clone()).filter(|u| u["sessionUpdate"] == kind).collect()
    }

    /// Closes stdin and waits for umbra to exit.
    fn finish(mut self) {
        self.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "umbra acp exited with {status}");
                while let Ok(line) = self.lines.recv_timeout(Duration::from_millis(200)) {
                    serde_json::from_str::<Value>(&line).unwrap_or_else(|_| panic!("stdout held a line that is not JSON: {line}"));
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        panic!("umbra acp did not exit after stdin closed");
    }
}

/// A prompt with one text block.
/// @param session - the session id
/// @param text - the text
fn prompt(session: &str, text: &str) -> Value {
    json!({ "sessionId": session, "prompt": [{ "type": "text", "text": text }] })
}

/// A scripted reply that runs one command.
/// @param content - the reply text
/// @param command - the command
fn run(content: &str, command: &str) -> Value {
    json!({ "content": content, "tool_calls": [{ "name": "bash", "arguments": { "command": command } }] })
}

/// A scripted reply that submits the final answer.
/// @param answer - the final answer
fn submit(answer: &str) -> Value {
    run("", &format!("echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT && echo '{answer}'"))
}

#[test]
fn initialize_offers_sessions_images_and_sign_in_methods() {
    let s = Scratch::new("acp-init");
    let mut editor = Editor::start(&s.path("home"), &[], &[]);
    let early = editor.call("session/list", json!({}));
    assert_eq!(early["error"]["code"], -32600, "{early}");
    let answer = editor.call("initialize", json!({ "protocolVersion": 1, "clientCapabilities": { "auth": { "terminal": true } } }));
    let result = &answer["result"];
    assert_eq!(result["protocolVersion"], 1);
    assert_eq!(result["agentInfo"]["name"], "umbra");
    assert_eq!(result["agentCapabilities"]["loadSession"], true);
    assert_eq!(result["agentCapabilities"]["promptCapabilities"]["image"], true);
    assert!(result["agentCapabilities"]["sessionCapabilities"]["resume"].is_object());
    let methods = result["authMethods"].as_array().unwrap();
    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0]["type"], "agent");
    assert_eq!(methods[1]["args"], json!(["--setup"]));
    let legacy = editor.call("initialize", json!({ "protocolVersion": 1, "clientCapabilities": { "_meta": { "terminal-auth": true } } }));
    let setup = &legacy["result"]["authMethods"][1];
    assert_eq!(setup["_meta"]["terminal-auth"]["args"], json!(["acp", "--setup"]), "{legacy}");
    let unknown = editor.call("session/fly", json!({}));
    assert_eq!(unknown["error"]["code"], -32601);
    let missing = editor.call("session/prompt", prompt("nope", "hi"));
    assert_eq!(missing["error"]["code"], -32002);
    editor.finish();
}

#[test]
fn a_session_without_a_model_asks_the_editor_to_sign_in() {
    let s = Scratch::new("acp-auth");
    let mut editor = Editor::start(&s.path("home"), &[], &[]);
    let answer = editor.initialize();
    assert_eq!(answer["result"]["authMethods"].as_array().unwrap().len(), 1);
    let new = editor.call("session/new", json!({ "cwd": slash(&s.root), "mcpServers": [] }));
    assert_eq!(new["error"]["code"], -32000, "{new}");
    let auth = editor.call("authenticate", json!({ "methodId": "umbra-config" }));
    assert_eq!(auth["error"]["code"], -32000, "{auth}");
    editor.finish();
}

#[test]
fn a_turn_in_yolo_mode_sends_messages_tool_calls_and_the_answer() {
    let s = Scratch::new("acp-turn");
    let work = s.path("work");
    std::fs::create_dir_all(&work).unwrap();
    s.write("work/.claude/commands/review.md", "---\ndescription: Review the code\n---\nReview $ARGUMENTS");
    let model = replay_model(&s, "turn.json", &json!([run("I will write the file.", "printf hi > out.txt && cat out.txt"), submit("all done")]));
    let mut editor = Editor::start(&s.path("home"), &["-m", &model, "--yolo"], &[]);
    editor.initialize();
    let session = editor.new_session(&work);
    let answer = editor.call("session/prompt", prompt(&session, "write out.txt"));
    assert_eq!(answer["result"]["stopReason"], "end_turn", "{answer}");
    assert_eq!(std::fs::read_to_string(work.join("out.txt")).unwrap(), "hi");
    let messages: Vec<String> = editor.updates_of("agent_message_chunk").iter().map(|u| u["content"]["text"].as_str().unwrap().to_string()).collect();
    assert_eq!(messages, vec!["I will write the file.", "all done"]);
    let calls = editor.updates_of("tool_call");
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["kind"], "execute");
    assert_eq!(calls[0]["title"], "printf hi > out.txt && cat out.txt");
    assert_eq!(calls[0]["rawInput"]["command"], "printf hi > out.txt && cat out.txt");
    let results = editor.updates_of("tool_call_update");
    let first: Vec<&Value> = results.iter().filter(|u| u["toolCallId"] == calls[0]["toolCallId"]).collect();
    assert_eq!(first[0]["status"], "in_progress");
    assert_eq!(first[1]["status"], "completed");
    assert_eq!(first[1]["rawOutput"]["output"], "hi");
    assert!(!editor.updates_of("usage_update").is_empty());
    assert_eq!(editor.updates_of("session_info_update")[0]["title"], "write out.txt");
    let commands = &editor.updates_of("available_commands_update")[0]["availableCommands"];
    assert!(commands.as_array().unwrap().iter().any(|c| c["name"] == "review" && c["description"] == "Review the code"), "{commands}");
    editor.finish();
}

#[test]
fn a_reply_without_a_command_ends_the_prompt_and_is_sent_once() {
    let s = Scratch::new("acp-reply");
    let work = s.path("work");
    std::fs::create_dir_all(&work).unwrap();
    let model = replay_model(&s, "reply.json", &json!(["Hi! What would you like to work on?", run("Listing.", "echo listed"), "One file."]));
    let mut editor = Editor::start(&s.path("home"), &["-m", &model, "--yolo"], &[]);
    editor.initialize();
    let session = editor.new_session(&work);
    assert_eq!(editor.call("session/prompt", prompt(&session, "Hello"))["result"]["stopReason"], "end_turn");
    assert_eq!(editor.call("session/prompt", prompt(&session, "list the files"))["result"]["stopReason"], "end_turn");
    let messages: Vec<String> = editor.updates_of("agent_message_chunk").iter().map(|u| u["content"]["text"].as_str().unwrap().to_string()).collect();
    assert_eq!(messages, vec!["Hi! What would you like to work on?", "Listing.", "One file."]);
    assert_eq!(editor.updates_of("tool_call").len(), 1);
    editor.finish();
}

#[test]
fn confirm_mode_asks_before_each_command_and_remembers_always_allow() {
    let s = Scratch::new("acp-confirm");
    let work = s.path("work");
    std::fs::create_dir_all(&work).unwrap();
    let record = s.path("requests.jsonl");
    let model = replay_model(&s, "confirm.json", &json!([run("", "printf a > a.txt"), run("", "printf b > b.txt"), run("", "printf c > c.txt"), submit("ok")]));
    let mut editor = Editor::start(&s.path("home"), &["-m", &model], &[("UMBRA_REPLAY_RECORD", &slash(&record))]);
    editor.initialize();
    let session = editor.new_session(&work);
    let mut answers = vec!["reject", "allow-always", "allow"].into_iter();
    let id = editor.ask("session/prompt", prompt(&session, "write three files"));
    let answer = editor.answer_of(id, |request| {
        assert_eq!(request["method"], "session/request_permission");
        json!({ "outcome": { "outcome": "selected", "optionId": answers.next().expect("asked too often") } })
    });
    assert_eq!(answer["result"]["stopReason"], "end_turn", "{answer}");
    assert_eq!(editor.asked.len(), 3, "printf was always allowed after the second question, and echo was asked about");
    assert_eq!(editor.asked[0]["params"]["toolCall"]["title"], "printf a > a.txt");
    assert!(editor.asked[2]["params"]["toolCall"]["title"].as_str().unwrap().starts_with("echo COMPLETE_TASK"));
    assert!(editor.asked[0]["params"]["options"].as_array().unwrap().iter().any(|o| o["kind"] == "allow_always"));
    assert!(!work.join("a.txt").exists());
    assert!(work.join("b.txt").exists() && work.join("c.txt").exists());
    let requests = recorded_requests(&record);
    let second: &Value = &requests[1]["messages"];
    let last = second.as_array().unwrap().last().unwrap();
    assert!(last["content"].as_str().unwrap().contains("The user rejected your commands"), "{last}");
    editor.finish();
}

#[test]
fn cancel_stops_a_command_and_a_model_call_and_the_session_carries_on() {
    let s = Scratch::new("acp-cancel");
    let work = s.path("work");
    std::fs::create_dir_all(&work).unwrap();
    let model = replay_model(&s, "cancel.json", &json!([run("", "sleep 30"), { "content": "", "sleep_ms": 30000 }, submit("after")]));
    let mut editor = Editor::start(&s.path("home"), &["-m", &model, "--yolo"], &[]);
    editor.initialize();
    let session = editor.new_session(&work);
    let id = editor.ask("session/prompt", prompt(&session, "wait"));
    loop {
        let message = editor.read(Duration::from_secs(30));
        let update = &message["params"]["update"];
        let running = update["sessionUpdate"] == "tool_call_update" && update["status"] == "in_progress";
        editor.updates.push(message);
        if running {
            break;
        }
    }
    let started = Instant::now();
    editor.send(&json!({ "jsonrpc": "2.0", "method": "session/cancel", "params": { "sessionId": session } }));
    let answer = editor.answer_of(id, |_| json!({}));
    assert_eq!(answer["result"]["stopReason"], "cancelled", "{answer}");
    assert!(started.elapsed() < Duration::from_secs(10), "the command was not stopped: {:?}", started.elapsed());
    let failed = editor.updates_of("tool_call_update").into_iter().find(|u| u["status"] == "failed").expect("the command failed");
    assert!(failed["rawOutput"]["exception_info"].as_str().unwrap().contains("interrupted"), "{failed}");

    let id = editor.ask("session/prompt", prompt(&session, "think slowly"));
    std::thread::sleep(Duration::from_millis(500));
    let started = Instant::now();
    editor.send(&json!({ "jsonrpc": "2.0", "method": "session/cancel", "params": { "sessionId": session } }));
    let answer = editor.answer_of(id, |_| json!({}));
    assert_eq!(answer["result"]["stopReason"], "cancelled", "{answer}");
    assert!(started.elapsed() < Duration::from_secs(5));

    let answer = editor.call("session/prompt", prompt(&session, "now finish"));
    assert_eq!(answer["result"]["stopReason"], "end_turn", "{answer}");
    editor.finish();
}

#[test]
fn a_session_continues_in_another_process_with_resume_and_load() {
    let s = Scratch::new("acp-resume");
    let work = s.path("work");
    std::fs::create_dir_all(&work).unwrap();
    let home = s.path("home");
    let record = s.path("requests.jsonl");
    let first = replay_model(&s, "first.json", &json!([run("Listing.", "echo listed"), submit("first answer")]));
    let mut editor = Editor::start(&home, &["-m", &first, "--yolo"], &[]);
    editor.initialize();
    let session = editor.new_session(&work);
    assert_eq!(editor.call("session/prompt", prompt(&session, "list the files"))["result"]["stopReason"], "end_turn");
    editor.finish();

    let second = replay_model(&s, "second.json", &json!([submit("second answer")]));
    let mut editor = Editor::start(&home, &["-m", &second, "--yolo"], &[("UMBRA_REPLAY_RECORD", &slash(&record))]);
    editor.initialize();
    let resumed = editor.call("session/resume", json!({ "sessionId": session, "cwd": slash(&work), "mcpServers": [] }));
    assert!(resumed["result"]["modes"].is_object(), "{resumed}");
    assert!(editor.updates_of("user_message_chunk").is_empty(), "resume sends no history");
    assert_eq!(editor.call("session/prompt", prompt(&session, "and then?"))["result"]["stopReason"], "end_turn");
    let sent = &recorded_requests(&record)[0]["messages"];
    let texts: Vec<&str> = sent.as_array().unwrap().iter().filter_map(|m| m["content"].as_str()).collect();
    assert!(texts.iter().any(|t| t.contains("list the files")), "the old task reached the model");
    assert!(texts.iter().any(|t| t.contains("listed")), "the old output reached the model");
    assert_eq!(*texts.last().unwrap(), "and then?", "a follow up is the person's words as they typed them");
    let listed = editor.call("session/list", json!({ "cwd": slash(&work) }));
    let entry = listed["result"]["sessions"].as_array().unwrap().iter().find(|e| e["sessionId"] == session.as_str()).cloned().expect("listed");
    assert_eq!(entry["title"], "list the files");
    assert!(entry["updatedAt"].as_str().unwrap().ends_with('Z'));
    editor.finish();

    let mut editor = Editor::start(&home, &["-m", &second], &[]);
    editor.initialize();
    let loaded = editor.call("session/load", json!({ "sessionId": session, "cwd": slash(&work), "mcpServers": [] }));
    assert!(loaded["error"].is_null(), "{loaded}");
    let said: Vec<String> = editor.updates_of("user_message_chunk").iter().map(|u| u["content"]["text"].as_str().unwrap().to_string()).collect();
    assert_eq!(said, vec!["list the files", "and then?"]);
    let answers: Vec<String> = editor.updates_of("agent_message_chunk").iter().map(|u| u["content"]["text"].as_str().unwrap().to_string()).collect();
    assert_eq!(answers, vec!["Listing.", "first answer", "second answer"]);
    let done: Vec<Value> = editor.updates_of("tool_call_update").into_iter().filter(|u| u["status"] == "completed").collect();
    assert_eq!(done.len(), 3);
    assert_eq!(done[0]["rawOutput"]["output"], "listed\n");
    let missing = editor.call("session/load", json!({ "sessionId": "123", "cwd": slash(&work), "mcpServers": [] }));
    assert_eq!(missing["error"]["code"], -32002);
    editor.finish();
}

#[test]
fn images_mcp_servers_and_options_from_the_editor_reach_the_agent() {
    let s = Scratch::new("acp-extras");
    let work = s.path("work");
    std::fs::create_dir_all(&work).unwrap();
    let record = s.path("requests.jsonl");
    let tool = json!({ "content": "", "tool_calls": [{ "name": "mcp__fake__echo", "arguments": { "text": "hello" } }] });
    let model = replay_model(&s, "extras.json", &json!([tool, submit("one")]));
    let other = replay_model(&s, "other.json", &json!([submit("from the other model")]));
    let config = s.write("models.toml", &format!("models = [\"{other}\"]\n"));
    let mut editor = Editor::start(&s.path("home"), &["-m", &model, "-c", &slash(&config)], &[("UMBRA_REPLAY_RECORD", &slash(&record))]);
    editor.initialize();
    let exe = slash(&std::env::current_exe().unwrap());
    let server = json!({ "name": "fake", "command": exe, "args": ["fake_mcp_server", "--exact", "--nocapture", "--test-threads", "1"], "env": [{ "name": "UMBRA_FAKE_MCP", "value": "1" }] });
    let opened = editor.call("session/new", json!({ "cwd": slash(&work), "mcpServers": [server] }));
    let session = opened["result"]["sessionId"].as_str().unwrap().to_string();
    let options = &opened["result"]["configOptions"];
    assert_eq!(options[1]["category"], "model");
    assert_eq!(options[1]["options"].as_array().unwrap().len(), 2, "{options}");
    let yolo = editor.call("session/set_config_option", json!({ "sessionId": session, "configId": "mode", "value": "yolo" }));
    assert_eq!(yolo["result"]["configOptions"][0]["currentValue"], "yolo", "{yolo}");

    let mut bytes = Vec::new();
    image::DynamicImage::new_rgb8(40, 30).write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let blocks = json!({ "sessionId": session, "prompt": [{ "type": "text", "text": "look and echo" }, { "type": "image", "mimeType": "image/png", "data": data }] });
    let answer = editor.call("session/prompt", blocks);
    assert_eq!(answer["result"]["stopReason"], "end_turn", "{answer}");
    let first = &recorded_requests(&record)[0];
    assert!(first["tools"].as_array().unwrap().iter().any(|t| t == "mcp__fake__echo"), "{first}");
    let user = &first["messages"][1];
    assert_eq!(user["images"][0]["width"], 40, "{user}");
    let call = editor.updates_of("tool_call").into_iter().find(|c| c["title"] == "fake.echo").expect("the MCP tool call");
    let done = editor.updates_of("tool_call_update").into_iter().find(|u| u["toolCallId"] == call["toolCallId"] && u["status"] == "completed").expect("it finished");
    assert!(done["rawOutput"]["output"].as_str().unwrap().contains("echo: hello"), "{done}");

    let switched = editor.call("session/set_config_option", json!({ "sessionId": session, "configId": "model", "value": other }));
    assert_eq!(switched["result"]["configOptions"][1]["currentValue"], other.as_str(), "{switched}");
    let answer = editor.call("session/prompt", prompt(&session, "again"));
    assert_eq!(answer["result"]["stopReason"], "end_turn");
    let last = editor.updates_of("agent_message_chunk").last().cloned().unwrap();
    assert_eq!(last["content"]["text"], "from the other model");
    let closed = editor.call("session/close", json!({ "sessionId": session }));
    assert_eq!(closed["result"], json!({}));
    editor.finish();
}
