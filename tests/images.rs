//! Image input: `--image`, image paths typed in a task or a prompt, scaling, refusing files that
//! are not images, and what the model is sent in both action formats.

mod common;

use common::{read_trajectory, recorded_requests, replay_model, run_umbra_with, slash, text, write_png, Scratch};
use serde_json::{json, Value};

/// A reply that submits at once.
fn submit() -> Value {
    json!({ "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT" } }] })
}

/// Runs umbra in yolo mode with a replay script, recording the requests. Returns the output, the
/// recorded requests and the trajectory.
/// @param scratch - the test folder
/// @param replies - the scripted replies
/// @param args - the arguments after the common ones, ending with the task
/// @param input - what to type on stdin
/// @param envs - extra environment variables
fn run(scratch: &Scratch, replies: Value, args: &[&str], input: &str, envs: &[(&str, &str)]) -> (std::process::Output, Vec<Value>, Value) {
    let model = replay_model(scratch, "script.json", &replies);
    let (traj, record, work) = (scratch.path("run.traj.json"), scratch.path("record.jsonl"), scratch.path("work"));
    std::fs::create_dir_all(&work).unwrap();
    let mut all = vec!["-m", &model, "-y", "--no-mcp", "--cwd"];
    let (work, traj_text) = (slash(&work), slash(&traj));
    all.extend([work.as_str(), "-o", traj_text.as_str()]);
    all.extend(args);
    let record_text = slash(&record);
    let mut environment = vec![("UMBRA_REPLAY_RECORD", record_text.as_str())];
    environment.extend(envs);
    let output = run_umbra_with(&scratch.path("home"), &all, &environment, input);
    let trajectory = if traj.exists() { read_trajectory(&traj) } else { Value::Null };
    (output, recorded_requests(&record), trajectory)
}

/// The images a recorded request sent with message `index`.
/// @param request - one recorded request
/// @param index - the message position
fn images_of(request: &Value, index: usize) -> Vec<Value> {
    request["messages"][index]["images"].as_array().cloned().unwrap_or_default()
}

#[test]
fn the_image_flag_attaches_images_to_the_first_message() {
    let s = Scratch::new("image-flag");
    write_png(&s, "work/shot.png", 40, 30);
    write_png(&s, "work/other.png", 20, 10);
    let (output, requests, trajectory) = run(&s, json!([submit()]), &["-i", "shot.png", "--image", "other.png", "look at these"], "", &[]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let images = images_of(&requests[0], 1);
    assert_eq!(images.len(), 2, "{}", requests[0]);
    assert_eq!((images[0]["media_type"].as_str(), images[0]["width"].as_u64(), images[0]["height"].as_u64()), (Some("image/png"), Some(40), Some(30)));
    assert!(images[0]["source"].as_str().unwrap().ends_with("shot.png"));
    let saved = &trajectory["messages"][1]["images"][0];
    assert!(saved["data"].as_str().unwrap().len() > 40, "the base64 data is kept for resume");
    assert!(text(&output).contains("attached"), "{}", text(&output));
}

#[test]
fn image_paths_typed_in_the_task_are_attached() {
    let s = Scratch::new("image-paths");
    write_png(&s, "work/a.png", 8, 8);
    write_png(&s, "work/two words.png", 8, 8);
    s.write("work/notes.txt", "not an image");
    let task = "compare @a.png with \"two words.png\", ignore notes.txt and missing.png";
    let (output, requests, _) = run(&s, json!([submit()]), &[task], "", &[]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let sources: Vec<String> = images_of(&requests[0], 1).iter().map(|i| i["source"].as_str().unwrap().replace('\\', "/")).collect();
    assert_eq!(sources.len(), 2, "{sources:?}");
    assert!(sources[0].ends_with("/a.png") && sources[1].ends_with("/two words.png"), "{sources:?}");
    assert!(requests[0]["messages"][1]["content"].as_str().unwrap().contains("@a.png"), "the text is sent unchanged");
}

#[test]
fn big_images_are_scaled_down_and_small_limits_can_be_set() {
    let s = Scratch::new("image-scale");
    write_png(&s, "work/big.png", 3000, 1500);
    let (output, requests, _) = run(&s, json!([submit()]), &["-i", "big.png", "task"], "", &[]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let image = &images_of(&requests[0], 1)[0];
    assert_eq!((image["width"].as_u64(), image["height"].as_u64()), (Some(1568), Some(784)));
    let config = s.write("small.toml", "[images]\nmax_long_edge = 500\n");
    let (output, requests, _) = run(&s, json!([submit()]), &["-c", &slash(&config), "-i", "big.png", "task"], "", &[]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(images_of(requests.last().unwrap(), 1)[0]["width"], 500, "the record file holds both runs, newest last");
}

#[test]
fn a_file_that_is_not_an_image_is_refused_before_any_model_call() {
    let s = Scratch::new("image-refused");
    s.write("work/fake.png", "I am text with a png name");
    let (output, requests, _) = run(&s, json!([submit()]), &["-i", "fake.png", "task"], "", &[]);
    assert_eq!(output.status.code(), Some(2), "{}", text(&output));
    assert!(text(&output).contains("not a PNG, JPEG, GIF or WebP image"), "{}", text(&output));
    assert!(requests.is_empty(), "no model call was made");
}

#[test]
fn a_bad_image_typed_in_a_task_is_skipped_with_a_notice() {
    let s = Scratch::new("image-skip");
    s.write("work/fake.png", "text");
    let (output, requests, _) = run(&s, json!([submit()]), &["look at fake.png"], "", &[]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(images_of(&requests[0], 1).is_empty());
    assert!(text(&output).contains("so it was not attached"), "{}", text(&output));
}

#[test]
fn images_work_in_text_mode() {
    let s = Scratch::new("image-text");
    write_png(&s, "work/shot.png", 16, 16);
    let reply = json!("THOUGHT: done.\n\n```umbra_bash\necho COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT\n```");
    let (output, requests, _) = run(&s, json!([reply]), &["--text", "-i", "shot.png", "task"], "", &[]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(images_of(&requests[0], 1).len(), 1);
}

#[test]
fn an_image_in_a_follow_up_task_is_attached_to_it() {
    let s = Scratch::new("image-follow-up");
    write_png(&s, "work/later.png", 12, 12);
    let args = ["first task"];
    let (output, requests, _) = run(&s, json!([submit(), submit()]), &args, "now look at later.png\n\n", &[("UMBRA_FORCE_INTERACTIVE", "1")]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    let messages = requests[1]["messages"].as_array().unwrap();
    let follow_up = messages.last().unwrap();
    assert_eq!(follow_up["content"], "The user added a new task: now look at later.png");
    assert_eq!(follow_up["images"][0]["width"], 12);
}
