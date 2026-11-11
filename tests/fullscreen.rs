//! Drives the real binary in the fullscreen view (`UMBRA_FORCE_FULLSCREEN=1`) and reads its
//! frames: the header, replies, command groups, MCP start, the status bar, commands, pickers,
//! selection and copy. The model side is the replay provider; the MCP side is this test binary
//! started as a fake server.

mod common;

use std::io::{BufRead, Write};
use std::time::{Duration, Instant};

use common::screen::{Frame, Screen};
use common::{recorded_requests, slash, Scratch};
use serde_json::{json, Value};

/// When `UMBRA_FAKE_MCP` is set, this test becomes an MCP server and never returns: `ok` answers
/// with one `echo` tool, `hang` never answers. Its process id is written to
/// `UMBRA_FAKE_PID_FILE`, so a test can check that it was stopped.
#[test]
fn fake_mcp_server() {
    let Ok(mode) = std::env::var("UMBRA_FAKE_MCP") else { return };
    if let Ok(path) = std::env::var("UMBRA_FAKE_PID_FILE") {
        std::fs::write(path, std::process::id().to_string()).unwrap();
    }
    let mut out = std::io::stdout();
    for line in std::io::stdin().lock().lines().map_while(|l| l.ok()) {
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        if mode == "hang" || message.get("id").is_none() {
            continue;
        }
        let result = match message["method"].as_str().unwrap_or("") {
            "initialize" => json!({ "protocolVersion": "2025-06-18", "capabilities": { "tools": {} }, "serverInfo": { "name": "fake" } }),
            "tools/list" => json!({ "tools": [{ "name": "echo", "description": "Echo the text back", "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } } } }] }),
            "tools/call" => json!({ "content": [{ "type": "text", "text": "echoed" }] }),
            _ => json!({}),
        };
        writeln!(out, "{}", json!({ "jsonrpc": "2.0", "id": message["id"], "result": result })).unwrap();
        out.flush().unwrap();
    }
    std::process::exit(0);
}

/// A test folder with a work folder, a fake home and helpers to write scripts and configs.
struct Setup {
    scratch: Scratch,
}

impl Setup {
    /// A new test folder.
    /// @param name - the test's name
    fn new(name: &str) -> Self {
        let scratch = Scratch::new(name);
        std::fs::create_dir_all(scratch.path("work")).unwrap();
        Setup { scratch }
    }

    /// Writes a replay script and returns its `-m` value.
    /// @param name - the file name
    /// @param replies - the scripted replies
    fn model(&self, name: &str, replies: Value) -> String {
        format!("replay/{}", slash(&self.scratch.write(name, &replies.to_string())))
    }

    /// Writes a config file and returns its path.
    /// @param text - the TOML
    fn config(&self, text: &str) -> String {
        slash(&self.scratch.write("umbra.toml", text))
    }

    /// The config that starts this test binary as a fake MCP server named `fake`.
    /// @param mode - ok or hang
    /// @param startup - the start timeout in seconds
    fn mcp_config(&self, mode: &str, startup: u64) -> String {
        let exe = slash(&std::env::current_exe().unwrap());
        let pid = slash(&self.scratch.path("fake.pid"));
        self.config(&format!("[mcp]\nstartup_timeout_seconds = {startup}\n[mcp_servers.fake]\ncommand = \"{exe}\"\nargs = [\"fake_mcp_server\", \"--exact\", \"--nocapture\", \"--test-threads\", \"1\"]\nenv = {{ UMBRA_FAKE_MCP = \"{mode}\", UMBRA_FAKE_PID_FILE = \"{pid}\" }}\n"))
    }

    /// Starts umbra in the fullscreen view.
    /// @param args - the options after the common ones
    /// @param envs - extra environment variables
    fn start(&self, args: &[&str], envs: &[(&str, &str)]) -> Screen {
        let work = slash(&self.scratch.path("work"));
        let traj = slash(&self.scratch.path("run.traj.json"));
        let mut all = vec!["--cwd", work.as_str(), "-o", traj.as_str()];
        all.extend_from_slice(args);
        Screen::start(&self.scratch.path("home"), &all, envs, (100, 32))
    }

    /// The requests the replay provider recorded.
    fn requests(&self) -> Vec<Value> {
        recorded_requests(&self.scratch.path("requests.jsonl"))
    }

    /// The environment that records requests.
    fn record(&self) -> (String, String) {
        ("UMBRA_REPLAY_RECORD".into(), slash(&self.scratch.path("requests.jsonl")))
    }
}

/// A bash tool call.
/// @param command - the command
fn bash(command: &str) -> Value {
    json!({ "name": "bash", "arguments": { "command": command } })
}

/// True when a row of the frame matches a regex.
/// @param frame - the frame
/// @param pattern - the regex
fn has(frame: &Frame, pattern: &str) -> bool {
    let regex = regex::Regex::new(pattern).unwrap();
    frame.rows.iter().any(|r| regex.is_match(r))
}

#[test]
fn the_header_has_a_row_each_for_model_folder_mode_skills_and_mcp() {
    let setup = Setup::new("fs-header");
    let model = setup.model("script.json", json!(["Hi."]));
    let screen = setup.start(&["-m", &model, "--mode", "confirm", "--no-mcp"], &[]);
    let frame = screen.wait_for("mcp     off");
    for label in ["model   replay/", "folder  ", "skills  0", "mcp     off"] {
        assert!(frame.row_of(label).is_some(), "{label}:\n{}", frame.text());
    }
    assert!(has(&frame, r"mode    confirm\s*│?\s*$") || has(&frame, r"mode    confirm\s+"), "the mode row is the mode word only:\n{}", frame.text());
    assert!(!frame.text().contains("instruction"), "{}", frame.text());
    assert!(frame.text().contains("▾"), "the model row opens a menu:\n{}", frame.text());
}

#[test]
fn a_reply_is_rendered_from_markdown_and_no_done_or_resume_line_is_printed() {
    let setup = Setup::new("fs-reply");
    let model = setup.model("script.json", json!(["The **parser** drops the `last` token."]));
    let mut screen = setup.start(&["-m", &model, "-y", "--no-mcp"], &[]);
    screen.wait_for("❯");
    screen.type_line("fix it");
    let frame = screen.wait_for("● The parser drops the last token.");
    assert!(frame.row_of("❯ fix it").is_some(), "{}", frame.text());
    let status = &frame.rows[frame.rows.len() - 2];
    assert!(has(&frame, r"\d+:\d\d"), "the status bar has the agent time: {status}");
    for gone in ["✔ Done", "--resume", "waiting for you", "left before compaction", "step "] {
        assert!(!frame.text().contains(gone), "{gone}:\n{}", frame.text());
    }
    let (output, code) = screen.finish(Duration::from_secs(10));
    assert_eq!(code, Some(0), "{output}");
    let main_screen = output.rsplit("\x1b[?1049l").next().unwrap();
    assert!(main_screen.contains("trajectory ") && !main_screen.contains("--resume"), "the exit text is the trajectory line only: {main_screen:?}");
    assert!(output.contains("\x1b[?1002l\x1b[?1000l"), "leaving turns mouse reporting off");
}

#[test]
fn commands_in_a_row_are_one_group_that_expands_with_clicks_and_keys() {
    let setup = Setup::new("fs-group");
    let model = setup.model("script.json", json!([
        { "content": "", "tool_calls": [bash("echo alpha"), bash("echo beta"), bash("echo gamma && exit 3")] },
        "Three ran."
    ]));
    let mut screen = setup.start(&["-m", &model, "-y", "--no-mcp"], &[]);
    screen.wait_for("❯");
    screen.type_line("run them");
    let frame = screen.wait_for("● Three ran.");
    assert!(has(&frame, r"▸ ✖ Ran 3 shell commands, 1 failed · \d+:\d\d"), "{}", frame.text());
    assert!(frame.row_of("$ echo alpha").is_none(), "the group starts collapsed:\n{}", frame.text());
    screen.click_text("Ran 3 shell commands");
    let frame = screen.wait_for("$ echo alpha");
    assert!(frame.row_of("$ echo beta").is_some() && has(&frame, r"\$ echo gamma && exit 3.*exit 3"), "{}", frame.text());
    screen.click_text("$ echo beta");
    let frame = screen.wait_for("exit 0 · ");
    assert!(frame.rows.iter().any(|r| r.trim() == "beta"), "the run's output is shown:\n{}", frame.text());
    screen.click_text("Ran 3 shell commands");
    screen.wait_until("the group to collapse", Duration::from_secs(5), |f| f.row_of("$ echo alpha").is_none());
    screen.press("Tab");
    screen.wait_until("focus on the group", Duration::from_secs(5), |f| f.rows.iter().any(|r| r.starts_with('›') && r.contains("Ran 3")));
    screen.press("Enter");
    screen.wait_for("$ echo alpha");
}

#[test]
fn a_reply_arrives_while_an_mcp_server_never_answers_and_the_failure_is_shown() {
    let setup = Setup::new("fs-mcp-hang");
    let config = setup.mcp_config("hang", 3);
    let model = setup.model("script.json", json!(["Hello back."]));
    let mut screen = setup.start(&["-m", &model, "-y", "-c", &config], &[]);
    screen.wait_for("starting");
    screen.type_line("hello");
    let sent = Instant::now();
    screen.wait_for("● Hello back.");
    assert!(sent.elapsed() < Duration::from_secs(2), "the reply did not wait for the server: {:?}", sent.elapsed());
    let frame = screen.wait_until("the failure", Duration::from_secs(10), |f| f.text().contains("1 failed") && f.text().contains("▲ 1 MCP error"));
    assert!(frame.row_of("mcp     0 running · 1 failed").is_some(), "{}", frame.text());
    assert!(!frame.text().contains("was not started"), "no yellow line in the fullscreen view:\n{}", frame.text());
    screen.click_text("▲ 1 MCP error");
    let frame = screen.wait_for("within 3 seconds");
    assert!(frame.text().contains("log "), "{}", frame.text());
    screen.click_text("▲ 1 MCP error");
    screen.wait_until("the error list to close", Duration::from_secs(5), |f| !f.text().contains("within 3 seconds"));
    let pid: u32 = std::fs::read_to_string(setup.scratch.path("fake.pid")).unwrap().trim().parse().unwrap();
    assert!(wait_for_exit(pid), "the server that missed its start was stopped");
}

#[test]
fn a_server_turned_off_in_the_panel_has_no_tools_in_the_next_request() {
    let setup = Setup::new("fs-mcp-toggle");
    let config = setup.mcp_config("ok", 20);
    let model = setup.model("script.json", json!(["one", "two", "three"]));
    let (key, value) = setup.record();
    let mut screen = setup.start(&["-m", &model, "-y", "-c", &config], &[(&key, &value)]);
    screen.wait_for("mcp     1 running");
    screen.type_line("first");
    screen.wait_for("● one");
    screen.type_line("/mcp");
    screen.wait_for("MCP servers");
    screen.press("Space");
    screen.wait_for("mcp     0 running · 1 off");
    screen.press("Esc");
    screen.type_line("second");
    screen.wait_for("● two");
    let pid: u32 = std::fs::read_to_string(setup.scratch.path("fake.pid")).unwrap().trim().parse().unwrap();
    assert!(wait_for_exit(pid), "turning the server off stopped its process");
    screen.type_line("/mcp");
    screen.wait_for("MCP servers");
    screen.press("Space");
    screen.wait_for("mcp     1 running");
    screen.press("Esc");
    screen.type_line("third");
    screen.wait_for("● three");
    let tools: Vec<Vec<String>> = setup.requests().iter().map(|r| r["tools"].as_array().unwrap().iter().map(|t| t.as_str().unwrap().to_string()).collect()).collect();
    assert!(tools[0].contains(&"mcp__fake__echo".to_string()), "{tools:?}");
    assert!(!tools[1].iter().any(|t| t.starts_with("mcp__fake")), "{tools:?}");
    assert!(tools[2].contains(&"mcp__fake__echo".to_string()), "{tools:?}");
}

#[test]
fn the_agent_time_counts_only_while_the_agent_works() {
    let setup = Setup::new("fs-clock");
    let model = setup.model("script.json", json!([{ "content": "first", "sleep_ms": 1300 }, { "content": "second", "sleep_ms": 1300 }]));
    let mut screen = setup.start(&["-m", &model, "-y", "--no-mcp"], &[]);
    screen.wait_for("❯");
    screen.type_line("go");
    let frame = screen.wait_for("● first");
    let first = agent_time(&frame);
    assert!(first >= 1, "{}", frame.text());
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(agent_time(&screen.frame()), first, "the time does not move while the prompt waits");
    screen.type_line("again");
    let frame = screen.wait_for("● second");
    assert!(agent_time(&frame) >= first + 1, "the time continues from the same value: {}", frame.text());
}

#[test]
fn typing_a_slash_lists_the_commands_and_tab_completes() {
    let setup = Setup::new("fs-popup");
    setup.scratch.write("work/.claude/commands/my-command.md", "---\ndescription: Say the body\n---\nThe body with $ARGUMENTS.\n");
    let model = setup.model("script.json", json!(["one", "two"]));
    let (key, value) = setup.record();
    let mut screen = setup.start(&["-m", &model, "-y", "--no-mcp"], &[(&key, &value)]);
    screen.wait_for("❯");
    screen.type_text("/co");
    let frame = screen.wait_for("/compact");
    assert!(frame.row_of("/config").is_some(), "{}", frame.text());
    screen.press("Tab");
    screen.wait_until("the completed name", Duration::from_secs(5), |f| f.input().contains("❯ /config "));
    for _ in 0.."/config ".len() {
        screen.press("Backspace");
    }
    screen.type_text("/my");
    let frame = screen.wait_for("/my-command");
    assert!(has(&frame, r"/my-command\s+custom\s+Say the body"), "{}", frame.text());
    screen.press("Esc");
    for _ in 0..3 {
        screen.press("Backspace");
    }
    screen.type_line("hello");
    screen.wait_for("● one");
    screen.type_line("/my-command x");
    screen.wait_for("● two");
    let requests = setup.requests();
    let last = requests[1]["messages"].as_array().unwrap().last().unwrap()["content"].as_str().unwrap().to_string();
    assert!(last.contains("The body with x."), "a custom command works on a follow up prompt: {last}");
}

#[test]
fn a_drag_selects_text_in_reverse_video_and_copies_it() {
    let setup = Setup::new("fs-select");
    let model = setup.model("script.json", json!(["alpha beta gamma delta"]));
    let clipboard = slash(&setup.scratch.path("clipboard.txt"));
    let mut screen = setup.start(&["-m", &model, "-y", "--no-mcp"], &[("UMBRA_CLIPBOARD_FILE", &clipboard)]);
    screen.wait_for("❯");
    screen.type_line("say it");
    let frame = screen.wait_for("● alpha beta gamma delta");
    let (col, row) = frame.position_of("beta").unwrap();
    screen.click(col, row);
    assert!(screen.frame().reversed().is_empty(), "a click selects nothing");
    screen.drag((col, row), (col + 9, row));
    let frame = screen.wait_until("the highlight", Duration::from_secs(5), |f| !f.reversed().is_empty());
    assert_eq!(frame.reversed(), vec!["beta gamma"]);
    let copied = std::fs::read_to_string(&clipboard).unwrap();
    assert_eq!(copied, "beta gamma");
    assert!(screen.output().contains("\x1b]52;c;YmV0YSBnYW1tYQ==\x07"), "OSC 52 carries the text");
    screen.wait_for("copied 10 characters");
}

#[test]
fn with_the_mouse_off_no_mouse_mode_is_sent_and_the_keyboard_still_expands_a_group() {
    let setup = Setup::new("fs-no-mouse");
    let model = setup.model("script.json", json!([{ "content": "", "tool_calls": [bash("echo one")] }, "Done here."]));
    let mut screen = setup.start(&["-m", &model, "-y", "--no-mcp", "--no-mouse"], &[]);
    screen.wait_for("❯");
    screen.type_line("go");
    screen.wait_for("● Done here.");
    assert!(!screen.output().contains("\x1b[?1002h"), "no mouse mode is turned on");
    screen.press("Tab");
    screen.press("Enter");
    screen.wait_for("$ echo one");
}

/// The agent time in the status bar, in seconds.
/// @param frame - the frame
fn agent_time(frame: &Frame) -> u64 {
    let status = &frame.rows[frame.rows.len() - 2];
    let found = regex::Regex::new(r"(\d+):(\d\d)").unwrap().captures_iter(status).last().unwrap_or_else(|| panic!("no time in {status}"));
    found[1].parse::<u64>().unwrap() * 60 + found[2].parse::<u64>().unwrap()
}

/// Waits up to 5 seconds for a process to exit.
/// @param pid - the process id
fn wait_for_exit(pid: u32) -> bool {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        if !process_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// True while a process runs.
/// @param pid - the process id
#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    let output = std::process::Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output().unwrap();
    String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
}

/// True while a process runs.
/// @param pid - the process id
#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    std::process::Command::new("kill").args(["-0", &pid.to_string()]).status().is_ok_and(|s| s.success())
}
