//! Terminal output: colors, headings and reading a line from the person.

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};

use super::interrupt;
use super::keys::KeyDecoder;
use super::screen::{self, KeyOutcome};
use super::status_bar::{render_status, Phase, StatusInfo};

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

/// When set, the run prints every detail: step headings, whole outputs and every notice.
static VERBOSE: AtomicBool = AtomicBool::new(false);

/// Turns the detailed output on or off.
/// @param verbose - true to print every detail of the run
pub fn set_verbose(verbose: bool) {
    VERBOSE.store(verbose, Ordering::Relaxed);
}

/// Returns true when the detailed output is on.
pub fn is_verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

/// Returns true when stdout is a terminal.
pub fn stdout_is_terminal() -> bool {
    std::io::stdout().is_terminal()
}

/// Returns true when stdout is a terminal and `NO_COLOR` is not set.
pub fn color_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

/// Wraps text in an ANSI color code when color is on.
/// @param code - the ANSI SGR code, such as "36" for cyan
/// @param text - the text to color
pub fn paint(code: &str, text: &str) -> String {
    if color_enabled() { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
}

/// Prints text and a line break. While the status bar is on, the bar is erased first and drawn
/// again under the text, so it stays at the end of the output. Every line a run prints goes
/// through here or through `prompt_line`.
/// @param text - the text, without the final line break
pub fn emit(text: &str) {
    if screen::is_active() {
        screen::push_text(text);
        return;
    }
    let mut footer = FOOTER.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = String::new();
    footer.erase(&mut out);
    out.push_str(text);
    out.push('\n');
    footer.draw(&mut out);
    write_out(&out);
}

/// Prints text in place of the last `previous` lines printed on the main screen, so a banner can be
/// drawn again with a change: the cursor moves up over those lines and they are cleared. In the
/// fullscreen view the text is added like `emit`, because there the banner is a laid out block.
/// @param previous - how many lines the text replaces
/// @param text - one or more lines, with color codes
pub fn emit_replacing(previous: usize, text: &str) {
    if screen::is_active() {
        screen::push_text(text);
        return;
    }
    let mut footer = FOOTER.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = String::new();
    footer.erase(&mut out);
    if previous > 0 {
        out.push_str(&format!("\x1b[{previous}F\x1b[J"));
    }
    out.push_str(text);
    out.push('\n');
    footer.draw(&mut out);
    write_out(&out);
}

/// When set, everything the console writes goes to stderr. The ACP server sets it, because its
/// stdout carries the protocol and must hold nothing else.
static USE_STDERR: AtomicBool = AtomicBool::new(false);

/// Sends everything the console writes to stderr from now on.
pub fn use_stderr() {
    USE_STDERR.store(true, Ordering::SeqCst);
}

/// Writes text to stdout at once and flushes it, or to stderr after `use_stderr`.
/// @param text - the text, escape codes included
fn write_out(text: &str) {
    if USE_STDERR.load(Ordering::SeqCst) {
        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(text.as_bytes());
        let _ = stderr.flush();
        return;
    }
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.flush();
}

/// The status bar's state. The bar is drawn on the line under the cursor, so the cursor sits on
/// an empty line between the output and the bar, where a prompt is written and typed text echoes.
#[derive(Default)]
struct Footer {
    /// True once the run turned the bar on.
    on: bool,
    /// The numbers to show. Nothing is drawn before the first update.
    info: Option<StatusInfo>,
    /// True while the bar is on screen under the cursor.
    drawn: bool,
}

impl Footer {
    /// Adds the codes that erase the bar: back to the start of the line, then clear to the end of
    /// the screen. The cursor is then where the next output belongs. If the person pressed Enter
    /// after typing, the cursor moved down onto the bar's line, and this still erases the bar.
    /// @param out - the text being built for stdout
    fn erase(&mut self, out: &mut String) {
        if self.drawn {
            out.push_str("\r\x1b[J");
            self.drawn = false;
        }
    }

    /// Adds the codes that draw the bar on the next line and bring the cursor back to the start of
    /// the empty line above it. The cursor must be at the start of a line.
    /// @param out - the text being built for stdout
    fn draw(&mut self, out: &mut String) {
        let Some(info) = self.info.as_ref().filter(|_| self.on) else { return };
        out.push('\n');
        out.push_str(&render_status(info, terminal_width(), color_enabled()));
        out.push_str("\x1b[1A\r");
        self.drawn = true;
    }
}

/// The status bar, shared by everything that prints during a run.
static FOOTER: Mutex<Footer> = Mutex::new(Footer { on: false, info: None, drawn: false });

/// Turns the status bar on when it was asked for and the output can show it: stdout is a terminal
/// that understands ANSI codes and the run is not quiet. `UMBRA_STATUS_BAR=0` turns it off and
/// `UMBRA_STATUS_BAR=1` turns it on even without a terminal, which is how the tests read it.
/// Returns true when the bar is on.
/// @param wanted - false when the config or `--no-status-bar` turned it off
pub fn enable_status_bar(wanted: bool) -> bool {
    let forced = std::env::var("UMBRA_STATUS_BAR").ok();
    let on = match forced.as_deref() {
        Some("0") => false,
        Some("1") => wanted && !is_quiet(),
        _ => wanted && !is_quiet() && std::io::stdout().is_terminal() && std::env::var("TERM").map_or(true, |t| t != "dumb") && enable_ansi(),
    };
    FOOTER.lock().unwrap_or_else(|e| e.into_inner()).on = on;
    on
}

/// Set once the fullscreen view was used, so the run's last lines are not printed twice.
static USED_FULLSCREEN: AtomicBool = AtomicBool::new(false);

/// Opens the fullscreen view when it was asked for and the terminal can show it: stdin and stdout
/// are real terminals, the run is not quiet, `TERM` is not `dumb`, ANSI codes work, and
/// `UMBRA_FULLSCREEN` is not `0`. `UMBRA_FORCE_FULLSCREEN=1` opens it over pipes, for tests.
/// Returns true when the view opened.
/// @param wanted - false when the config or `--no-fullscreen` turned it off
/// @param mouse - true to turn mouse reporting on
pub fn start_fullscreen(wanted: bool, mouse: bool) -> bool {
    let allowed = std::env::var("UMBRA_FULLSCREEN").map_or(true, |v| v != "0");
    let terminal = screen::forced() || (std::io::stdin().is_terminal() && std::io::stdout().is_terminal() && std::env::var("TERM").map_or(true, |t| t != "dumb"));
    let opened = wanted && allowed && terminal && !is_quiet() && !USE_STDERR.load(Ordering::SeqCst) && enable_ansi() && screen::enter(mouse, color_enabled());
    if opened {
        USED_FULLSCREEN.store(true, Ordering::SeqCst);
    }
    opened
}

/// True once the fullscreen view was opened in this run.
pub fn used_fullscreen() -> bool {
    USED_FULLSCREEN.load(Ordering::SeqCst)
}

/// Returns true when the status bar is on.
pub fn status_bar_on() -> bool {
    FOOTER.lock().unwrap_or_else(|e| e.into_inner()).on
}

/// Shows new numbers in the status bar and draws it again where it is.
/// @param info - the numbers to show
pub fn update_status(info: StatusInfo) {
    let mut footer = FOOTER.lock().unwrap_or_else(|e| e.into_inner());
    if !footer.on {
        return;
    }
    if screen::is_active() {
        screen::set_status(Some(info.clone()));
        footer.info = Some(info);
        return;
    }
    footer.info = Some(info);
    let mut out = String::new();
    footer.erase(&mut out);
    footer.draw(&mut out);
    write_out(&out);
}

/// Ends the status bar: erases it and prints it once more as an ordinary last line, so the
/// scrollback keeps the session's final numbers.
/// @param phase - the phase the last line shows
pub fn finish_status(phase: Phase) {
    screen::leave();
    let mut footer = FOOTER.lock().unwrap_or_else(|e| e.into_inner());
    if !footer.on || USED_FULLSCREEN.load(Ordering::SeqCst) {
        footer.on = false;
        return;
    }
    let mut out = String::new();
    footer.erase(&mut out);
    if let Some(info) = footer.info.as_mut() {
        info.phase = phase;
        info.work_secs = crate::services::work_clock::elapsed().as_secs();
        info.working = false;
        out.push_str(&render_status(info, terminal_width(), color_enabled()));
        out.push('\n');
    }
    footer.on = false;
    write_out(&out);
}

/// The terminal's width in columns: asked of the console, then `COLUMNS`, then 100.
pub fn terminal_width() -> usize {
    console_width().or_else(|| std::env::var("COLUMNS").ok().and_then(|c| c.trim().parse().ok())).filter(|w| *w > 0).unwrap_or(100)
}

/// Asks the Windows console for the width of its visible window.
#[cfg(windows)]
fn console_width() -> Option<usize> {
    use windows_sys::Win32::System::Console::{GetConsoleScreenBufferInfo, GetStdHandle, CONSOLE_SCREEN_BUFFER_INFO, STD_OUTPUT_HANDLE};
    // SAFETY: the handle is the process's own stdout and the struct is written by the call.
    unsafe {
        let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
        if GetConsoleScreenBufferInfo(GetStdHandle(STD_OUTPUT_HANDLE), &mut info) == 0 {
            return None;
        }
        Some((info.srWindow.Right - info.srWindow.Left + 1).max(0) as usize)
    }
}

/// Asks the terminal for its width.
#[cfg(unix)]
fn console_width() -> Option<usize> {
    // SAFETY: TIOCGWINSZ writes a winsize struct for the terminal on stdout.
    unsafe {
        let mut size: libc::winsize = std::mem::zeroed();
        (libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) == 0 && size.ws_col > 0).then_some(size.ws_col as usize)
    }
}

