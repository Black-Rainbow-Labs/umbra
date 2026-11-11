//! A real Ctrl+C in a real Windows console. Umbra runs in a console of its own, reading its input
//! with `ReadConsoleW`, the way it does in a terminal. A helper process (this test binary,
//! started with `UMBRA_CONSOLE_HELPER` set) attaches to that console, sends `CTRL_C_EVENT` to
//! every process on it, and types keys into it.
//!
//! `CTRL_C_EVENT` reaches every process attached to the console, so this also checks that the
//! shell command Umbra started is not killed by the key press itself, only by Umbra.
//!
//! The tests open a console window for a few seconds, so they are ignored by default:
//!
//! ```text
//! cargo test --test console_windows -- --ignored --test-threads 1
//! ```

#![cfg(windows)]

mod common;

use std::os::windows::process::CommandExt;
use std::time::{Duration, Instant};

use common::{read_trajectory, replay_model, slash, umbra_command, Scratch};
use serde_json::{json, Value};
use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows_sys::Win32::System::Console::{AttachConsole, FreeConsole, GenerateConsoleCtrlEvent, GetConsoleMode, SetConsoleCtrlHandler, WriteConsoleInputW, CTRL_C_EVENT, INPUT_RECORD, KEY_EVENT};

/// `CREATE_NEW_CONSOLE`: the child gets a console of its own.
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

/// When `UMBRA_CONSOLE_HELPER` is set to `<pid> <action>`, this test attaches to that process's
/// console and does the action: `ctrlc`, `type:<text>` which types the text and Enter, or
/// `getmode:<file>` which writes the console's input mode to the file.
#[test]
fn console_helper() {
    let Ok(request) = std::env::var("UMBRA_CONSOLE_HELPER") else { return };
    let (pid, action) = request.split_once(' ').expect("pid and action");
    let pid: u32 = pid.parse().expect("a pid");
    // SAFETY: plain Win32 console calls on this helper process's own console attachment.
    unsafe {
        FreeConsole();
        assert!(AttachConsole(pid) != 0, "could not attach to the console of {pid}");
        if action == "ctrlc" {
            SetConsoleCtrlHandler(None, 1);
            assert!(GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0) != 0);
        } else if let Some(text) = action.strip_prefix("type:") {
            type_into_console(&format!("{text}\r"));
        } else if let Some(file) = action.strip_prefix("getmode:") {
            std::fs::write(file, console_input_mode().to_string()).unwrap();
        }
        std::thread::sleep(Duration::from_millis(300));
        FreeConsole();
    }
    std::process::exit(0);
}

/// The attached console's input mode.
unsafe fn console_input_mode() -> u32 {
    let name: Vec<u16> = "CONIN$\0".encode_utf16().collect();
    // SAFETY: opening the attached console's input buffer by its reserved name and reading its mode.
    unsafe {
        let input = CreateFileW(name.as_ptr(), GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut());
        assert!(input != INVALID_HANDLE_VALUE, "could not open CONIN$");
        let mut mode = 0;
        assert!(GetConsoleMode(input, &mut mode) != 0, "could not read the console mode");
        mode
    }
}

/// Writes key presses for the text into the attached console's input buffer.
/// @param text - the characters to type; `\r` is Enter
unsafe fn type_into_console(text: &str) {
    let name: Vec<u16> = "CONIN$\0".encode_utf16().collect();
    // SAFETY: opening the attached console's input buffer by its reserved name.
    let input = unsafe { CreateFileW(name.as_ptr(), GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut()) };
    assert!(input != INVALID_HANDLE_VALUE, "could not open CONIN$");
    let mut records = Vec::new();
    for unit in text.encode_utf16() {
        for down in [1, 0] {
            let mut record = INPUT_RECORD { EventType: KEY_EVENT as u16, ..Default::default() };
            record.Event.KeyEvent.bKeyDown = down;
            record.Event.KeyEvent.wRepeatCount = 1;
            record.Event.KeyEvent.wVirtualKeyCode = if unit == 13 { 0x0D } else { 0 };
            record.Event.KeyEvent.uChar.UnicodeChar = unit;
            records.push(record);
        }
    }
    let mut written = 0;
    // SAFETY: the records are fully initialized and the handle is the console input buffer.
    let ok = unsafe { WriteConsoleInputW(input, records.as_ptr(), records.len() as u32, &mut written) };
    assert!(ok != 0 && written as usize == records.len(), "typed {written} of {} key events", records.len());
}

