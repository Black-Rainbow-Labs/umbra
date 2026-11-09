//! Ctrl+C. Each test starts the real binary in its own process group and sends it the console
//! event Ctrl+C sends (Ctrl+Break on Windows, which cannot send Ctrl+C to one group; SIGINT on
//! Unix). A machine that cannot send one skips the test and says so.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use common::{read_trajectory, replay_model, slash, text, Running, Scratch};
use serde_json::{json, Value};

/// A reply that runs one command.
/// @param command - the command
fn call(command: &str) -> Value {
    json!({ "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": command } }] })
}

/// A reply that submits with the given output.
/// @param answer - the submission
fn submit(answer: &str) -> Value {
    call(&format!("echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT && echo {answer}"))
}

/// One interrupt test: a scratch folder, a working folder and a trajectory path.
struct Case {
    scratch: Scratch,
}

impl Case {
    /// Creates the folders.
    /// @param name - the test name
    fn new(name: &str) -> Self {
        let scratch = Scratch::new(name);
        std::fs::create_dir_all(scratch.path("work")).unwrap();
        Case { scratch }
    }

    /// Starts umbra on a replay script.
    /// @param replies - the scripted replies
    /// @param extra - more arguments, before the task
    /// @param interactive - whether a piped stdin counts as a terminal
    fn start(&self, replies: Value, extra: &[&str], interactive: bool) -> Running {
        let model = replay_model(&self.scratch, "script.json", &replies);
        let (work, traj) = (slash(&self.scratch.path("work")), slash(&self.trajectory_path()));
        let mut args = vec!["-m", model.as_str(), "--no-mcp", "--cwd", work.as_str(), "-o", traj.as_str(), "--exit-immediately"];
        args.extend(extra);
        args.push("task");
        let envs: &[(&str, &str)] = if interactive { &[("UMBRA_FORCE_INTERACTIVE", "1")] } else { &[] };
        Running::start(&self.scratch.path("home"), &args, envs)
    }

    /// The trajectory file.
    fn trajectory_path(&self) -> std::path::PathBuf {
        self.scratch.path("run.traj.json")
    }

    /// The messages of the trajectory.
    fn messages(&self) -> Vec<Value> {
        read_trajectory(&self.trajectory_path())["messages"].as_array().unwrap().clone()
    }
}

/// Sends Ctrl+C, or returns false after saying the test is skipped. Waits half a second after
/// sending, because the event reaches umbra on its own thread and a line typed straight after it
/// could otherwise arrive first, which a person at a keyboard cannot do.
/// @param running - the running umbra
fn press_ctrl_c(running: &Running) -> bool {
    let sent = running.ctrl_c();
    if !sent {
        eprintln!("skipped: this machine cannot send a console event");
    }
    std::thread::sleep(Duration::from_millis(500));
    sent
}

