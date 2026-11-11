//! Runs the real binary and checks what the person sees in the terminal: the banner, replies,
//! the one line notes for routine events, and that an event that changed nothing is not repeated.

mod common;

use common::{run_umbra_with, slash, text, Scratch};
use serde_json::{json, Value};

/// Writes a replay script and returns the `-m` value for it.
/// @param scratch - the test folder
/// @param replies - the scripted replies
fn script(scratch: &Scratch, replies: Value) -> String {
    let path = scratch.write("script.json", &replies.to_string());
    format!("replay/{}", slash(&path))
}

/// The reply that ends the run with a final output.
fn submit() -> Value {
    json!({ "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT && echo all done" } }] })
}

/// Runs umbra in yolo mode with the given replies and extra options. Returns all of its output.
/// @param scratch - the test folder
/// @param replies - the scripted replies
/// @param extra - more command line options
/// @param envs - extra environment variables
fn run(scratch: &Scratch, replies: Value, extra: &[&str], envs: &[(&str, &str)]) -> String {
    let model = script(scratch, replies);
    let traj = slash(&scratch.path("run.traj.json"));
    let cwd = slash(&scratch.root);
    let mut args = vec!["-m", model.as_str(), "-y", "--no-mcp", "--cwd", cwd.as_str(), "-o", traj.as_str()];
    args.extend_from_slice(extra);
    args.push("say hello");
    let output = run_umbra_with(&scratch.path("home"), &args, envs, "");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    text(&output)
}

#[test]
fn a_compaction_that_cannot_shrink_the_history_warns_once() {
    let scratch = Scratch::new("stuck-compaction");
    let call = json!({ "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo hi" } }] });
    let config = scratch.write("compaction.toml", "[compaction]\nmin_reclaim_ratio = 0.5\n");
    let config = slash(&config);
    let output = run(&scratch, json!([call, call, call, submit()]), &["--context-window", "500", "-c", &config], &[]);
    assert_eq!(output.matches("compaction cannot remove them").count(), 1, "said once, not before every call:\n{output}");
    assert!(output.contains("so none was made"), "{output}");
    assert!(!output.contains("could not summarize") && !output.contains("Compaction failed"), "no summary is attempted:\n{output}");
}

#[test]
fn a_reply_without_a_command_ends_the_turn_and_is_shown_once() {
    let scratch = Scratch::new("reply");
    let output = run(&scratch, json!(["Hello. What would you like me to do?"]), &[], &[]);
    assert_eq!(output.matches("● Hello. What would you like me to do?").count(), 1, "{output}");
    assert_eq!(output.matches("What would you like me to do?").count(), 1, "the turn summary does not repeat the reply:\n{output}");
    assert!(!output.contains("✔ Done") && !output.contains("1 step"), "the Done line is gone (task-2611):\n{output}");
    assert!(!output.contains("asked the model again"), "{output}");
}

#[test]
fn a_reply_without_a_command_is_shown_with_one_note_in_the_submit_style() {
    let scratch = Scratch::new("no-command");
    let output = run(&scratch, json!(["Hello. What would you like me to do?", submit()]), &["--finish", "submit"], &[]);
    assert!(output.contains("● Hello. What would you like me to do?"), "the reply is shown like any reply:\n{output}");
    assert!(output.contains("↺ no tool calls found in the response; asked the model again (1 of 3)"), "{output}");
    assert!(!output.contains("MUST include"), "the full error is only in verbose output:\n{output}");
    assert!(!output.contains("✔ Done") && output.contains("● all done"), "the final output is shown as a reply, with no Done line:\n{output}");
    assert_eq!(output.matches("COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT").count(), 1, "the submit command is shown, its output is not shown twice:\n{output}");
}

#[test]
fn verbose_output_shows_the_whole_error_and_the_steps() {
    let scratch = Scratch::new("verbose");
    let output = run(&scratch, json!(["Hello.", submit()]), &["--verbose", "--finish", "submit"], &[]);
    assert!(output.contains("MUST include at least one tool call"), "{output}");
    assert!(output.contains("step 2 · $0.0000"), "{output}");
}

#[test]
fn long_command_output_is_cut_to_ten_lines() {
    let scratch = Scratch::new("long-output");
    let call = json!({ "content": "Counting.", "tool_calls": [{ "name": "bash", "arguments": { "command": "seq 1 30" } }] });
    let output = run(&scratch, json!([call, submit()]), &[], &[]);
    assert!(output.contains("⎿  1\n"), "{output}");
    assert!(output.contains("… 20 more lines"), "{output}");
    assert!(!output.contains("\n     11\n"), "{output}");
}

#[test]
fn the_banner_shows_the_version_and_model() {
    let scratch = Scratch::new("banner");
    let output = run(&scratch, json!([submit()]), &[], &[("UMBRA_BANNER", "1"), ("COLUMNS", "100")]);
    assert!(output.contains(&format!("umbra v{}", env!("CARGO_PKG_VERSION"))), "{output}");
    assert!(output.contains("by Black Rainbow Labs") && output.contains("model   replay/"), "{output}");
    assert!(output.contains("❯ say hello"), "the task is echoed under the banner:\n{output}");
    let scratch = Scratch::new("no-banner");
    let output = run(&scratch, json!([submit()]), &[], &[]);
    assert!(!output.contains("by Black Rainbow Labs"), "no banner in a pipe:\n{output}");
}
