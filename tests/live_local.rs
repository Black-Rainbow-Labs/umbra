//! End to end runs against a real local model. They are ignored by default; run them with
//!
//! ```text
//! cargo test --test live_local -- --ignored --test-threads 1
//! ```
//!
//! `UMBRA_LIVE_MODEL` picks the model, `local/ninfer-qwen3.8-27b-coletti-v9` by default, which is
//! the OpenAI compatible server on localhost:8080. The model must accept images.

mod common;

use std::time::{Duration, Instant};

use common::{read_trajectory, run_umbra_with, slash, text, Running, Scratch};
use serde_json::Value;

/// The longest a live run may take.
const LIVE_LIMIT: Duration = Duration::from_secs(600);

/// The model the live tests use.
fn live_model() -> String {
    std::env::var("UMBRA_LIVE_MODEL").unwrap_or_else(|_| "local/ninfer-qwen3.8-27b-coletti-v9".into())
}

/// Runs a task in yolo mode to the end and returns the output and the trajectory.
/// @param scratch - the test folder
/// @param args - the arguments after the common ones
/// @param trajectory - the trajectory file name
fn run_live(scratch: &Scratch, args: &[&str], trajectory: &str) -> (std::process::Output, Value) {
    let model = live_model();
    let (work, traj) = (slash(&scratch.path("work")), slash(&scratch.path(trajectory)));
    std::fs::create_dir_all(scratch.path("work")).unwrap();
    let mut all = vec!["-m", model.as_str(), "-y", "--no-mcp", "--exit-immediately", "--cwd", work.as_str(), "-o", traj.as_str()];
    all.extend(args);
    let output = run_umbra_with(&scratch.path("home"), &all, &[], "");
    keep_evidence(scratch, trajectory, &format!("{}-{}", scratch.root.file_name().unwrap().to_string_lossy(), trajectory.trim_end_matches(".traj.json")));
    let trajectory = read_trajectory(&scratch.path(trajectory));
    (output, trajectory)
}

/// Copies a trajectory to `UMBRA_LIVE_EVIDENCE`, when it names a folder, so a person can read
/// what the model was sent and what it answered.
/// @param scratch - the test folder
/// @param file - the trajectory file name in the test folder
/// @param name - the name to save it as
fn keep_evidence(scratch: &Scratch, file: &str, name: &str) {
    let Ok(dir) = std::env::var("UMBRA_LIVE_EVIDENCE") else { return };
    std::fs::create_dir_all(&dir).unwrap();
    let _ = std::fs::copy(scratch.path(file), std::path::Path::new(&dir).join(format!("{name}.traj.json")));
}

/// The submission of a trajectory, in upper case.
/// @param trajectory - the trajectory JSON
fn submission(trajectory: &Value) -> String {
    trajectory["info"]["submission"].as_str().unwrap_or("").to_uppercase()
}

/// Writes a PNG of three vertical stripes: red, green and blue from left to right.
/// @param scratch - the test folder
fn write_stripes(scratch: &Scratch) -> std::path::PathBuf {
    let path = scratch.path("work/stripes.png");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let image = image::RgbImage::from_fn(300, 120, |x, _| match x / 100 {
        0 => image::Rgb([220, 20, 20]),
        1 => image::Rgb([20, 180, 40]),
        _ => image::Rgb([30, 60, 220]),
    });
    image.save(&path).unwrap();
    path
}

#[test]
#[ignore = "needs the local model on localhost:8080"]
fn live_the_model_reads_an_attached_image() {
    let s = Scratch::new("live-image");
    write_stripes(&s);
    let task = "Look at the attached image stripes.png. Name the colors of its three vertical stripes from left to right, then submit them as one line such as `color1, color2, color3`. Do not run any other command.";
    let (output, trajectory) = run_live(&s, &["-i", "stripes.png", task], "run.traj.json");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let answer = submission(&trajectory);
    let (red, green, blue) = (answer.find("RED"), answer.find("GREEN"), answer.find("BLUE"));
    assert!(red.is_some() && green.is_some() && blue.is_some(), "answer: {answer}");
    assert!(red < green && green < blue, "the stripes are in order: {answer}");
}