/// No way to ask on other systems.
#[cfg(not(any(windows, unix)))]
fn console_width() -> Option<usize> {
    None
}

/// Turns on ANSI escape handling in a Windows console. A terminal that is not a console, such as
/// the one Git Bash's mintty provides, already handles them.
#[cfg(windows)]
fn enable_ansi() -> bool {
    use windows_sys::Win32::System::Console::{GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_OUTPUT_HANDLE};
    // SAFETY: the handle is the process's own stdout.
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0;
        if GetConsoleMode(handle, &mut mode) == 0 {
            return true;
        }
        mode & ENABLE_VIRTUAL_TERMINAL_PROCESSING != 0 || SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) != 0
    }
}

/// Unix terminals handle ANSI escapes.
#[cfg(not(windows))]
fn enable_ansi() -> bool {
    true
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

/// Sends each line of stdin through the channel, then None at the end of input. Inline, the
/// terminal collects a line and the bytes are split at line breaks. In the fullscreen view the
/// terminal is in raw mode, so the bytes are keys: they edit the view's input line, and Enter
/// sends it. On Windows a console event can end a pending read with an error or with no bytes;
/// when Ctrl+C was just pressed that is not the end of input, so the read is tried again.
/// @param sender - the channel to the prompts
fn read_stdin_lines(sender: Sender<Option<String>>) {
    let mut stdin = std::io::stdin();
    let mut interrupted_reads = 0;
    let mut buffer = [0u8; 4096];
    let mut partial: Vec<u8> = Vec::new();
    let mut keys = KeyDecoder::default();
    loop {
        let read = match std::io::Read::read(&mut stdin, &mut buffer) {
            Ok(0) | Err(_) if interrupted_reads < MAX_INTERRUPTED_READS && read_was_interrupted() => {
                interrupted_reads += 1;
                continue;
            }
            Ok(0) | Err(_) => 0,
            Ok(count) => count,
        };
        interrupted_reads = 0;
        let lines = if read == 0 {
            end_of_input(&mut partial)
        } else if screen::is_active() {
            keys_to_lines(&mut keys, &buffer[..read])
        } else {
            split_lines(&mut partial, &buffer[..read])
        };
        for line in lines {
            let ended = line.is_none();
            if sender.send(line).is_err() || ended {
                return;
            }
        }
    }
}

/// The lines left at the end of input: an unfinished last line, then the end marker.
/// @param partial - bytes read after the last line break
fn end_of_input(partial: &mut Vec<u8>) -> Vec<Option<String>> {
    let mut lines = Vec::new();
    if !partial.is_empty() {
        lines.push(Some(String::from_utf8_lossy(partial).trim_end_matches('\r').to_string()));
        partial.clear();
    }
    lines.push(None);
    lines
}

/// Adds bytes to the unfinished line and returns every line they complete, without its ending.
/// @param partial - bytes read after the last line break
/// @param bytes - the bytes just read
fn split_lines(partial: &mut Vec<u8>, bytes: &[u8]) -> Vec<Option<String>> {
    partial.extend_from_slice(bytes);
    let mut lines = Vec::new();
    while let Some(at) = partial.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = partial.drain(..=at).collect();
        lines.push(Some(String::from_utf8_lossy(&line).trim_end_matches(['\r', '\n']).to_string()));
    }
    lines
}