#[test]
fn with_no_terminal_ctrl_c_ends_the_run_and_kills_the_command() {
    let case = Case::new("interrupt-batch");
    let running = case.start(json!([call("sleep 30 && echo finished > finished.txt"), submit("never")]), &["-y"], false);
    std::thread::sleep(Duration::from_secs(2));
    if !press_ctrl_c(&running) {
        return;
    }
    let sent = Instant::now();
    let output = running.finish(Duration::from_secs(15));
    assert_eq!(output.status.code(), Some(130), "{}", text(&output));
    assert!(sent.elapsed() < Duration::from_secs(8), "stopped in {:?}:
{}", sent.elapsed(), text(&output));
    let trajectory = read_trajectory(&case.trajectory_path());
    assert_eq!(trajectory["info"]["exit_status"], "Interrupted");
    let messages = case.messages();
    assert!(messages[3]["extra"]["exception_info"].as_str().unwrap().starts_with("The user interrupted this command (Ctrl+C) after"), "{}", messages[3]);
    std::thread::sleep(Duration::from_secs(1));
    assert!(!case.scratch.path("work/finished.txt").exists(), "the shell was killed, so the rest of the command never ran");
    assert!(text(&output).contains("umbra --resume"), "{}", text(&output));
}

#[test]
fn the_comment_goes_to_the_model_and_the_run_continues() {
    let case = Case::new("interrupt-comment");
    let mut running = case.start(json!([call("sleep 30"), submit("after")]), &["-y"], true);
    std::thread::sleep(Duration::from_secs(2));
    if !press_ctrl_c(&running) {
        return;
    }
    running.type_line("use a shorter sleep");
    let output = running.finish(Duration::from_secs(15));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(text(&output).contains("Interrupted. Type a comment/command (/h for available commands)"), "{}", text(&output));
    let messages = case.messages();
    let roles: Vec<&str> = messages.iter().map(|m| m["role"].as_str().unwrap()).collect();
    assert_eq!(roles, ["system", "user", "assistant", "tool", "user", "assistant", "tool", "exit"]);
    assert_eq!(messages[4]["content"], "Interrupted by user: use a shorter sleep");
    assert_eq!(messages[4]["extra"]["interrupt_type"], "UserInterruption");
}

#[test]
fn an_empty_comment_sends_the_temporary_message() {
    let case = Case::new("interrupt-empty");
    let mut running = case.start(json!([call("sleep 30"), submit("after")]), &["-y"], true);
    std::thread::sleep(Duration::from_secs(2));
    if !press_ctrl_c(&running) {
        return;
    }
    running.type_line("");
    let output = running.finish(Duration::from_secs(15));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(case.messages()[4]["content"], "Interrupted by user: Temporary interruption caught.");
}

#[test]
fn a_second_ctrl_c_at_the_comment_prompt_ends_the_run() {
    let case = Case::new("interrupt-twice");
    let running = case.start(json!([call("sleep 30"), submit("never")]), &["-y"], true);
    std::thread::sleep(Duration::from_secs(2));
    if !press_ctrl_c(&running) {
        return;
    }
    std::thread::sleep(Duration::from_millis(1500));
    assert!(running.ctrl_c());
    let output = running.finish(Duration::from_secs(15));
    assert_eq!(output.status.code(), Some(130), "{}", text(&output));
    assert_eq!(read_trajectory(&case.trajectory_path())["info"]["exit_status"], "Interrupted");
}

#[test]
fn ctrl_c_while_the_model_is_answering_drops_the_reply() {
    let case = Case::new("interrupt-model");
    let slow = json!({ "sleep_ms": 60000, "content": "too late", "tool_calls": [] });
    let mut running = case.start(json!([slow, submit("after")]), &["-y"], true);
    std::thread::sleep(Duration::from_secs(2));
    if !press_ctrl_c(&running) {
        return;
    }
    running.type_line("answer faster");
    let output = running.finish(Duration::from_secs(15));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let messages = case.messages();
    let roles: Vec<&str> = messages.iter().map(|m| m["role"].as_str().unwrap()).collect();
    assert_eq!(roles, ["system", "user", "user", "assistant", "tool", "exit"], "no assistant message for the interrupted call");
    assert_eq!(messages[2]["content"], "Interrupted by user: answer faster");
}

#[test]
fn ctrl_c_at_the_confirm_prompt_skips_the_actions_and_mode_commands_work() {
    let case = Case::new("interrupt-confirm");
    let mut running = case.start(json!([call("echo should not run > ran.txt"), submit("after")]), &[], true);
    std::thread::sleep(Duration::from_secs(2));
    if !press_ctrl_c(&running) {
        return;
    }
    running.type_line("/y");
    let output = running.finish(Duration::from_secs(15));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(text(&output).contains("Switched to yolo mode."), "{}", text(&output));
    let messages = case.messages();
    assert_eq!(messages[3]["content"].as_str().map(|c| c.contains("because the user interrupted the run")), Some(true), "{}", messages[3]);
    assert_eq!(messages[4]["content"], "Interrupted by user: Temporary interruption caught.");
    assert!(!case.scratch.path("work/ran.txt").exists());
}

/// Serves one streaming Chat Completions reply that never ends: a chunk every 100 ms until the
/// client goes away. Sends the time it noticed on the channel. Other requests get a 404.
/// @param listener - the bound socket
/// @param closed - where to report the disconnect
fn serve_endless_stream(listener: TcpListener, closed: mpsc::Sender<Instant>) {
    for stream in listener.incoming().flatten() {
        let Some(is_chat) = read_request(&stream) else { continue };
        let mut stream = stream;
        if !is_chat {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            continue;
        }
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n");
        let chunk = "data: {\"choices\":[{\"delta\":{\"content\":\"word \"},\"finish_reason\":null}]}\n\n";
        while stream.write_all(chunk.as_bytes()).and_then(|_| stream.flush()).is_ok() {
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = closed.send(Instant::now());
        return;
    }
}

/// Reads a request's head and body. Returns whether it was the chat completions POST.
/// @param stream - the connection
fn read_request(stream: &TcpStream) -> Option<bool> {
    let mut reader = BufReader::new(stream);
    let mut first = String::new();
    reader.read_line(&mut first).ok()?;
    let mut length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        if line.trim().is_empty() {
            break;
        }
        if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(first.starts_with("POST") && first.contains("/chat/completions"))
}

#[test]
fn ctrl_c_closes_a_streaming_connection_so_the_server_stops() {
    let case = Case::new("interrupt-stream");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, closed) = mpsc::channel();
    std::thread::spawn(move || serve_endless_stream(listener, sender));
    let config = case.scratch.write("fake.toml", &format!("[providers.fake]\nkind = \"openai\"\nbase_url = \"http://127.0.0.1:{port}/v1\"\nmax_retries = 0\n"));
    let (work, traj) = (slash(&case.scratch.path("work")), slash(&case.trajectory_path()));
    let args = ["-m", "fake/endless", "-c", &slash(&config), "-y", "--no-mcp", "--no-compact", "--cwd", &work, "-o", &traj, "task"];
    let running = Running::start(&case.scratch.path("home"), &args, &[]);
    std::thread::sleep(Duration::from_secs(3));
    if !press_ctrl_c(&running) {
        return;
    }
    let pressed = Instant::now();
    let output = running.finish(Duration::from_secs(15));
    assert_eq!(output.status.code(), Some(130), "{}", text(&output));
    let noticed = closed.recv_timeout(Duration::from_secs(10)).expect("the server saw the connection close");
    assert!(noticed.duration_since(pressed) < Duration::from_secs(5), "the server noticed {:?} after Ctrl+C", noticed.duration_since(pressed));
}
