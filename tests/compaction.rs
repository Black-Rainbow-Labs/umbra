//! Runs the real binary with scripted replies and a small context window, and checks what the
//! model was sent before and after each compaction. The replay provider writes every request it
//! receives to the file in `UMBRA_REPLAY_RECORD`, one JSON line per request.

mod common;

use std::path::Path;
use std::process::{Command, Output};

use common::{read_trajectory, slash, text, Scratch};
use serde_json::{json, Value};

/// A bash call that prints `size` letter a characters.
/// @param size - how many characters the command prints
fn print_call(size: usize) -> Value {
    json!({ "name": "bash", "arguments": { "command": format!("head -c {size} < /dev/zero | tr '\\0' a") } })
}

/// The reply that ends the run.
fn submit() -> Value {
    json!({ "content": "Done.", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT && echo finished" } }] })
}

/// Runs umbra with the replay script, recording each request, and returns the output and the
/// recorded requests.
/// @param scratch - the test folder
/// @param replies - the scripted replies
/// @param extra_args - more command line options
fn run(scratch: &Scratch, replies: Value, extra_args: &[&str]) -> (Output, Vec<Value>) {
    let script = scratch.write("script.json", &replies.to_string());
    let record = scratch.path("requests.jsonl");
    let model = format!("replay/{}", slash(&script));
    let traj = slash(&scratch.path("run.traj.json"));
    let cwd = slash(&scratch.root);
    let mut args = vec!["-m", model.as_str(), "-y", "--no-mcp", "--cwd", cwd.as_str(), "-o", traj.as_str()];
    args.extend_from_slice(extra_args);
    args.push("do the task");
    let output = run_recording(&scratch.path("home"), &args, &record);
    let requests = std::fs::read_to_string(&record).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    (output, requests)
}

/// Runs the binary the way `common::run_umbra` does, with the replay recording turned on.
/// @param home - the fake home folder
/// @param args - the command line
/// @param record - the file the replay provider writes requests to
fn run_recording(home: &Path, args: &[&str], record: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_umbra"))
        .args(args)
        .env("UMBRA_HOME_OVERRIDE", home)
        .env("UMBRA_REPLAY_RECORD", record)
        .env("UMBRA_RETRY_BASE_MS", "10")
        .env("MSYS_NO_PATHCONV", "1")
        .env("NO_COLOR", "1")
        .env_remove("CODEX_HOME")
        .env_remove("UMBRA_MODEL")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("umbra binary runs")
}

/// The messages of one recorded request.
/// @param request - a recorded request
fn messages(request: &Value) -> &Vec<Value> {
    request["messages"].as_array().unwrap()
}

/// The contents of the command outputs in one recorded request, in order.
/// @param request - a recorded request
fn outputs(request: &Value) -> Vec<String> {
    messages(request).iter().filter(|m| m["role"] == "tool").map(|m| m["content"].as_str().unwrap().to_string()).collect()
}

/// Returns true for a placeholder left by the mask pass.
/// @param content - a message's content
fn is_placeholder(content: &str) -> bool {
    content.starts_with("[Output removed to save context after you read it: 2000 characters, return code 0.")
}

#[test]
fn masking_replaces_old_outputs_first() {
    let scratch = Scratch::new("compact-mask");
    let mut replies: Vec<Value> = (0..10).map(|_| json!({ "content": "", "tool_calls": [print_call(2000)] })).collect();
    replies.push(submit());
    let (output, requests) = run(&scratch, json!(replies), &["--context-window", "6000"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let trajectory = read_trajectory(&scratch.path("run.traj.json"));
    let records = trajectory["info"]["compactions"].as_array().unwrap();
    assert!(!records.is_empty(), "no compaction ran: {}", text(&output));
    assert!(records.iter().all(|r| r["kind"] == "mask"), "masking alone was enough, so no summary should run: {records:?}");
    assert_eq!(requests.len(), 11, "no extra model call is made for a mask");
    let last = outputs(requests.last().unwrap());
    assert!(last.iter().take(3).all(|c| is_placeholder(c)), "the oldest outputs are masked: {last:?}");
    assert!(last.iter().rev().take(2).all(|c| c.contains("aaaa")), "the newest outputs are kept whole");
    assert_eq!(trajectory["info"]["model_stats"]["compactions"].as_u64().unwrap() as usize, records.len());
    let first_masked = records[0]["masked_message_indexes"][0].as_u64().unwrap() as usize;
    assert_eq!(trajectory["messages"][first_masked]["extra"]["raw_output"].as_str().unwrap().len(), 2000, "the trajectory keeps the full output");
}

#[test]
fn masking_works_in_text_mode() {
    let scratch = Scratch::new("compact-text");
    let block = "THOUGHT: print.\n\n```umbra_bash\nhead -c 2000 < /dev/zero | tr '\\0' a\n```";
    let mut replies: Vec<Value> = (0..10).map(|_| json!(block)).collect();
    replies.push(json!("THOUGHT: done.\n\n```umbra_bash\necho COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT\n```"));
    let (output, requests) = run(&scratch, json!(replies), &["--text", "--context-window", "6000"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let masked = messages(requests.last().unwrap()).iter().filter(|m| m["role"] == "user" && m["content"].as_str().unwrap().starts_with("[Output removed to save context after you read it:")).count();
    assert!(masked >= 3, "old text mode observations are masked: {masked}");
}

#[test]
fn no_compact_turns_it_off() {
    let scratch = Scratch::new("compact-off");
    let mut replies: Vec<Value> = (0..10).map(|_| json!({ "content": "", "tool_calls": [print_call(2000)] })).collect();
    replies.push(submit());
    let (output, requests) = run(&scratch, json!(replies), &["--context-window", "6000", "--no-compact"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let trajectory = read_trajectory(&scratch.path("run.traj.json"));
    assert!(trajectory["info"]["compactions"].as_array().unwrap().is_empty());
    assert!(outputs(requests.last().unwrap()).iter().all(|c| c.contains("aaaa")));
}

#[test]
fn summary_runs_when_masking_is_not_enough() {
    let scratch = Scratch::new("compact-summary");
    let config = scratch.write("compact.toml", "[compaction]\ncontext_window = 10700\nkeep_recent_ratio = 0.15\n");
    let long_reply = json!({ "content": "b".repeat(4000), "tool_calls": [{ "name": "bash", "arguments": { "command": "echo ok" } }] });
    let mut replies: Vec<Value> = (0..8).map(|_| long_reply.clone()).collect();
    replies.push(json!("SUMMARY-TEXT: printed ok eight times."));
    replies.push(submit());
    let (output, requests) = run(&scratch, json!(replies), &["-c", &slash(&config)]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let trajectory = read_trajectory(&scratch.path("run.traj.json"));
    let records = trajectory["info"]["compactions"].as_array().unwrap();
    assert_eq!(records.len(), 1, "{records:?}\n{}", text(&output));
    assert_eq!(records[0]["kind"], "summary");
    assert_eq!(records[0]["reason"], "threshold");
    assert_eq!(records[0]["summary"], "SUMMARY-TEXT: printed ok eight times.");
    assert_eq!(records[0]["removed_messages"].as_array().unwrap().len(), 14, "seven turns of two messages were summarized");
    let summary_request = &requests[8];
    let ask = messages(summary_request).last().unwrap()["content"].as_str().unwrap();
    assert!(ask.contains("about to be compacted"), "the ninth request asks for the summary: {ask}");
    assert_eq!(summary_request["tools"], json!(["bash"]), "the summary request offers the same tools");
    let after = messages(&requests[9]);
    let roles: Vec<&str> = after.iter().map(|m| m["role"].as_str().unwrap()).collect();
    assert_eq!(roles, vec!["system", "user", "user", "assistant", "tool"], "system, task, summary, then the newest turn");
    assert!(after[1]["content"].as_str().unwrap().contains("do the task"));
    assert!(after[2]["content"].as_str().unwrap().contains("SUMMARY-TEXT"));
    assert_eq!(after[4]["tool_call_id"], after[3]["tool_calls"][0]["id"], "the kept tool result still follows its call");
}

#[test]
fn an_overflow_error_compacts_and_retries() {
    let scratch = Scratch::new("compact-overflow");
    let config = scratch.write("compact.toml", "[compaction]\nkeep_recent_ratio = 0.001\n");
    let replies = json!([
        { "content": "", "tool_calls": [print_call(2000)] },
        { "content": "", "tool_calls": [print_call(2000)] },
        { "error": { "status": 400, "body": "{\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"prompt is too long: 210000 tokens > 200000 maximum\"}}" } },
        "SUMMARY-TEXT: printed two blocks of a.",
        submit()
    ]);
    let (output, requests) = run(&scratch, replies, &["-c", &slash(&config)]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let trajectory = read_trajectory(&scratch.path("run.traj.json"));
    let records = trajectory["info"]["compactions"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["kind"], "summary");
    assert_eq!(records[0]["reason"], "overflow");
    let retried = messages(requests.last().unwrap());
    assert!(retried.iter().any(|m| m["content"].as_str().unwrap().contains("SUMMARY-TEXT")), "the retried request carries the summary");
    assert_eq!(trajectory["info"]["model_stats"]["api_calls"], 3, "the summary call and the retry are part of the third step");
    assert_eq!(requests.len(), 5, "two steps, the refused request, the summary request and the retry");
}

#[test]
fn a_failed_summary_is_not_tried_again_on_the_next_steps() {
    let scratch = Scratch::new("compact-summary-failed");
    let config = scratch.write("compact.toml", "[compaction]\ncontext_window = 10700\nkeep_recent_ratio = 0.15\n");
    let long_reply = json!({ "content": "b".repeat(4000), "tool_calls": [{ "name": "bash", "arguments": { "command": "echo ok" } }] });
    let short_reply = json!({ "content": "c", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo ok" } }] });
    let mut replies: Vec<Value> = (0..8).map(|_| long_reply.clone()).collect();
    replies.push(json!(""));
    replies.extend([short_reply.clone(), short_reply, submit()]);
    let (output, requests) = run(&scratch, json!(replies), &["-c", &slash(&config)]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let asks = requests.iter().filter(|r| messages(r).last().and_then(|m| m["content"].as_str()).is_some_and(|c| c.contains("about to be compacted"))).count();
    assert_eq!(asks, 1, "the summary that failed is not asked for again before the context grows");
}

#[test]
fn max_context_tokens_caps_the_working_window() {
    let scratch = Scratch::new("compact-cap");
    let config = scratch.write("compact.toml", "[compaction]\ncontext_window = 1000000\nmax_context_tokens = 10700\nkeep_recent_ratio = 0.15\n");
    let long_reply = json!({ "content": "b".repeat(4000), "tool_calls": [{ "name": "bash", "arguments": { "command": "echo ok" } }] });
    let mut replies: Vec<Value> = (0..8).map(|_| long_reply.clone()).collect();
    replies.push(json!("SUMMARY-TEXT: capped."));
    replies.push(submit());
    let (output, _) = run(&scratch, json!(replies), &["-c", &slash(&config)]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let records = read_trajectory(&scratch.path("run.traj.json"))["info"]["compactions"].as_array().unwrap().clone();
    assert_eq!(records.len(), 1, "the cap makes a 1,000,000 window compact like a 10,700 one: {records:?}");
    assert_eq!(records[0]["window"], 10700);
}