#[test]
#[ignore = "needs the local model on localhost:8080"]
fn live_a_resumed_session_remembers_the_first_run() {
    let s = Scratch::new("live-resume");
    let first_task = "Remember this code word for later: PINEAPPLE-73. Do not run any command except the one that submits. Submit the word OK.";
    let (output, first) = run_live(&s, &[first_task], "run.traj.json");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let id = first["info"]["session"]["id"].as_str().unwrap().to_string();
    let model = live_model();
    let second_task = "What was the code word I asked you to remember? Submit just the code word.";
    let output = run_umbra_with(&s.path("home"), &["-m", &model, "-y", "--no-mcp", "--exit-immediately", "--resume", &id, second_task], &[], "");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    keep_evidence(&s, "run.traj.json", "live-resume-after-resume");
    let second = read_trajectory(&s.path("run.traj.json"));
    assert!(submission(&second).contains("PINEAPPLE-73"), "answer: {}", submission(&second));
    assert_eq!(second["info"]["session"]["resumes"].as_array().unwrap().len(), 1);
}

#[test]
#[ignore = "needs the local model on localhost:8080"]
fn live_ctrl_c_while_the_model_writes_then_a_comment_redirects_it() {
    let s = Scratch::new("live-interrupt-model");
    std::fs::create_dir_all(s.path("work")).unwrap();
    let model = live_model();
    let (work, traj) = (slash(&s.path("work")), slash(&s.path("run.traj.json")));
    let task = "Write a 3000 word essay about the history of rivers in your reply text, then submit the word ESSAY.";
    let args = ["-m", model.as_str(), "-y", "--no-mcp", "--exit-immediately", "--cwd", work.as_str(), "-o", traj.as_str(), task];
    let mut running = Running::start(&s.path("home"), &args, &[("UMBRA_FORCE_INTERACTIVE", "1")]);
    std::thread::sleep(Duration::from_secs(4));
    assert!(running.ctrl_c(), "this machine cannot send a console event");
    std::thread::sleep(Duration::from_millis(500));
    let redirected = Instant::now();
    running.type_line("Stop the essay. Do not write it. Instead submit exactly the word REDIRECTED.");
    let output = running.finish(LIVE_LIMIT);
    keep_evidence(&s, "run.traj.json", "live-interrupt-model");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let trajectory = read_trajectory(&s.path("run.traj.json"));
    assert!(submission(&trajectory).contains("REDIRECTED"), "answer: {}\n{}", submission(&trajectory), text(&output));
    let messages = trajectory["messages"].as_array().unwrap();
    assert_eq!(messages[2]["extra"]["interrupt_type"], "UserInterruption", "no reply was kept from the interrupted call: {}", messages[2]);
    eprintln!("finished {:?} after the comment", redirected.elapsed());
}

#[test]
#[ignore = "needs the local model on localhost:8080"]
fn live_ctrl_c_stops_a_running_command_and_the_model_recovers() {
    let s = Scratch::new("live-interrupt-command");
    std::fs::create_dir_all(s.path("work")).unwrap();
    let model = live_model();
    let (work, traj) = (slash(&s.path("work")), slash(&s.path("run.traj.json")));
    let task = "Run exactly this one command: `echo started > started.txt && sleep 120 && echo slept > slept.txt`. After it finishes, submit the word DONE.";
    let args = ["-m", model.as_str(), "-y", "--no-mcp", "--exit-immediately", "--cwd", work.as_str(), "-o", traj.as_str(), task];
    let mut running = Running::start(&s.path("home"), &args, &[("UMBRA_FORCE_INTERACTIVE", "1")]);
    let started = Instant::now();
    while !s.path("work/started.txt").exists() {
        assert!(started.elapsed() < LIVE_LIMIT, "the model never ran the command");
        std::thread::sleep(Duration::from_millis(200));
    }
    std::thread::sleep(Duration::from_secs(1));
    assert!(running.ctrl_c(), "this machine cannot send a console event");
    std::thread::sleep(Duration::from_millis(500));
    running.type_line("I stopped that command on purpose. Do not run it or any sleep again. Submit exactly the word RECOVERED.");
    let output = running.finish(LIVE_LIMIT);
    keep_evidence(&s, "run.traj.json", "live-interrupt-command");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let trajectory = read_trajectory(&s.path("run.traj.json"));
    assert!(submission(&trajectory).contains("RECOVERED"), "answer: {}\n{}", submission(&trajectory), text(&output));
    let interrupted = trajectory["messages"].as_array().unwrap().iter().any(|m| m["extra"]["exception_info"].as_str().is_some_and(|e| e.starts_with("The user interrupted this command")));
    assert!(interrupted, "the command's result says it was interrupted");
    assert!(!s.path("work/slept.txt").exists(), "the sleep was killed");
}
