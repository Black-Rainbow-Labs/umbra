//! Runs the real binary against scripted model replies and checks what the agent loop did: the
//! commands it ran, how it ended, and the trajectory it wrote.

mod common;

use common::{read_trajectory, recorded_requests, run_umbra, run_umbra_with, slash, text, Running, Scratch};
use serde_json::json;

/// Writes a replay script and returns the `-m` value for it.
/// @param scratch - the test folder
/// @param replies - the scripted replies
fn script(scratch: &Scratch, replies: serde_json::Value) -> String {
    let path = scratch.write("script.json", &replies.to_string());
    format!("replay/{}", slash(&path))
}

#[test]
fn tool_mode_runs_commands_and_submits() {
    let scratch = Scratch::new("submit");
    let work = scratch.path("work");
    std::fs::create_dir_all(&work).unwrap();
    let model = script(&scratch, json!([
        { "content": "Writing the file.", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo hello > out.txt && cat out.txt" } }] },
        { "content": "Done.", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT && echo the answer" } }] }
    ]));
    let traj = scratch.path("run.traj.json");
    let output = run_umbra(&scratch.path("home"), &["-m", &model, "-y", "--no-mcp", "--cwd", &slash(&work), "-o", &slash(&traj), "write a file"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(std::fs::read_to_string(work.join("out.txt")).unwrap().trim(), "hello");
    let trajectory = read_trajectory(&traj);
    assert_eq!(trajectory["info"]["exit_status"], "Submitted");
    assert_eq!(trajectory["info"]["submission"].as_str().unwrap().trim(), "the answer");
    assert_eq!(trajectory["trajectory_format"], "mini-swe-agent-1.1");
    let roles: Vec<&str> = trajectory["messages"].as_array().unwrap().iter().map(|m| m["role"].as_str().unwrap()).collect();
    assert_eq!(roles, vec!["system", "user", "assistant", "tool", "assistant", "tool", "exit"]);
}

#[test]
fn text_mode_recovers_from_a_format_error() {
    let scratch = Scratch::new("format");
    let model = script(&scratch, json!([
        "I forgot the code block.",
        "THOUGHT: now properly.\n\n```umbra_bash\necho COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT\n```"
    ]));
    let traj = scratch.path("run.traj.json");
    let output = run_umbra(&scratch.path("home"), &["-m", &model, "--text", "--finish", "submit", "-y", "--no-mcp", "--cwd", &slash(&scratch.root), "-o", &slash(&traj), "task"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let trajectory = read_trajectory(&traj);
    let errors: Vec<_> = trajectory["messages"].as_array().unwrap().iter().filter(|m| m["extra"]["interrupt_type"] == "FormatError").collect();
    assert_eq!(errors.len(), 1);
    assert!(errors[0]["content"].as_str().unwrap().contains("Expected exactly 1 action, found 0."));
}

#[test]
fn three_format_errors_in_a_row_end_the_run() {
    let scratch = Scratch::new("repeated");
    let model = script(&scratch, json!(["no action", "still none", "nothing"]));
    let traj = scratch.path("run.traj.json");
    let output = run_umbra(&scratch.path("home"), &["-m", &model, "--text", "--finish", "submit", "-y", "--no-mcp", "--cwd", &slash(&scratch.root), "-o", &slash(&traj), "task"]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    assert_eq!(read_trajectory(&traj)["info"]["exit_status"], "RepeatedFormatError");
}

#[test]
fn a_greeting_gets_one_reply_and_the_turn_ends() {
    let scratch = Scratch::new("hello");
    let model = script(&scratch, json!(["Hi! What would you like to work on?"]));
    let traj = scratch.path("run.traj.json");
    let output = run_umbra(&scratch.path("home"), &["-m", &model, "-y", "--no-mcp", "-q", "--cwd", &slash(&scratch.root), "-o", &slash(&traj), "Hello"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Hi! What would you like to work on?", "--quiet prints the reply");
    let trajectory = read_trajectory(&traj);
    assert_eq!(trajectory["info"]["exit_status"], "Replied");
    assert_eq!(trajectory["info"]["model_stats"]["api_calls"], 1);
    let messages = trajectory["messages"].as_array().unwrap();
    assert_eq!(messages[1]["content"], "Hello", "the person's words reach the model as they are");
    assert!(!messages[0]["content"].as_str().unwrap().contains("COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT"));
    let roles: Vec<&str> = messages.iter().map(|m| m["role"].as_str().unwrap()).collect();
    assert_eq!(roles, vec!["system", "user", "assistant", "exit"]);
}

#[test]
fn the_submit_style_still_rejects_a_reply_without_a_command() {
    let scratch = Scratch::new("hello-submit");
    let model = script(&scratch, json!(["Hi!", "Hi!", "Hi!"]));
    let traj = scratch.path("run.traj.json");
    let output = run_umbra(&scratch.path("home"), &["-m", &model, "--finish", "submit", "-y", "--no-mcp", "--cwd", &slash(&scratch.root), "-o", &slash(&traj), "Hello"]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    let trajectory = read_trajectory(&traj);
    assert_eq!(trajectory["info"]["exit_status"], "RepeatedFormatError");
    assert!(trajectory["messages"][1]["content"].as_str().unwrap().starts_with("Please solve this issue: Hello"));
}

#[test]
fn a_follow_up_continues_the_conversation_after_a_reply() {
    let scratch = Scratch::new("follow-up");
    let record = scratch.path("requests.jsonl");
    let model = script(&scratch, json!([
        "Hi! What would you like to work on?",
        { "content": "Listing.", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo listed" } }] },
        "There is one file."
    ]));
    let traj = scratch.path("run.traj.json");
    let envs = [("UMBRA_FORCE_INTERACTIVE", "1"), ("UMBRA_REPLAY_RECORD", &slash(&record) as &str)];
    // The follow up is typed once the first reply is in, as a person would. Typed earlier, it is
    // read while the first turn runs and goes to the model as a message sent during the turn.
    let mut running = Running::start(&scratch.path("home"), &["-m", &model, "-y", "--no-mcp", "--cwd", &slash(&scratch.root), "-o", &slash(&traj), "Hello"], &envs);
    let started = std::time::Instant::now();
    while recorded_requests(&record).is_empty() && started.elapsed() < std::time::Duration::from_secs(20) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    running.type_line("list the files");
    let output = running.finish(std::time::Duration::from_secs(30));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let requests = recorded_requests(&record);
    assert_eq!(requests.len(), 3);
    let second: Vec<&str> = requests[1]["messages"].as_array().unwrap().iter().filter_map(|m| m["content"].as_str()).collect();
    assert_eq!(second[second.len() - 2], "Hi! What would you like to work on?", "the model sees its own reply");
    assert_eq!(second[second.len() - 1], "list the files", "the follow up is the person's words");
    let shown = text(&output);
    assert_eq!(shown.matches("There is one file.").count(), 1, "the final reply is shown once:\n{shown}");
    assert_eq!(read_trajectory(&traj)["info"]["exit_status"], "Replied");
}

#[test]
fn the_step_limit_stops_the_run() {
    let scratch = Scratch::new("limit");
    let reply = json!({ "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo step" } }] });
    let model = script(&scratch, json!([reply.clone(), reply.clone(), reply]));
    let traj = scratch.path("run.traj.json");
    let output = run_umbra(&scratch.path("home"), &["-m", &model, "-y", "--no-mcp", "--step-limit", "2", "--cwd", &slash(&scratch.root), "-o", &slash(&traj), "task"]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    let trajectory = read_trajectory(&traj);
    assert_eq!(trajectory["info"]["exit_status"], "LimitsExceeded");
    assert_eq!(trajectory["info"]["model_stats"]["api_calls"], 2);
}

#[test]
fn a_command_that_runs_too_long_is_killed() {
    let scratch = Scratch::new("timeout");
    scratch.write("short.toml", "[environment]\ntimeout = 1\n");
    let model = script(&scratch, json!([
        { "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "sleep 20" } }] },
        { "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT" } }] }
    ]));
    let traj = scratch.path("run.traj.json");
    let started = std::time::Instant::now();
    let output = run_umbra(&scratch.path("home"), &["-m", &model, "-y", "--no-mcp", "-c", &slash(&scratch.path("short.toml")), "--cwd", &slash(&scratch.root), "-o", &slash(&traj), "task"]);
    assert!(started.elapsed().as_secs() < 15, "the command was not killed");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let tool = &read_trajectory(&traj)["messages"][3];
    assert_eq!(tool["extra"]["returncode"], -1);
    assert!(tool["extra"]["exception_info"].as_str().unwrap().contains("timed out after 1 seconds"));
}

#[test]
fn unknown_tools_are_reported_to_the_model() {
    let scratch = Scratch::new("unknown");
    let model = script(&scratch, json!([
        { "content": "", "tool_calls": [{ "name": "str_replace_editor", "arguments": {} }] },
        { "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT" } }] }
    ]));
    let traj = scratch.path("run.traj.json");
    let output = run_umbra(&scratch.path("home"), &["-m", &model, "-y", "--no-mcp", "--cwd", &slash(&scratch.root), "-o", &slash(&traj), "task"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let message = &read_trajectory(&traj)["messages"][2];
    assert!(message["content"].as_str().unwrap().contains("Unknown tool 'str_replace_editor'."));
}

#[test]
fn confirm_mode_refuses_to_run_without_a_terminal() {
    let scratch = Scratch::new("confirm");
    let model = script(&scratch, json!(["x"]));
    let output = run_umbra(&scratch.path("home"), &["-m", &model, "--no-mcp", "--cwd", &slash(&scratch.root), "task"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output).contains("Pass --yolo"));
}

/// With no model set, an interactive start says so before it asks for a task, so what the person
/// types is never taken and thrown away when the run fails.
#[test]
fn no_model_is_reported_before_the_prompt() {
    let scratch = Scratch::new("no-model");
    let envs = [("UMBRA_FORCE_INTERACTIVE", "1")];
    let output = run_umbra_with(&scratch.path("home"), &["--no-mcp", "--cwd", &slash(&scratch.root)], &envs, "hello\n");
    let shown = text(&output);
    assert_eq!(output.status.code(), Some(2), "{shown}");
    assert!(shown.contains("no model set"), "{shown}");
    assert!(shown.contains("umbra setup"), "the message names the setup command:\n{shown}");
    assert!(!shown.contains("What do you want to do?"), "no prompt is shown when no model can answer it:\n{shown}");
}
