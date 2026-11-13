//! Sessions: `--resume`, `--continue`, `--fork`, `umbra sessions`, and repairing a history that
//! stopped in the middle of a step.

mod common;

use std::path::{Path, PathBuf};

use common::{read_trajectory, recorded_requests, replay_model, run_umbra_with, slash, text, write_png, Scratch};
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

/// One test's folders: a fake home, a working folder and a record file.
struct Setup {
    scratch: Scratch,
    work: PathBuf,
}

impl Setup {
    /// Creates the folders.
    /// @param name - the test name
    fn new(name: &str) -> Self {
        let scratch = Scratch::new(name);
        let work = scratch.path("work");
        std::fs::create_dir_all(&work).unwrap();
        Setup { scratch, work }
    }

    /// Runs umbra in yolo mode with no MCP servers, recording what the model is sent.
    /// @param args - the arguments
    /// @param input - what to type on stdin
    /// @param interactive - whether a piped stdin counts as a terminal
    fn umbra(&self, args: &[&str], input: &str, interactive: bool) -> std::process::Output {
        let mut all = vec!["-y", "--no-mcp"];
        all.extend(args);
        let record = slash(&self.record());
        let mut envs = vec![("UMBRA_REPLAY_RECORD", record.as_str())];
        if interactive {
            envs.push(("UMBRA_FORCE_INTERACTIVE", "1"));
        }
        run_umbra_with(&self.scratch.path("home"), &all, &envs, input)
    }

    /// Starts a session in the working folder and returns its trajectory.
    /// @param script - the script file name
    /// @param replies - the scripted replies
    /// @param task - the task
    fn first_run(&self, script: &str, replies: Value, task: &str) -> Value {
        let model = replay_model(&self.scratch, script, &replies);
        let output = self.umbra(&["-m", &model, "--cwd", &slash(&self.work), task], "", false);
        assert_eq!(output.status.code(), Some(0), "{}", text(&output));
        let path = exported_trajectory(&self.scratch.path("home"), &output);
        read_trajectory(&path)
    }

    /// Runs `umbra sessions` with the given options.
    /// @param args - the options after `sessions`
    fn sessions(&self, args: &[&str]) -> std::process::Output {
        let mut all = vec!["sessions"];
        all.extend(args);
        run_umbra_with(&self.scratch.path("home"), &all, &[], "")
    }

    /// The file the replay provider records requests in.
    fn record(&self) -> PathBuf {
        self.scratch.path("record.jsonl")
    }

    /// The requests recorded so far.
    fn requests(&self) -> Vec<Value> {
        recorded_requests(&self.record())
    }
}

/// Reads the session file out of the run summary: the trajectory when one was written, or the
/// journal (`session <path>`), which is all a run without -o writes (task-2643, E17).
/// @param output - the run's output
fn session_line(output: &std::process::Output) -> PathBuf {
    let all = text(output);
    let line = all.lines().map(str::trim_start).find(|l| l.starts_with("trajectory ") || l.starts_with("session ")).unwrap_or_else(|| panic!("no trajectory or session line in:\n{all}"));
    PathBuf::from(line.split_once(' ').unwrap().1.trim())
}

/// The trajectory path of a run's session, written with `umbra export` when the run wrote only
/// its journal.
/// @param home - the isolated home
/// @param output - the run's output
fn exported_trajectory(home: &Path, output: &std::process::Output) -> PathBuf {
    let file = session_line(output);
    let name = file.to_string_lossy().to_string();
    let Some(stem) = name.strip_suffix(".journal.jsonl") else { return file };
    let exported = run_umbra_with(home, &["export", &name], &[], "");
    assert_eq!(exported.status.code(), Some(0), "{}", text(&exported));
    PathBuf::from(format!("{stem}.json"))
}

/// The session id of a trajectory.
/// @param trajectory - the trajectory JSON
fn session_id(trajectory: &Value) -> String {
    trajectory["info"]["session"]["id"].as_str().unwrap().to_string()
}

/// The roles of the messages in a recorded request.
/// @param request - one recorded request
fn roles(request: &Value) -> Vec<String> {
    request["messages"].as_array().unwrap().iter().map(|m| m["role"].as_str().unwrap().to_string()).collect()
}

