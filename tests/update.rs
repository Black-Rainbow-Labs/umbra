//! The update check and install against a fake release served on 127.0.0.1: `umbra update
//! --check`, `umbra update` replacing a copy of the binary after the checksum passes, a wrong
//! checksum refused, and the fullscreen prompt that installs and restarts into the same session.
//! The fake release's binary is a shell script, so these tests run on Unix only.

#![cfg(unix)]

mod common;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::screen::Screen;
use common::{read_trajectory, run_umbra_with, slash, text, Scratch};
use serde_json::json;
use sha2::{Digest, Sha256};

/// Files served by path, such as `/releases/latest`.
type Files = Arc<Mutex<HashMap<String, Vec<u8>>>>;

/// Serves `files` over HTTP on a free port until the test ends. Returns the base address.
/// @param files - the files by path
fn serve(files: Files) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap_or(0);
            let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
            let mut header = String::new();
            while reader.read_line(&mut header).is_ok_and(|n| n > 2) {
                header.clear();
            }
            let body = files.lock().unwrap().get(&path).cloned();
            let mut stream = stream;
            let (status, body) = match body {
                Some(body) => ("200 OK", body),
                None => ("404 Not Found", b"not found".to_vec()),
            };
            let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = stream.write_all(&body);
        }
    });
    base
}

/// The archive name for this machine, as the release names it.
fn archive_name() -> String {
    let target = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", _) => "x86_64-apple-darwin",
        (_, "aarch64") => "aarch64-unknown-linux-musl",
        _ => "x86_64-unknown-linux-musl",
    };
    format!("umbra-{target}.tar.gz")
}

/// A fake release 99.0.0 whose `umbra` is a script: `--version` prints the version, anything
/// else writes its arguments to `args_file`. Returns the files to serve and the script's text.
/// @param scratch - the test folder
/// @param base - the server's address
/// @param args_file - where the script writes the arguments it was started with
/// @param good_sum - false to publish a wrong checksum
fn fake_release(scratch: &Scratch, base: &str, args_file: &Path, good_sum: bool) -> (HashMap<String, Vec<u8>>, String) {
    let script = format!("#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"umbra 99.0.0\"; exit 0; fi\necho \"$@\" > \"{}\"\n", args_file.display());
    let staging = scratch.path("staging");
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::write(staging.join("umbra"), &script).unwrap();
    let archive = scratch.path(&archive_name());
    let status = std::process::Command::new("tar").arg("-czf").arg(&archive).arg("-C").arg(&staging).arg("umbra").status().unwrap();
    assert!(status.success());
    let bytes = std::fs::read(&archive).unwrap();
    let sum: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    let sum = if good_sum { sum } else { "0".repeat(64) };
    let name = archive_name();
    let release = json!({ "tag_name": "v99.0.0", "assets": [
        { "name": name, "browser_download_url": format!("{base}/dl/{name}") },
        { "name": "SHA256SUMS", "browser_download_url": format!("{base}/dl/SHA256SUMS") },
    ] });
    let mut files = HashMap::new();
    files.insert("/releases/latest".to_string(), release.to_string().into_bytes());
    files.insert(format!("/dl/{name}"), bytes);
    files.insert("/dl/SHA256SUMS".to_string(), format!("{sum}  {name}\n").into_bytes());
    (files, script)
}

/// Starts a server for a fake release and returns its address and the environment that points
/// Umbra at it and at a copy of a binary to replace.
/// @param scratch - the test folder
/// @param good_sum - false to publish a wrong checksum
fn release_env(scratch: &Scratch, good_sum: bool) -> (Vec<(String, String)>, PathBuf, String) {
    let files: Files = Arc::new(Mutex::new(HashMap::new()));
    let base = serve(files.clone());
    let (served, script) = fake_release(scratch, &base, &scratch.path("args.txt"), good_sum);
    *files.lock().unwrap() = served;
    let target = scratch.path("bin/umbra");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, "old binary").unwrap();
    let env = vec![("UMBRA_TEST_UPDATE_URL".to_string(), format!("{base}/releases")), ("UMBRA_UPDATE_TARGET".to_string(), slash(&target)), ("UMBRA_UPDATE_CHECK".to_string(), "1".to_string())];
    (env, target, script)
}

/// The environment as the slices the helpers take.
/// @param env - the variables
fn pairs(env: &[(String, String)]) -> Vec<(&str, &str)> {
    env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
}

#[test]
fn update_check_reports_a_newer_release_and_install_replaces_the_binary() {
    let scratch = Scratch::new("update-install");
    let (env, target, script) = release_env(&scratch, true);
    let home = scratch.path("home");
    let output = run_umbra_with(&home, &["update", "--check"], &pairs(&env), "");
    assert!(text(&output).contains("umbra 99.0.0 is available"), "{}", text(&output));
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "old binary", "--check installs nothing");
    let state: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(home.join(".umbra/update.json")).unwrap()).unwrap();
    assert_eq!(state["latest"], "99.0.0");
    let output = run_umbra_with(&home, &["update"], &pairs(&env), "");
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("checksum matches SHA256SUMS"), "{}", text(&output));
    assert_eq!(std::fs::read_to_string(&target).unwrap(), script, "the binary was replaced by the release's");
}