/// Applies the keys in the bytes to the fullscreen view and returns the lines sent with Enter,
/// and the end marker for Ctrl+D on an empty line.
/// @param keys - the key decoder
/// @param bytes - the bytes just read
fn keys_to_lines(keys: &mut KeyDecoder, bytes: &[u8]) -> Vec<Option<String>> {
    let mut lines = Vec::new();
    for key in keys.feed(bytes) {
        match screen::handle_key(key) {
            KeyOutcome::Line(line) => lines.push(Some(line)),
            KeyOutcome::Eof => lines.push(None),
            KeyOutcome::None => {}
        }
    }
    lines
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
    show_prompt(prompt);
    let input = wait_for_line();
    if screen::is_active() {
        // Nothing echoed the typed line, so it is added to the transcript after its prompt.
        if let Input::Line(line) = &input {
            let tail = prompt.rsplit('\n').next().unwrap_or("");
            if tail.trim() == "❯" {
                if !line.trim().is_empty() && crate::services::command_service::parse_builtin(line).is_none() {
                    screen::push_block(crate::ui::block::Block::UserPrompt { marker: "❯".into(), text: line.clone() });
                }
            } else {
                screen::push_text(&format!("{}{line}", paint("1;33", tail)));
            }
        }
        screen::set_prompt("");
        screen::set_pending(Vec::new());
    }
    input
}