#[test]
fn resume_by_id_continues_the_same_session_file() {
    let setup = Setup::new("resume-id");
    let first = setup.first_run("one.json", json!([call("echo remembered > note.txt"), submit("first")]), "write a note");
    let id = session_id(&first);
    let model = replay_model(&setup.scratch, "two.json", &json!([submit("second")]));
    let output = setup.umbra(&["-m", &model, "--resume", &id, "read the note"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let request = setup.requests().last().unwrap().clone();
    assert_eq!(roles(&request), ["system", "user", "assistant", "tool", "assistant", "tool", "user"], "the old history is sent without its exit marker");
    assert_eq!(request["messages"][6]["content"], "read the note");
    let path = exported_trajectory(&setup.scratch.path("home"), &output);
    assert!(path.to_string_lossy().contains(&id), "the same session file is written");
    let trajectory = read_trajectory(&path);
    assert_eq!(trajectory["info"]["model_stats"]["api_calls"], 3, "the totals continue");
    assert_eq!(trajectory["info"]["submission"].as_str().unwrap().trim(), "second");
    assert_eq!(trajectory["info"]["session"]["resumes"].as_array().unwrap().len(), 1);
    assert!(Path::new(&slash(&setup.work)).join("note.txt").exists());
    assert!(!text(&output).contains("--resume"), "no resume command is printed (task-2611):
{}", text(&output));
}

#[test]
fn a_resume_runs_in_the_sessions_folder_with_its_model() {
    let setup = Setup::new("resume-defaults");
    let first = setup.first_run("one.json", json!([submit("same script")]), "task");
    let id = session_id(&first);
    let output = setup.umbra(&["--resume", &id, "again"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let trajectory = read_trajectory(&exported_trajectory(&setup.scratch.path("home"), &output));
    assert!(trajectory["info"]["config"]["model"]["replay_file"].as_str().unwrap().ends_with("one.json"), "the session's model is used");
    // Compared as real paths: a runner's temp folder can be a link (/var -> /private/var on macOS)
    // or a Windows short name (RUNNER~1), and the recorded folder is the real one.
    let recorded = Path::new(trajectory["info"]["session"]["cwd"].as_str().unwrap()).canonicalize().unwrap();
    assert_eq!(recorded, setup.work.canonicalize().unwrap());
}

#[test]
fn continue_takes_the_newest_session_in_this_folder() {
    let setup = Setup::new("continue");
    let other = setup.scratch.path("other");
    std::fs::create_dir_all(&other).unwrap();
    setup.first_run("a.json", json!([submit("a")]), "older task here");
    setup.first_run("b.json", json!([submit("b")]), "newer task here");
    let model = replay_model(&setup.scratch, "c.json", &json!([submit("c")]));
    let output = setup.umbra(&["-m", &model, "--cwd", &slash(&other), "unrelated"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let output = setup.umbra(&["-m", &model, "--continue", "--cwd", &slash(&setup.work), "and then"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let request = setup.requests().last().unwrap().clone();
    assert!(request["messages"][1]["content"].as_str().unwrap().contains("newer task here"), "{}", request["messages"][1]);
}

#[test]
fn resume_by_prefix_or_path_and_unknown_ids_fail() {
    let setup = Setup::new("resume-prefix");
    let first = setup.first_run("one.json", json!([submit("x")]), "task");
    let id = session_id(&first);
    let model = replay_model(&setup.scratch, "two.json", &json!([submit("y")]));
    let output = setup.umbra(&["-m", &model, "-r", &id[..id.len() - 2], "by prefix"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let path = exported_trajectory(&setup.scratch.path("home"), &output);
    let output = setup.umbra(&["-m", &model, "--resume", &slash(&path), "by path"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let output = setup.umbra(&["-m", &model, "--resume", "999", "nothing"], "", false);
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output).contains("no session '999'"), "{}", text(&output));
}

#[test]
fn fork_copies_the_history_and_leaves_the_old_file_alone() {
    let setup = Setup::new("fork");
    let first = setup.first_run("one.json", json!([submit("x")]), "task");
    let id = session_id(&first);
    let old_path = setup.scratch.path("home").join(".umbra/trajectories").join(format!("{id}.traj.journal.jsonl"));
    let before = std::fs::read(&old_path).unwrap();
    let model = replay_model(&setup.scratch, "two.json", &json!([submit("y")]));
    let output = setup.umbra(&["-m", &model, "--resume", &id, "--fork", "try another way"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(std::fs::read(&old_path).unwrap(), before, "the old session is unchanged");
    let forked = read_trajectory(&exported_trajectory(&setup.scratch.path("home"), &output));
    assert_ne!(session_id(&forked), id);
    assert_eq!(forked["info"]["session"]["resumes"][0]["forked_from"], id.as_str());
}

#[test]
fn tool_calls_that_never_ran_get_a_result_on_resume() {
    let setup = Setup::new("repair");
    let first = setup.first_run("one.json", json!([call("echo a"), submit("x")]), "task");
    let path = setup.scratch.path("home").join(".umbra/trajectories").join(format!("{}.traj.json", session_id(&first)));
    let mut trajectory = first.clone();
    let messages = trajectory["messages"].as_array_mut().unwrap();
    messages.truncate(5);
    messages.remove(3);
    std::fs::write(&path, serde_json::to_string(&trajectory).unwrap()).unwrap();
    let model = replay_model(&setup.scratch, "two.json", &json!([submit("y")]));
    let output = setup.umbra(&["-m", &model, "--resume", &slash(&path), "carry on"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(text(&output).contains("2 tool calls that never ran were given a result"), "{}", text(&output));
    let request = setup.requests().last().unwrap().clone();
    assert_eq!(roles(&request), ["system", "user", "assistant", "tool", "assistant", "tool", "user"]);
    assert_eq!(request["messages"][3]["content"], "This action was not run: the session ended before it finished.");
}

#[test]
fn limits_count_from_the_resume_point() {
    let setup = Setup::new("limits");
    let first = setup.first_run("one.json", json!([call("echo 1"), call("echo 2"), submit("x")]), "task");
    let model = replay_model(&setup.scratch, "two.json", &json!([submit("y")]));
    let output = setup.umbra(&["-m", &model, "--resume", &session_id(&first), "--step-limit", "1", "one more"], "", false);
    assert_eq!(output.status.code(), Some(0), "a session with 3 calls can still take 1 more under --step-limit 1: {}", text(&output));
}

#[test]
fn the_sessions_command_lists_this_folders_sessions() {
    let setup = Setup::new("list");
    let first = setup.first_run("one.json", json!([submit("x")]), "list me please");
    let output = setup.sessions(&["--json", "--cwd", &slash(&setup.work)]);
    assert!(output.status.success(), "{}", text(&output));
    let listed: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(listed[0]["id"], session_id(&first).as_str());
    assert_eq!(listed[0]["exit_status"], "Submitted");
    assert_eq!(listed[0]["task"], "list me please");
    let elsewhere = setup.sessions(&["--json", "--cwd", &slash(&setup.scratch.root)]);
    assert_eq!(serde_json::from_slice::<Value>(&elsewhere.stdout).unwrap().as_array().unwrap().len(), 0);
    let all = setup.sessions(&["--all"]);
    assert!(text(&all).contains("list me please"), "{}", text(&all));
}

#[test]
fn images_are_sent_again_after_a_resume() {
    let setup = Setup::new("resume-images");
    write_png(&setup.scratch, "work/shot.png", 24, 24);
    let model = replay_model(&setup.scratch, "one.json", &json!([submit("x")]));
    let output = setup.umbra(&["-m", &model, "--cwd", &slash(&setup.work), "-i", "shot.png", "task"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    std::fs::remove_file(setup.work.join("shot.png")).unwrap();
    let id = session_id(&read_trajectory(&exported_trajectory(&setup.scratch.path("home"), &output)));
    let model = replay_model(&setup.scratch, "two.json", &json!([submit("y")]));
    let output = setup.umbra(&["-m", &model, "--resume", &id, "what was in the image?"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let request = setup.requests().last().unwrap().clone();
    assert_eq!(request["messages"][1]["images"][0]["width"], 24, "the image comes from the trajectory, not the deleted file");
}

#[test]
fn a_resume_without_a_task_asks_for_one() {
    let setup = Setup::new("resume-ask");
    let first = setup.first_run("one.json", json!([submit("x")]), "task");
    let model = replay_model(&setup.scratch, "two.json", &json!([submit("y")]));
    let output = setup.umbra(&["-m", &model, "--resume", &session_id(&first)], "the typed task\n\n", true);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(text(&output).contains("What next?"), "{}", text(&output));
    let request = setup.requests().last().unwrap().clone();
    assert_eq!(request["messages"].as_array().unwrap().last().unwrap()["content"], "the typed task");
    let output = setup.umbra(&["-m", &model, "--resume", &session_id(&first)], "", false);
    assert_eq!(output.status.code(), Some(2), "with no terminal and no task it fails: {}", text(&output));
}

#[test]
fn each_step_is_appended_to_the_journal_and_a_cut_journal_still_resumes() {
    let setup = Setup::new("journal-cut");
    let first = setup.first_run("one.json", json!([call("echo a"), call("echo b"), submit("x")]), "remember PINE-7");
    let id = session_id(&first);
    let trajectory = setup.scratch.path("home").join(".umbra/trajectories").join(format!("{id}.traj.json"));
    let journal = setup.scratch.path("home").join(".umbra/trajectories").join(format!("{id}.traj.journal.jsonl"));
    let lines = std::fs::read_to_string(&journal).unwrap();
    assert_eq!(lines.lines().filter(|l| l.contains("\"t\":\"msg\"")).count(), first["messages"].as_array().unwrap().len(), "each message is written to the journal once");
    // A crash in the middle of a step: no trajectory written for the turn, and the journal's
    // last line cut off part way.
    let _ = std::fs::remove_file(&trajectory);
    let bytes = std::fs::read(&journal).unwrap();
    std::fs::write(&journal, &bytes[..bytes.len() - 20]).unwrap();
    let model = replay_model(&setup.scratch, "two.json", &json!([submit("y")]));
    let output = setup.umbra(&["-m", &model, "--resume", &id, "carry on"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let request = setup.requests().last().unwrap().clone();
    assert_eq!(request["messages"][1]["content"].as_str().map(|t| t.contains("PINE-7")), Some(true), "the history survived the cut");
}

#[test]
fn a_branch_at_message_n_keeps_the_first_n_messages() {
    let setup = Setup::new("branch-at");
    let first = setup.first_run("one.json", json!([call("echo a"), call("echo b"), submit("x")]), "task");
    let id = session_id(&first);
    let model = replay_model(&setup.scratch, "two.json", &json!([submit("y")]));
    let output = setup.umbra(&["-m", &model, "--resume", &id, "--at", "4", "--fork", "another way"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let request = setup.requests().last().unwrap().clone();
    assert_eq!(roles(&request), ["system", "user", "assistant", "tool", "user"], "4 messages kept, then the new task");
    let forked = read_trajectory(&exported_trajectory(&setup.scratch.path("home"), &output));
    assert_ne!(session_id(&forked), id, "a branch is a new session");
    let output = setup.umbra(&["-m", &model, "--resume", &id, "--at", "4", "again"], "", false);
    assert_ne!(output.status.code(), Some(0), "--at needs --fork");
}

#[test]
fn a_run_without_output_writes_only_the_journal_and_export_writes_the_json() {
    let setup = Setup::new("journal-only");
    let model = replay_model(&setup.scratch, "one.json", &json!([call("echo a"), submit("done")]));
    let output = setup.umbra(&["-m", &model, "--cwd", &slash(&setup.work), "task"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let journal = session_line(&output);
    assert!(journal.to_string_lossy().ends_with(".traj.journal.jsonl"), "the summary names the journal: {}", journal.display());
    let name = journal.to_string_lossy().to_string();
    let json_path = PathBuf::from(format!("{}.json", name.strip_suffix(".journal.jsonl").unwrap()));
    assert!(!json_path.exists(), "no trajectory JSON is written without -o");
    let out = setup.scratch.path("exported.json");
    let exported = run_umbra_with(&setup.scratch.path("home"), &["export", &name, "-o", &slash(&out)], &[], "");
    assert_eq!(exported.status.code(), Some(0), "{}", text(&exported));
    let trajectory = read_trajectory(&out);
    assert_eq!(trajectory["trajectory_format"], "mini-swe-agent-1.1");
    assert_eq!(trajectory["info"]["submission"].as_str().unwrap().trim(), "done");
    assert_eq!(trajectory["messages"].as_array().unwrap().iter().filter(|m| m["role"] == "tool").count(), 2);
    let with_o = setup.scratch.path("asked.traj.json");
    let model = replay_model(&setup.scratch, "two.json", &json!([submit("again")]));
    let output = setup.umbra(&["-m", &model, "--cwd", &slash(&setup.work), "-o", &slash(&with_o), "task"], "", false);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(with_o.is_file(), "-o still writes the trajectory JSON");
}
