//! Runs the real binary with scripted replies and reads the status bar out of its output.
//! `UMBRA_STATUS_BAR=1` draws the bar through a pipe, which is how a test can see it, and
//! `COLUMNS` sets the width the bar has to fit.

mod common;

use common::{read_trajectory, run_umbra_with, slash, text, Scratch};
use serde_json::{json, Value};

/// The codes that end every drawing of the bar: up one line and back to its start.
const BAR_END: &str = "\x1b[1A\r";

/// A bash call that prints `size` letter a characters.
/// @param size - how many characters the command prints
fn print_call(size: usize) -> Value {
    json!({ "name": "bash", "arguments": { "command": format!("head -c {size} < /dev/zero | tr '\\0' a") } })
}

/// The reply that ends the run.
fn submit() -> Value {
    json!({ "content": "Done.", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT && echo finished" } }] })
}

/// Runs umbra with ten 2000 character outputs and a 6000 token window, so compaction runs.
/// Returns stdout and the trajectory.
/// @param scratch - the test folder
/// @param extra_args - more command line options
/// @param envs - extra environment variables
/// @param input - what to type on stdin
fn run(scratch: &Scratch, extra_args: &[&str], envs: &[(&str, &str)], input: &str) -> (String, Value) {
    let mut replies: Vec<Value> = (0..10).map(|_| json!({ "content": "", "tool_calls": [print_call(2000)] })).collect();
    replies.push(submit());
    let script = scratch.write("script.json", &json!(replies).to_string());
    let model = format!("replay/{}", slash(&script));
    let traj = slash(&scratch.path("run.traj.json"));
    let cwd = slash(&scratch.root);
    let mut args = vec!["-m", model.as_str(), "-y", "--no-mcp", "--cwd", cwd.as_str(), "-o", traj.as_str(), "--context-window", "6000"];
    args.extend_from_slice(extra_args);
    args.push("do the task");
    let output = run_umbra_with(&scratch.path("home"), &args, envs, input);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    (String::from_utf8_lossy(&output.stdout).into_owned(), read_trajectory(&scratch.path("run.traj.json")))
}

/// Every bar drawn under the output, in order: the line before each `BAR_END`.
/// @param stdout - the run's output
fn frames(stdout: &str) -> Vec<String> {
    let pieces: Vec<&str> = stdout.split(BAR_END).collect();
    pieces[..pieces.len() - 1].iter().map(|piece| piece.rsplit('\n').next().unwrap_or("").to_string()).collect()
}

#[test]
fn the_bar_follows_the_context_and_counts_compactions() {
    let scratch = Scratch::new("status-bar");
    let (stdout, trajectory) = run(&scratch, &[], &[("UMBRA_STATUS_BAR", "1"), ("COLUMNS", "150")], "");
    let frames = frames(&stdout);
    assert!(frames.len() > 20, "the bar is drawn after each step: {} frames", frames.len());
    assert!(stdout.contains("\r\x1b[J"), "the bar is erased before new output");
    for frame in &frames {
        assert!(frame.chars().count() < 150, "the bar fits the width: {frame}");
        assert!(frame.contains("/ 6k") && frame.contains('%'), "the bar shows the window and the share in use: {frame}");
    }
    assert!(frames.iter().any(|f| f.contains("● thinking")), "{frames:#?}");
    assert!(frames.iter().any(|f| f.contains("● running head")), "{frames:#?}");
    assert!(frames.iter().any(|f| f.contains("left before compaction at 80%")), "{frames:#?}");
    assert!(frames[0].contains("↻ 0 compactions"), "{}", frames[0]);
    let compactions = trajectory["info"]["model_stats"]["compactions"].as_u64().unwrap();
    assert!(compactions >= 1, "the run compacted");
    let last = frames.last().unwrap();
    let expected = if compactions == 1 { "↻ 1 compaction".to_string() } else { format!("↻ {compactions} compactions") };
    assert!(last.contains(&expected), "the counter matches the trajectory ({compactions}): {last}");
    let final_line = stdout.trim_end().rsplit('\n').next().unwrap();
    assert!(final_line.contains("● finished") && final_line.contains(&expected), "the last line keeps the final numbers: {final_line}");
}

#[test]
fn the_bar_is_drawn_under_a_prompt() {
    let scratch = Scratch::new("status-bar-prompt");
    let envs = [("UMBRA_STATUS_BAR", "1"), ("UMBRA_FORCE_INTERACTIVE", "1"), ("COLUMNS", "120")];
    let (stdout, _) = run(&scratch, &[], &envs, "");
    let hint = stdout.find("Type a follow up task, or press Enter to quit").expect("the follow up hint is shown");
    let at = hint + stdout[hint..].find("❯ ").expect("the follow up prompt is shown under the hint");
    let before = &stdout[..at];
    assert!(before.ends_with(BAR_END), "the bar is drawn just before the prompt, under the prompt's line");
    let bar = frames(&stdout[..at + "❯".len()]).pop().unwrap();
    assert!(bar.contains("● waiting for you"), "{bar}");
}

#[test]
fn no_bar_without_a_terminal_or_with_the_flag() {
    let scratch = Scratch::new("status-bar-off");
    let (stdout, _) = run(&scratch, &[], &[("COLUMNS", "120")], "");
    assert!(!stdout.contains('\x1b'), "no escape codes reach a pipe");
    assert!(!stdout.contains("● thinking"));
    let scratch = Scratch::new("status-bar-flag");
    let (stdout, _) = run(&scratch, &["--no-status-bar"], &[("UMBRA_STATUS_BAR", "1")], "");
    assert!(!stdout.contains('\x1b') && !stdout.contains("● thinking"), "--no-status-bar wins");
}