/// Runs this test binary as the helper against umbra's console.
/// @param pid - umbra's process id
/// @param action - `ctrlc` or `type:<text>`
fn helper(pid: u32, action: &str) {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["console_helper", "--exact", "--nocapture", "--test-threads", "1"])
        .env("UMBRA_CONSOLE_HELPER", format!("{pid} {action}"))
        .creation_flags(0x0800_0000)
        .status()
        .unwrap();
    assert!(status.success(), "the console helper failed for {action}");
}

/// A umbra running in a console of its own.
struct ConsoleRun {
    /// `cmd /c start /wait`, which exits with umbra's exit code.
    launcher: std::process::Child,
    /// umbra itself, which the helper attaches to.
    pid: u32,
    trajectory: std::path::PathBuf,
}

/// Starts umbra in a console of its own on a replay script. It goes through `cmd /c start /wait`
/// because a child started from Rust is handed the parent's stdin, and umbra must read from its
/// own console for the test to mean anything.
/// @param scratch - the test folder
/// @param replies - the scripted replies
fn start_in_console(scratch: &Scratch, replies: Value) -> ConsoleRun {
    start_in_console_with(scratch, replies, "")
}

/// Starts umbra in a console of its own with extra options.
/// @param scratch - the test folder
/// @param replies - the scripted replies
/// @param extra - more options, written into the command line as they are
fn start_in_console_with(scratch: &Scratch, replies: Value, extra: &str) -> ConsoleRun {
    start_in_console_env(scratch, replies, extra, &[])
}

/// Starts umbra in a console of its own with extra options and environment variables.
/// @param scratch - the test folder
/// @param replies - the scripted replies
/// @param extra - more options, written into the command line as they are
/// @param envs - more environment variables
fn start_in_console_env(scratch: &Scratch, replies: Value, extra: &str, envs: &[(&str, &str)]) -> ConsoleRun {
    std::fs::create_dir_all(scratch.path("work")).unwrap();
    let model = replay_model(scratch, "script.json", &replies);
    let trajectory = scratch.path("run.traj.json");
    let exe = env!("CARGO_BIN_EXE_umbra").replace('/', "\\");
    let line = format!("/c start \"umbra-console-test\" /wait \"{exe}\" -m \"{model}\" -y --no-mcp --exit-immediately {extra} --cwd \"{}\" -o \"{}\" task", slash(&scratch.path("work")), slash(&trajectory));
    let mut settings = umbra_command(&scratch.path("home"), &[]);
    settings.envs(envs.iter().copied());
    let mut launch = std::process::Command::new("cmd");
    launch.raw_arg(line).envs(settings.get_envs().filter_map(|(k, v)| v.map(|v| (k.to_owned(), v.to_owned())))).creation_flags(CREATE_NEW_CONSOLE);
    launch.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let launcher = launch.spawn().expect("cmd starts");
    let pid = find_umbra(&trajectory);
    ConsoleRun { launcher, pid, trajectory }
}

