//! Terminal output: colors, headings and reading a line from the person.

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};

use super::interrupt;

/// When set, the run prints nothing but its final answer and errors.
static QUIET: AtomicBool = AtomicBool::new(false);

/// Turns the step by step output off or on.
/// @param quiet - true to print only the final answer
pub fn set_quiet(quiet: bool) {
    QUIET.store(quiet, Ordering::Relaxed);
}

/// Returns true when the step by step output is off.
pub fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}

/// Returns true when stdout is a terminal and `NO_COLOR` is not set.
fn color_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

/// Wraps text in an ANSI color code when color is on.
/// @param code - the ANSI SGR code, such as "36" for cyan
/// @param text - the text to color
pub fn paint(code: &str, text: &str) -> String {
    if color_enabled() { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
}

/// Prints a heading line such as `── assistant (step 3, $0.0123) ──`.
/// @param label - the heading text
/// @param code - the ANSI color for the heading
pub fn heading(label: &str, code: &str) {
    println!("\n{}", paint(code, &format!("── {label} ──")));
}

/// Shortens text to at most `limit` characters by cutting out the middle.
/// @param text - the text to shorten
/// @param limit - the most characters to keep
pub fn clip(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        return text.to_string();
    }
    let half = limit / 2;
    let head: String = text.chars().take(half).collect();
    let tail: String = text.chars().skip(count - half).collect();
    format!("{head}\n... {} characters not shown ...\n{tail}", count - limit)
}

/// What a prompt got back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Line(String),
    /// Stdin was closed.
    Eof,
    /// Ctrl+C was pressed while Umbra waited.
    Interrupted,
}

/// The lines read by the stdin thread. None means end of input.
static LINES: OnceLock<Mutex<Receiver<Option<String>>>> = OnceLock::new();

/// Set when the end of input was taken off the channel by `take_queued_lines`, so a later prompt
/// still sees it.
static STDIN_CLOSED: AtomicBool = AtomicBool::new(false);

/// Starts the thread that owns stdin, once, and returns its channel. Reading on a thread lets a
/// prompt stop waiting when Ctrl+C is pressed, which a blocking read cannot do.
fn lines() -> &'static Mutex<Receiver<Option<String>>> {
    LINES.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || read_stdin_lines(sender));
        Mutex::new(receiver)
    })
}

/// Reads that may fail because a Ctrl+C cut them short before stdin counts as closed.
const MAX_INTERRUPTED_READS: u32 = 50;

/// Sends each line of stdin through the channel, then None at the end of input. On Windows a
/// console event can end a pending read with an error or with no bytes; when Ctrl+C was just
/// pressed that is not the end of input, so the read is tried again.
/// @param sender - the channel to the prompts
fn read_stdin_lines(sender: Sender<Option<String>>) {
    let stdin = std::io::stdin();
    let mut interrupted_reads = 0;
    loop {
        let mut line = String::new();
        let line = match stdin.read_line(&mut line) {
            Ok(0) | Err(_) if interrupted_reads < MAX_INTERRUPTED_READS && read_was_interrupted() => {
                interrupted_reads += 1;
                continue;
            }
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim_end_matches(['\r', '\n']).to_string()),
        };
        interrupted_reads = 0;
        let ended = line.is_none();
        if sender.send(line).is_err() || ended {
            return;
        }
    }
}

/// Returns true when a failed or empty read came from a Ctrl+C. The handler runs on its own
/// thread and may set the flag a moment after the read returns, so this waits briefly first.
fn read_was_interrupted() -> bool {
    std::thread::sleep(interrupt::POLL);
    interrupt::is_pending()
}

/// Starts the stdin thread if it is not running yet, so lines typed while the agent works are
/// read and queued instead of waiting in the console until the next prompt.
pub fn start_reading_stdin() {
    let _ = lines();
}

/// Takes every line typed since the last prompt without waiting, for the agent to add to the
/// conversation between steps. Empty lines are dropped. Returns nothing before the stdin thread
/// has started.
pub fn take_queued_lines() -> Vec<String> {
    let Some(lines) = LINES.get() else { return Vec::new() };
    let receiver = lines.lock().unwrap_or_else(|e| e.into_inner());
    let mut queued = Vec::new();
    loop {
        match receiver.try_recv() {
            Ok(Some(line)) if !line.trim().is_empty() => queued.push(line),
            Ok(Some(_)) => continue,
            Ok(None) => {
                STDIN_CLOSED.store(true, Ordering::SeqCst);
                break;
            }
            Err(_) => break,
        }
    }
    queued
}

/// Prints a prompt and waits for one line, end of input, or Ctrl+C. A Ctrl+C that is already
/// pending answers at once, and either way it is handled here.
/// @param prompt - the text shown before the cursor
pub fn prompt_line(prompt: &str) -> Input {
    print!("{}", paint("1;33", prompt));
    let _ = std::io::stdout().flush();
    let receiver = lines().lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if interrupt::take() {
            println!();
            return Input::Interrupted;
        }
        if STDIN_CLOSED.load(Ordering::SeqCst) {
            return Input::Eof;
        }
        match receiver.recv_timeout(interrupt::POLL) {
            Ok(Some(line)) => return Input::Line(line),
            Ok(None) | Err(RecvTimeoutError::Disconnected) => return Input::Eof,
            Err(RecvTimeoutError::Timeout) => continue,
        }
    }
}

/// Prints a prompt and reads one line. Returns None at end of input or on Ctrl+C.
/// @param prompt - the text shown before the cursor
pub fn read_line(prompt: &str) -> Option<String> {
    match prompt_line(prompt) {
        Input::Line(line) => Some(line),
        Input::Eof | Input::Interrupted => None,
    }
}

/// Returns true when stdin is a terminal, so Umbra can ask the person questions.
/// `UMBRA_FORCE_INTERACTIVE=1` treats a pipe as a terminal, which is how tests type into prompts.
pub fn stdin_is_terminal() -> bool {
    std::env::var("UMBRA_FORCE_INTERACTIVE").is_ok_and(|v| v == "1") || std::io::stdin().is_terminal()
}