#[test]
fn a_wrong_checksum_is_refused_and_the_binary_is_kept() {
    let scratch = Scratch::new("update-bad-sum");
    let (env, target, _) = release_env(&scratch, false);
    let output = run_umbra_with(&scratch.path("home"), &["update"], &pairs(&env), "");
    assert!(!output.status.success());
    assert!(text(&output).contains("does not match SHA256SUMS"), "{}", text(&output));
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "old binary");
    let leftovers: Vec<_> = std::fs::read_dir(target.parent().unwrap()).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(leftovers.len(), 1, "no download folder is left behind: {leftovers:?}");
}

#[test]
fn the_prompt_installs_and_restarts_into_the_same_session() {
    let scratch = Scratch::new("update-prompt");
    let (env, _, _) = release_env(&scratch, true);
    let home = scratch.path("home");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    std::fs::create_dir_all(home.join(".umbra")).unwrap();
    std::fs::write(home.join(".umbra/update.json"), json!({ "checked_at": now, "latest": "99.0.0" }).to_string()).unwrap();
    std::fs::create_dir_all(scratch.path("work")).unwrap();
    let model = format!("replay/{}", slash(&scratch.write("script.json", &json!(["Hi."]).to_string())));
    let (work, traj) = (slash(&scratch.path("work")), slash(&scratch.path("run.traj.json")));
    let mut screen = Screen::start(&home, &["--cwd", &work, "-o", &traj, "-m", &model, "-y", "--no-mcp"], &pairs(&env), (100, 32));
    let frame = screen.wait_for("Umbra 99.0.0 is available");
    assert!(frame.text().contains("Install and restart"), "{}", frame.text());
    screen.type_text("h");
    screen.wait_until("the prompt closed by typing", Duration::from_secs(5), |f| !f.text().contains("is available") && f.input().contains("h"));
    screen.type_line("ello");
    screen.wait_for("● Hi.");
    screen.type_line("/update");
    let args_file = scratch.path("args.txt");
    let started = Instant::now();
    while !args_file.exists() && started.elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(50));
    }
    let args = std::fs::read_to_string(&args_file).unwrap_or_else(|_| panic!("the new binary was not started:\n{}", screen.frame().text()));
    let session = read_trajectory(&scratch.path("run.traj.json"))["info"]["session"]["id"].as_str().unwrap().to_string();
    assert!(args.contains(&format!("--resume {session}")), "{args}");
    assert!(args.contains("--yolo") && args.contains("--no-mcp") && args.contains(&format!("--cwd {work}")), "{args}");
}

#[test]
fn enter_on_the_prompt_installs_and_restarts() {
    let scratch = Scratch::new("update-prompt-enter");
    let (env, _, _) = release_env(&scratch, true);
    let home = scratch.path("home");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    std::fs::create_dir_all(home.join(".umbra")).unwrap();
    std::fs::write(home.join(".umbra/update.json"), json!({ "checked_at": now, "latest": "99.0.0" }).to_string()).unwrap();
    std::fs::create_dir_all(scratch.path("work")).unwrap();
    let model = format!("replay/{}", slash(&scratch.write("script.json", &json!(["Hi."]).to_string())));
    let work = slash(&scratch.path("work"));
    let mut screen = Screen::start(&home, &["--cwd", &work, "-m", &model, "-y", "--no-mcp"], &pairs(&env), (100, 32));
    screen.wait_for("Umbra 99.0.0 is available");
    screen.press("Right");
    screen.press("Left");
    screen.press("Enter");
    let args_file = scratch.path("args.txt");
    let started = Instant::now();
    while !args_file.exists() && started.elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(50));
    }
    let args = std::fs::read_to_string(&args_file).unwrap_or_else(|_| panic!("the new binary was not started:\n{}", screen.frame().text()));
    assert!(args.contains("--yolo") && !args.contains("--resume"), "no session existed yet: {args}");
}

#[test]
fn skip_this_version_is_remembered() {
    let scratch = Scratch::new("update-skip");
    let (env, _, _) = release_env(&scratch, true);
    let home = scratch.path("home");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    std::fs::create_dir_all(home.join(".umbra")).unwrap();
    std::fs::write(home.join(".umbra/update.json"), json!({ "checked_at": now, "latest": "99.0.0" }).to_string()).unwrap();
    std::fs::create_dir_all(scratch.path("work")).unwrap();
    let model = format!("replay/{}", slash(&scratch.write("script.json", &json!(["Hi."]).to_string())));
    let work = slash(&scratch.path("work"));
    let mut screen = Screen::start(&home, &["--cwd", &work, "-m", &model, "-y", "--no-mcp"], &pairs(&env), (100, 32));
    screen.wait_for("Umbra 99.0.0 is available");
    screen.press("Left");
    screen.press("Enter");
    screen.wait_until("the prompt to close", Duration::from_secs(5), |f| !f.text().contains("is available"));
    let started = Instant::now();
    let read = || std::fs::read_to_string(home.join(".umbra/update.json")).unwrap_or_default();
    while !read().contains("\"skipped\": \"99.0.0\"") && started.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(read().contains("\"skipped\": \"99.0.0\""), "{}", read());
}
