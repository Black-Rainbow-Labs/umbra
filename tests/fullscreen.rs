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

impl Setup {
    /// Starts umbra in the fullscreen view with exactly these arguments.
    /// @param args - the whole command line
    /// @param envs - extra environment variables
    fn start_raw(&self, args: &[&str], envs: &[(&str, &str)]) -> Screen {
        Screen::start(&self.scratch.path("home"), args, envs, (100, 32))
    }

    /// The work folder, with forward slashes.
    fn work(&self) -> String {
        slash(&self.scratch.path("work"))
    }

    /// Runs one session over a pipe, so it is saved in the session index.
    /// @param name - the script file name
    /// @param reply - the reply
    /// @param task - the task
    fn saved_session(&self, name: &str, reply: &str, task: &str) {
        let model = self.model(name, json!([reply]));
        let work = self.work();
        let output = common::run_umbra(&self.scratch.path("home"), &["-m", &model, "-y", "--no-mcp", "--cwd", &work, task]);
        assert_eq!(output.status.code(), Some(0), "{}", common::text(&output));
        std::thread::sleep(Duration::from_millis(30));
    }

    /// The session index entries in the fake home, newest first.
    fn index(&self) -> Vec<Value> {
        let dir = self.scratch.path("home/.umbra/sessions");
        let mut entries: Vec<Value> = std::fs::read_dir(dir).map(|d| d.flatten().filter_map(|e| std::fs::read_to_string(e.path()).ok()).filter_map(|t| serde_json::from_str(&t).ok()).collect()).unwrap_or_default();
        entries.sort_by(|a: &Value, b: &Value| b["updated_at"].as_f64().unwrap_or(0.0).total_cmp(&a["updated_at"].as_f64().unwrap_or(0.0)));
        entries
    }
}

#[test]
fn the_model_picker_switches_the_model_for_the_next_request() {
    let setup = Setup::new("fs-model");
    let first = slash(&setup.scratch.write("first.json", &json!(["from the first model"]).to_string()));
    let second = slash(&setup.scratch.write("second.json", &json!(["from the second model"]).to_string()));
    let config = setup.config(&format!("[models.first]\nprovider = \"replay\"\nmodel = \"{first}\"\n[models.second]\nprovider = \"replay\"\nmodel = \"{second}\"\n[models.broken]\nprovider = \"anthropic\"\nmodel = \"claude-sonnet-5-5\"\n"));
    let mut screen = setup.start(&["-m", "first", "-y", "--no-mcp", "-c", &config], &[]);
    screen.wait_for("❯");
    screen.type_line("/model");
    let frame = screen.wait_for("Other...");
    assert!(has(&frame, r"● first") && has(&frame, r"○ second") && has(&frame, r"○ broken"), "{}", frame.text());
    screen.type_text("broken");
    screen.press("Enter");
    let frame = screen.wait_for("no Anthropic credential");
    assert!(frame.text().contains("first.json"), "the model did not change:\n{}", frame.text());
    for _ in 0.."broken".len() {
        screen.press("Backspace");
    }
    screen.type_text("second");
    screen.press("Enter");
    screen.wait_until("the picker to close", Duration::from_secs(5), |f| f.row_of("Other...").is_none());
    screen.type_line("hello");
    screen.wait_for("● from the second model");
}

#[test]
fn clear_starts_a_new_session_and_keeps_the_old_one() {
    let setup = Setup::new("fs-clear");
    let model = setup.model("script.json", json!(["first answer"]));
    let mut screen = setup.start_raw(&["-m", &model, "-y", "--no-mcp", "--cwd", &setup.work()], &[]);
    screen.wait_for("❯");
    screen.type_line("first question");
    screen.wait_for("● first answer");
    screen.type_line("/clear");
    screen.wait_until("an empty transcript", Duration::from_secs(10), |f| f.row_of("first answer").is_none() && f.row_of("mcp     off").is_some());
    screen.type_line("second question");
    let frame = screen.wait_for("● first answer");
    assert!(frame.row_of("❯ second question").is_some() && frame.row_of("❯ first question").is_none(), "the new session starts its own script again:\n{}", frame.text());
    let (output, code) = screen.finish(Duration::from_secs(10));
    assert_eq!(code, Some(0), "{output}");
    let index = setup.index();
    assert_eq!(index.len(), 2, "both sessions are listed: {index:#?}");
    assert_eq!(index[0]["title"], "second question");
    assert_eq!(index[1]["title"], "first question");
}