/// Waits for one line, end of input, or Ctrl+C.
fn wait_for_line() -> Input {
    let receiver = lines().lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if interrupt::take() {
            if !screen::is_active() {
                write_out("\n");
            }
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

/// Prints a prompt with the cursor after it. While the status bar is on, the bar shows that Umbra
/// is waiting and is drawn under the prompt's last line, where the person types.
/// @param prompt - the prompt text, which may have several lines
fn show_prompt(prompt: &str) {
    let mut footer = FOOTER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(info) = footer.info.as_mut() {
        info.phase = Phase::Waiting;
    }
    let (head, tail) = match prompt.rfind('\n') {
        Some(index) => prompt.split_at(index + 1),
        None => ("", prompt),
    };
    if screen::is_active() {
        let status = footer.info.clone().filter(|_| footer.on);
        drop(footer);
        let head = head.trim_end_matches('\n');
        if !head.is_empty() {
            screen::push_text(&paint("1;33", head));
        }
        screen::set_status(status);
        screen::set_prompt(&paint("1;33", tail));
        return;
    }
    let mut out = String::new();
    footer.erase(&mut out);
    if !head.is_empty() {
        out.push_str(&paint("1;33", head));
    }
    footer.draw(&mut out);
    out.push_str(&paint("1;33", tail));
    write_out(&out);
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
    std::env::var("UMBRA_FORCE_INTERACTIVE").is_ok_and(|v| v == "1") || screen::forced() || std::io::stdin().is_terminal()
}