/// Finds the umbra process whose command line names this test's trajectory.
/// @param trajectory - the trajectory path, unique to the test
fn find_umbra(trajectory: &std::path::Path) -> u32 {
    let needle = trajectory.parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
    let script = format!("Get-CimInstance Win32_Process -Filter \"Name='umbra.exe'\" | Where-Object {{ $_.CommandLine -like '*{needle}*' }} | Select-Object -First 1 -ExpandProperty ProcessId");
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(20) {
        let output = std::process::Command::new("pwsh").args(["-NoProfile", "-Command", &script]).output().unwrap();
        if let Ok(pid) = String::from_utf8_lossy(&output.stdout).trim().parse() {
            return pid;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!("umbra did not start");
}

/// Waits for the launcher to exit, killing it after `limit`. Its exit code is not umbra's:
/// `start /wait` does not pass it on, so the tests read the trajectory instead.
/// @param child - the running umbra
/// @param limit - the longest wait
fn wait_exit(child: &mut std::process::Child, limit: Duration) -> Option<i32> {
    let started = Instant::now();
    while started.elapsed() < limit {
        if let Some(status) = child.try_wait().unwrap() {
            return status.code();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    panic!("umbra did not exit within {limit:?}");
}

#[test]
#[ignore = "opens a console window"]
fn a_real_ctrl_c_stops_the_command_and_typed_keys_reach_the_comment_prompt() {
    let s = Scratch::new("console-comment");
    let sleep = json!({ "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "sleep 30" } }] });
    let submit = json!({ "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT && echo done" } }] });
    let mut run = start_in_console(&s, json!([sleep, submit]));
    std::thread::sleep(Duration::from_secs(3));
    helper(run.pid, "ctrlc");
    std::thread::sleep(Duration::from_secs(1));
    helper(run.pid, "type:typed in the console");
    wait_exit(&mut run.launcher, Duration::from_secs(20));
    assert_eq!(read_trajectory(&run.trajectory)["info"]["exit_status"], "Submitted");
    let messages = read_trajectory(&run.trajectory)["messages"].as_array().unwrap().clone();
    assert!(messages[3]["extra"]["exception_info"].as_str().unwrap().starts_with("The user interrupted this command (Ctrl+C)"), "{}", messages[3]);
    assert_eq!(messages[4]["content"], "Interrupted by user: typed in the console");
}

#[test]
#[ignore = "opens a console window"]
fn a_real_ctrl_c_at_the_comment_prompt_ends_the_run() {
    let s = Scratch::new("console-quit");
    let sleep = json!({ "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "sleep 30" } }] });
    let mut run = start_in_console(&s, json!([sleep]));
    std::thread::sleep(Duration::from_secs(3));
    helper(run.pid, "ctrlc");
    std::thread::sleep(Duration::from_secs(1));
    helper(run.pid, "ctrlc");
    wait_exit(&mut run.launcher, Duration::from_secs(20));
    assert_eq!(read_trajectory(&run.trajectory)["info"]["exit_status"], "Interrupted");
}

/// Reads the console's input mode while umbra's fullscreen view is up, then stops umbra.
/// @param extra - more options for umbra
fn mode_while_fullscreen(extra: &str) -> u32 {
    mode_while_fullscreen_env(extra, &[])
}

/// Reads the console's input mode while the fullscreen view is up, with environment variables.
/// @param extra - more options for umbra
/// @param envs - more environment variables
fn mode_while_fullscreen_env(extra: &str, envs: &[(&str, &str)]) -> u32 {
    let s = Scratch::new("console-mode");
    let sleep = json!({ "content": "", "tool_calls": [{ "name": "bash", "arguments": { "command": "sleep 20" } }] });
    let mut run = start_in_console_env(&s, json!([sleep]), extra, envs);
    std::thread::sleep(Duration::from_secs(3));
    let file = s.path("mode.txt");
    helper(run.pid, &format!("getmode:{}", file.display()));
    let mode: u32 = std::fs::read_to_string(&file).unwrap().trim().parse().unwrap();
    helper(run.pid, "ctrlc");
    std::thread::sleep(Duration::from_secs(1));
    helper(run.pid, "ctrlc");
    wait_exit(&mut run.launcher, Duration::from_secs(20));
    mode
}

/// The console flags the fullscreen view sets or clears.
const QUICK_EDIT: u32 = 0x0040;
const EXTENDED_FLAGS: u32 = 0x0080;
const MOUSE_INPUT: u32 = 0x0010;
const VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;

#[test]
#[ignore = "opens a console window"]
fn with_the_mouse_off_quick_edit_stays_on_so_a_drag_selects_text() {
    let mode = mode_while_fullscreen("--no-mouse");
    assert_eq!(mode & QUICK_EDIT, QUICK_EDIT, "quick edit is kept (a new console has it on): {mode:#x}");
    assert_eq!(mode & EXTENDED_FLAGS, EXTENDED_FLAGS, "{mode:#x}");
    assert_eq!(mode & VIRTUAL_TERMINAL_INPUT, VIRTUAL_TERMINAL_INPUT, "{mode:#x}");
}

#[test]
#[ignore = "opens a console window"]
fn with_the_mouse_on_quick_edit_is_off_and_mouse_input_stays_off() {
    let mode = mode_while_fullscreen_env("", &[("UMBRA_MOUSE", "1")]);
    assert_eq!(mode & QUICK_EDIT, 0, "conhost must not take the drag: {mode:#x}");
    assert_eq!(mode & MOUSE_INPUT, 0, "the mouse arrives as escape sequences, not console events: {mode:#x}");
    assert_eq!(mode & VIRTUAL_TERMINAL_INPUT, VIRTUAL_TERMINAL_INPUT, "{mode:#x}");
}

#[test]
#[ignore = "opens a console window"]
fn a_classic_console_window_keeps_quick_edit_unless_the_mouse_is_asked_for() {
    let mode = mode_while_fullscreen("");
    assert_eq!(mode & QUICK_EDIT, QUICK_EDIT, "in a conhost window the mouse is off by default: {mode:#x}");
}