#[test]
fn umbra_resume_lists_sessions_with_titles_and_rebuilds_the_chosen_one() {
    let setup = Setup::new("fs-resume");
    setup.saved_session("one.json", "answer one", "task one");
    setup.saved_session("two.json", "answer two", "task two");
    setup.saved_session("three.json", "answer three", "task three");
    let output = common::run_umbra(&setup.scratch.path("home"), &["resume", "--cwd", &setup.work()]);
    assert_eq!(output.status.code(), Some(2), "over a pipe an id is needed: {}", common::text(&output));
    assert!(common::text(&output).contains("task two"), "{}", common::text(&output));
    let model = setup.model("next.json", json!(["after the resume"]));
    let work = setup.work();
    let mut screen = setup.start_raw(&["resume", "--cwd", &work, "-m", &model, "-y", "--no-mcp"], &[]);
    let frame = screen.wait_for("Resume a session");
    let three = frame.row_of("task three").unwrap();
    assert!(three < frame.row_of("task two").unwrap() && frame.row_of("task two").unwrap() < frame.row_of("task one").unwrap(), "newest first:\n{}", frame.text());
    assert!(has(&frame, r"task three .*1 call"), "{}", frame.text());
    screen.press("Down");
    screen.press("Enter");
    let frame = screen.wait_for("● answer two");
    assert!(frame.row_of("❯ task two").is_some() && frame.row_of("resuming session").is_some(), "{}", frame.text());
    assert!(frame.row_of("answer three").is_none(), "{}", frame.text());
    screen.type_line("carry on");
    screen.wait_for("● after the resume");
}

#[test]
fn a_session_gets_a_title_and_summary_from_the_model_after_its_first_turn() {
    let setup = Setup::new("fs-title");
    let title = json!({ "title_reply": "{\"title\": \"Greeting the user warmly today\", \"summary\": \"The user said hello.\"}" });
    let model = setup.model("script.json", json!(["Hello there.", title]));
    let mut screen = setup.start(&["-m", &model, "-y", "--no-mcp"], &[]);
    screen.wait_for("❯");
    screen.type_line("say hi");
    screen.wait_for("● Hello there.");
    let (output, code) = screen.finish(Duration::from_secs(10));
    assert_eq!(code, Some(0), "{output}");
    let index = setup.index();
    assert_eq!(index[0]["title"], "Greeting the user warmly today");
    assert_eq!(index[0]["summary"], "The user said hello.");
    let setup = Setup::new("fs-title-off");
    let config = setup.config("[sessions]\nauto_title = false\n");
    let model = setup.model("script.json", json!(["Hello there.", title]));
    let mut screen = setup.start(&["-m", &model, "-y", "--no-mcp", "-c", &config], &[]);
    screen.wait_for("❯");
    screen.type_line("say hi");
    screen.wait_for("● Hello there.");
    screen.finish(Duration::from_secs(10));
    assert_eq!(setup.index()[0]["title"], "say hi", "with auto_title off the title is the first prompt");
}

#[test]
fn the_config_modal_saves_a_change_and_keeps_the_comments() {
    let setup = Setup::new("fs-config");
    let user = setup.scratch.write("home/.umbra/config.toml", "# my settings\nstep_limit = 0\n");
    let model = setup.model("script.json", json!(["ok"]));
    let mut screen = setup.start(&["-m", &model, "-y", "--no-mcp"], &[]);
    screen.wait_for("❯");
    screen.type_line("/config");
    screen.wait_for("Configure Umbra");
    screen.press("Down");
    screen.press("Down");
    screen.press("Tab");
    screen.wait_for("Context window");
    screen.press("Down");
    screen.press("Enter");
    screen.type_text("abc");
    screen.press("Enter");
    screen.wait_for("Enter a whole number.");
    screen.press("Esc");
    screen.press("Down");
    screen.press("Enter");
    screen.send(b"\x15");
    screen.type_text("70");
    screen.press("Enter");
    let frame = screen.wait_for("changed");
    assert!(has(&frame, r"Trigger\s+70%"), "{}", frame.text());
    screen.send(b"\x13");
    screen.wait_for("Saved to");
    let text = std::fs::read_to_string(&user).unwrap();
    assert!(text.contains("# my settings") && text.contains("step_limit = 0"), "the comment and the other keys are kept:\n{text}");
    assert!(text.contains("trigger_ratio = 0.7"), "{text}");
    screen.press("Esc");
    screen.wait_until("the modal to close", Duration::from_secs(5), |f| f.row_of("Configure Umbra").is_none());
}

/// The paths a fake HTTP server saw.
type Seen = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

/// A local HTTP server for a test. Each request's path is recorded, and the reply comes from a
/// function of the path. Returns the address and the recorded paths.
/// @param reply - the status and body for a path
fn http_server(reply: impl Fn(&str) -> (u16, String) + Send + 'static) -> (String, Seen) {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let paths: Seen = Default::default();
    let seen = paths.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut data = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                let n = stream.read(&mut buffer).unwrap_or(0);
                data.extend_from_slice(&buffer[..n]);
                let text = String::from_utf8_lossy(&data).to_string();
                let done = match text.find("\r\n\r\n") {
                    Some(end) => {
                        let length = text.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                        data.len() >= end + 4 + length
                    }
                    None => false,
                };
                if done || n == 0 {
                    break;
                }
            }
            let text = String::from_utf8_lossy(&data).to_string();
            let path = text.split_whitespace().nth(1).unwrap_or("/").to_string();
            seen.lock().unwrap().push(path.clone());
            let (status, body) = reply(&path);
            let response = format!("HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (address, paths)
}

