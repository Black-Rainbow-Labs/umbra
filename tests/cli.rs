//! The command line forms of Umbra's settings and commands, run against a fake home folder:
//! `umbra config`, `umbra mcp enable` and `disable`, `umbra models`, and `umbra session`, which a
//! command the agent runs uses to change the agent's own session.

mod common;

use common::{read_trajectory, run_umbra, slash, text, Scratch};
use serde_json::json;

/// The user config file in a fake home folder.
/// @param scratch - the test folder
fn user_config(scratch: &Scratch) -> String {
    std::fs::read_to_string(scratch.path("home/.umbra/config.toml")).unwrap_or_default()
}

#[test]
fn config_set_get_list_and_unset_change_the_user_file_and_keep_comments() {
    let scratch = Scratch::new("cli-config");
    let home = scratch.path("home");
    let work = slash(&scratch.root);
    scratch.write("home/.umbra/config.toml", "# my settings\nmodel = \"anthropic/claude-haiku-5-5\"\n");
    let output = run_umbra(&home, &["config", "set", "compaction.trigger_ratio", "70", "--cwd", &work]);
    assert!(output.status.success(), "{}", text(&output));
    let output = run_umbra(&home, &["config", "get", "compaction.trigger_ratio", "--cwd", &work]);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0.7");
    let output = run_umbra(&home, &["config", "set", "mcp.defer_tools", "sometimes", "--cwd", &work]);
    assert!(!output.status.success() && text(&output).contains("Choose one of"), "{}", text(&output));
    let output = run_umbra(&home, &["config", "list", "compaction", "--cwd", &work]);
    assert!(regex::Regex::new(r"compaction\.trigger_ratio\s+70%\s+user").unwrap().is_match(&text(&output)), "{}", text(&output));
    assert!(user_config(&scratch).contains("# my settings"), "{}", user_config(&scratch));
    let output = run_umbra(&home, &["config", "unset", "compaction.trigger_ratio", "--cwd", &work]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(!user_config(&scratch).contains("trigger_ratio"), "{}", user_config(&scratch));
    let output = run_umbra(&home, &["config", "set", "syntax_highlight", "false", "--project", "--cwd", &work]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(std::fs::read_to_string(scratch.path(".umbra/config.toml")).unwrap().contains("syntax_highlight = false"));
}

#[test]
fn mcp_disable_and_enable_change_the_lists_and_the_listing() {
    let scratch = Scratch::new("cli-mcp");
    let home = scratch.path("home");
    let work = slash(&scratch.root);
    scratch.write("home/.claude.json", &json!({ "mcpServers": { "blender": { "command": "blender-mcp" }, "github": { "command": "gh-mcp" } } }).to_string());
    let output = run_umbra(&home, &["mcp", "disable", "blender", "--cwd", &work]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(user_config(&scratch).contains("disabled = [\"blender\"]"), "{}", user_config(&scratch));
    let listing = text(&run_umbra(&home, &["mcp", "--cwd", &work]));
    assert!(regex::Regex::new(r"blender\s+off").unwrap().is_match(&listing), "{listing}");
    assert!(regex::Regex::new(r"github\s+on").unwrap().is_match(&listing), "{listing}");
    let output = run_umbra(&home, &["mcp", "enable", "blender", "--cwd", &work]);
    assert!(output.status.success(), "{}", text(&output));
    assert!(user_config(&scratch).contains("disabled = []"), "{}", user_config(&scratch));
}

#[test]
fn models_lists_the_default_and_the_known_models() {
    let scratch = Scratch::new("cli-models");
    scratch.write("home/.umbra/config.toml", "model = \"opus\"\n[models.opus]\nprovider = \"anthropic\"\nmodel = \"claude-opus-5-5\"\n");
    let output = run_umbra(&scratch.path("home"), &["models", "--cwd", &slash(&scratch.root)]);
    let out = text(&output);
    assert!(out.contains("default  opus") && out.contains("opus             anthropic/claude-opus-5-5") && out.contains("claude-haiku-5-5"), "{out}");
}

#[test]
fn a_command_the_agent_runs_switches_its_own_session_with_umbra_session() {
    let scratch = Scratch::new("cli-session");
    let second = scratch.write("second.json", &json!(["Switched to the second model."]).to_string());
    let command = format!("\"$UMBRA_BIN\" session status && \"$UMBRA_BIN\" session model replay/{}", slash(&second));
    let first = scratch.write("first.json", &json!([{ "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": command } }] }, "Still on the first model."]).to_string());
    let traj = scratch.path("run.traj.json");
    let output = run_umbra(&scratch.path("home"), &["-m", &format!("replay/{}", slash(&first)), "-y", "--no-mcp", "--cwd", &slash(&scratch.root), "-o", &slash(&traj), "switch models"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let trajectory = read_trajectory(&traj);
    let messages = trajectory["messages"].as_array().unwrap();
    let observation = messages.iter().find(|m| m["role"] == "tool").map(|m| m["content"].to_string()).unwrap_or_default();
    assert!(observation.contains("model    replay/") && observation.contains("mode     yolo") && observation.contains("sent: the model changes"), "{observation}");
    let last = messages.iter().rev().find(|m| m["role"] == "assistant").unwrap();
    assert_eq!(last["content"], "Switched to the second model.");
    let left: Vec<_> = std::fs::read_dir(scratch.path("home/.umbra/control")).map(|d| d.flatten().collect()).unwrap_or_default();
    assert!(left.is_empty(), "the control folder is removed when the session ends");
}

#[test]
fn umbra_session_outside_a_session_says_none_is_running() {
    let scratch = Scratch::new("cli-no-session");
    let output = run_umbra(&scratch.path("home"), &["session", "mode", "yolo"]);
    assert!(!output.status.success());
    assert!(text(&output).contains("no Umbra session is running"), "{}", text(&output));
}