/// A reply from the Anthropic messages endpoint.
fn claude_reply() -> String {
    json!({ "id": "m", "type": "message", "role": "assistant", "model": "claude-sonnet-5-5", "content": [{ "type": "text", "text": "Hi from Claude." }], "stop_reason": "end_turn", "usage": { "input_tokens": 10, "output_tokens": 3 } }).to_string()
}

/// Starts a session on a fake Claude login or API key, with a usage endpoint that answers
/// `usage`. Returns the screen and the paths the server saw.
/// @param setup - the test folder
/// @param usage - the usage endpoint's status and body
/// @param token - true to sign in with a login token, false with an API key
fn claude_session(setup: &Setup, usage: (u16, String), token: bool) -> (Screen, Seen) {
    let (address, paths) = http_server(move |path| if path.starts_with("/api/oauth/usage") { usage.clone() } else { (200, claude_reply()) });
    let config = setup.config(&format!("[providers.claude]\nkind = \"anthropic\"\nbase_url = \"{address}\"\napi_key_env = \"UMBRA_TEST_CLAUDE_KEY\"\n"));
    let usage_url = format!("{address}/api/oauth/usage");
    let mut envs = vec![("UMBRA_TEST_USAGE_URL", usage_url.as_str())];
    if token {
        envs.push(("ANTHROPIC_AUTH_TOKEN", "test-login-token"));
    } else {
        envs.push(("UMBRA_TEST_CLAUDE_KEY", "sk-test"));
    }
    let screen = setup.start(&["-m", "claude/claude-sonnet-5-5", "-y", "--no-mcp", "-c", &config], &envs);
    (screen, paths)
}

#[test]
fn the_weekly_usage_bar_follows_the_claude_usage_endpoint() {
    let setup = Setup::new("fs-usage");
    let body = json!({ "five_hour": { "utilization": 12.0, "resets_at": "2026-11-10T20:00:00Z" }, "seven_day": { "utilization": 23.0, "resets_at": "2099-11-14T08:00:00Z" } }).to_string();
    let (mut screen, paths) = claude_session(&setup, (200, body), true);
    screen.wait_until("the usage bar", Duration::from_secs(10), |f| f.rows.iter().any(|r| r.contains("week") && r.contains("23%")));
    screen.type_line("hello");
    screen.wait_for("● Hi from Claude.");
    screen.click_text("week");
    screen.wait_for("resets in");
    assert_eq!(paths.lock().unwrap().iter().filter(|p| p.starts_with("/api/oauth/usage")).count(), 1, "polled once, not again within 180 seconds");
    for (status, body) in [(401, "{}".to_string()), (200, json!({ "seven_day": null }).to_string())] {
        let setup = Setup::new("fs-usage-hidden");
        let (screen, paths) = claude_session(&setup, (status, body), true);
        screen.wait_for("mcp     off");
        let started = Instant::now();
        while paths.lock().unwrap().is_empty() && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(50));
        }
        std::thread::sleep(Duration::from_millis(300));
        assert!(!paths.lock().unwrap().is_empty(), "the endpoint was polled");
        assert!(!screen.frame().text().contains("week"), "status {status} hides the bar:\n{}", screen.frame().text());
    }
    let setup = Setup::new("fs-usage-key");
    let (mut screen, paths) = claude_session(&setup, (200, "{}".into()), false);
    screen.wait_for("❯");
    screen.type_line("hello");
    screen.wait_for("● Hi from Claude.");
    assert!(!paths.lock().unwrap().iter().any(|p| p.starts_with("/api/oauth/usage")), "an API key never polls the usage endpoint");
}

#[test]
fn durations_are_saved_and_an_old_trajectory_without_them_still_resumes() {
    let setup = Setup::new("fs-durations");
    let model = setup.model("script.json", json!([{ "content": "", "tool_calls": [bash("echo hi")] }, "done"]));
    let work = setup.work();
    let traj = slash(&setup.scratch.path("run.traj.json"));
    let output = common::run_umbra(&setup.scratch.path("home"), &["-m", &model, "-y", "--no-mcp", "--cwd", &work, "-o", &traj, "go"]);
    assert_eq!(output.status.code(), Some(0), "{}", common::text(&output));
    let mut trajectory = common::read_trajectory(&setup.scratch.path("run.traj.json"));
    let tool = trajectory["messages"].as_array().unwrap().iter().find(|m| m["role"] == "tool").unwrap().clone();
    assert!(tool["extra"]["duration_ms"].as_u64().is_some(), "{tool}");
    for message in trajectory["messages"].as_array_mut().unwrap() {
        if let Some(extra) = message["extra"].as_object_mut() {
            extra.remove("duration_ms");
            extra.remove("started_at");
        }
    }
    std::fs::write(setup.scratch.path("run.traj.json"), trajectory.to_string()).unwrap();
    let next = setup.model("next.json", json!(["resumed fine"]));
    let mut screen = setup.start_raw(&["-m", &next, "-y", "--no-mcp", "--cwd", &work, "--resume", &traj], &[]);
    let frame = screen.wait_for("Ran 1 shell command");
    assert!(frame.row_of("❯ go").is_some(), "{}", frame.text());
    screen.type_line("again");
    screen.wait_for("● resumed fine");
}
