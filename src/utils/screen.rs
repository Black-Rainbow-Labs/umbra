//! The fullscreen view's terminal side, like Claude Code's fullscreen mode. Umbra opens on the
//! terminal's alternate screen, so the shell's earlier output is not in view, and leaving puts the
//! shell's screen back as it was. The view itself, a transcript of blocks with overlays, lives in
//! `crate::ui`; this file turns the terminal's modes on and off, draws frames, redraws after a
//! resize and on a ticker while something animates, and hands keys and mouse reports to the view.

use std::cell::Cell;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use super::keys::Key;
use super::status_bar::StatusInfo;
use super::terminal_mode::{enable_raw, reassert_raw, restore, terminal_height};
use crate::ui::block::Block;
use crate::ui::frame::render_frame;
use crate::ui::overlay::Overlay;
use crate::ui::transcript::Transcript;
pub use crate::ui::view::KeyOutcome;
use crate::ui::view::View;

/// Alternate screen, alternate scroll (mode 1007: a terminal that ignores the mouse modes sends the
/// wheel as Up and Down arrows), bracketed paste, then a clear screen.
const ENTER: &str = "\x1b[?1049h\x1b[?1007h\x1b[?2004h\x1b[H\x1b[2J";

/// Mouse reporting: presses, releases and drags (mode 1002) in SGR coordinates (mode 1006).
const MOUSE_ON: &str = "\x1b[?1002h\x1b[?1006h";

/// The reverse of `ENTER` and `MOUSE_ON`, with the cursor shown, back on the main screen. The mouse
/// modes are always turned off, so no exit path leaves the shell receiving mouse reports.
const LEAVE: &str = "\x1b[?1006l\x1b[?1002l\x1b[?1000l\x1b[?2004l\x1b[?1007l\x1b[0m\x1b[?25h\x1b[?1049l";

/// True while the fullscreen view is on.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// The view's state while it is on.
static VIEW: Mutex<Option<View>> = Mutex::new(None);

/// Wakes the ticker when something starts to animate.
static TICK: (Mutex<()>, Condvar) = (Mutex::new(()), Condvar::new());

thread_local! {
    /// Set while code catches its own panics, so the panic hook leaves the screen alone.
    static QUIET_PANICS: Cell<bool> = const { Cell::new(false) };
}

/// Lays out rows for a screen width. Kept for blocks that draw themselves with a function.
pub type Layout = Arc<dyn Fn(usize) -> Vec<String> + Send + Sync>;

/// Returns true while the fullscreen view is on.
pub fn is_active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

/// True when `UMBRA_FORCE_FULLSCREEN=1` asks for the fullscreen view over pipes, which is how the
/// tests drive it. The terminal's own modes are then left alone.
pub fn forced() -> bool {
    std::env::var("UMBRA_FORCE_FULLSCREEN").is_ok_and(|v| v == "1")
}

/// Opens the fullscreen view: raw input, the alternate screen, mouse reporting when wanted, a
/// thread that redraws after a resize, and the ticker. Returns false, and changes nothing, when
/// raw input mode cannot be turned on.
/// @param mouse - true to turn mouse reporting on
/// @param color - true to draw ANSI colors
pub fn enter(mouse: bool, color: bool) -> bool {
    if is_active() {
        return true;
    }
    if !enable_raw(mouse) && !forced() {
        return false;
    }
    *VIEW.lock().unwrap_or_else(|e| e.into_inner()) = Some(View::new(color));
    write_raw(ENTER);
    if mouse {
        write_raw(MOUSE_ON);
    }
    ACTIVE.store(true, Ordering::SeqCst);
    install_panic_hook();
    std::thread::spawn(watch_size);
    std::thread::spawn(tick);
    redraw();
    true
}

/// Closes the fullscreen view and gives the shell its screen back. Safe to call twice.
pub fn leave() {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    VIEW.lock().unwrap_or_else(|e| e.into_inner()).take();
    write_raw(LEAVE);
    restore();
    poke();
}

/// Leaves the alternate screen without taking any lock, for a panic or a forced exit, where the
/// lock may be held by the thread that stopped.
pub fn leave_now() {
    if ACTIVE.swap(false, Ordering::SeqCst) {
        write_raw(LEAVE);
        restore();
    }
}

/// Wakes the ticker, after something started that animates, such as a running command.
pub fn poke() {
    TICK.1.notify_all();
}

/// Runs code that may panic on purpose and catches the panic itself, such as a layout of odd
/// markdown, without the panic hook leaving the screen.
/// @param run - the code
pub fn quiet_panics<R>(run: impl FnOnce() -> R) -> R {
    QUIET_PANICS.with(|q| q.set(true));
    let result = run();
    QUIET_PANICS.with(|q| q.set(false));
    result
}

/// Adds printed text to the transcript, one block per line, and draws the screen again.
/// @param text - one or more lines, with color codes
pub fn push_text(text: &str) {
    with_view(|view| {
        for line in text.split('\n') {
            view.push_block(Block::Text(line.trim_end_matches('\r').to_string()));
        }
    });
}

/// Adds a block to the transcript and returns its id, or None when the view is off.
/// @param block - the block
pub fn push_block(block: Block) -> Option<u64> {
    let mut id = None;
    with_view(|view| id = Some(view.push_block(block)));
    poke();
    id
}

/// Changes the transcript, such as a running command's state, and draws the screen again.
/// Returns what the change returned, or None when the view is off.
/// @param change - the change
pub fn with_transcript<R>(change: impl FnOnce(&mut Transcript) -> R) -> Option<R> {
    let mut result = None;
    with_view(|view| result = Some(change(&mut view.transcript)));
    poke();
    result
}

/// Removes every block, for `/clear`.
pub fn clear_transcript() {
    with_view(|view| {
        view.transcript.clear();
        view.scroll = 0;
        view.focus = None;
        view.selection = None;
    });
}

/// Shows the status bar's numbers, or none.
/// @param status - the numbers
pub fn set_status(status: Option<StatusInfo>) {
    with_view(|view| view.status = status);
}

/// Shows a prompt on the input line. An empty prompt shows the prompt for typing while the agent
/// works.
/// @param prompt - the prompt, with color codes
pub fn set_prompt(prompt: &str) {
    with_view(|view| view.prompt = prompt.to_string());
}

/// Shows the commands waiting for approval above the input, or clears them.
/// @param lines - the commands, one line each, already colored
pub fn set_pending(lines: Vec<String>) {
    with_view(|view| view.pending = lines);
}

/// Replaces what is typed on the input line.
/// @param text - the new text
pub fn set_input(text: &str) {
    with_view(|view| {
        view.input = text.chars().collect();
        view.cursor = view.input.len();
    });
}

/// Opens an overlay.
/// @param overlay - the overlay
pub fn open_overlay(overlay: Box<dyn Overlay>) {
    with_view(|view| view.overlay = Some(overlay));
}

/// Applies a key or a mouse report to the view, draws it again, then runs any work the key asked
/// for once the lock is released.
/// @param key - the key the person pressed
pub fn handle_key(key: Key) -> KeyOutcome {
    let mut outcome = KeyOutcome::None;
    let mut effects = Vec::new();
    with_view(|view| (outcome, effects) = view.apply_key(key));
    for effect in effects {
        effect();
    }
    poke();
    outcome
}

/// Runs a change on the view and draws it again. Does nothing when the view is off.
/// @param change - the change
fn with_view(change: impl FnOnce(&mut View)) {
    let mut guard = VIEW.lock().unwrap_or_else(|e| e.into_inner());
    let Some(view) = guard.as_mut() else { return };
    let size = screen_size();
    if size != view.size {
        view.size = size;
    }
    change(view);
    let frame = render_frame(view, size.0, size.1);
    write_raw(&frame);
}

/// Draws the screen again with no change.
pub fn redraw() {
    with_view(|_| {});
}

/// The screen size as (columns, rows).
fn screen_size() -> (usize, usize) {
    (super::console::terminal_width().max(10), terminal_height().max(4))
}

/// Redraws after a resize, and turns raw input back on when a program Umbra ran turned it off.
/// Ends when the view closes.
fn watch_size() {
    let mut last = screen_size();
    while is_active() {
        std::thread::sleep(Duration::from_millis(200));
        reassert_raw();
        let size = screen_size();
        if size != last {
            last = size;
            redraw();
        }
    }
}

/// The ticker: while something on screen animates (a spinner, the agent time, a starting MCP
/// server, the logo reveal, a copy note), draws a frame every 100 ms, or every `UMBRA_TICK_MS`.
/// Otherwise it waits to be woken and uses no CPU. `UMBRA_TICK_MS=0` turns it off.
fn tick() {
    let period = std::env::var("UMBRA_TICK_MS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(100);
    if period == 0 {
        return;
    }
    while is_active() {
        let animating = VIEW.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|v| v.animating(Instant::now()));
        if animating {
            redraw();
            std::thread::sleep(Duration::from_millis(period));
            continue;
        }
        let guard = TICK.0.lock().unwrap_or_else(|e| e.into_inner());
        let _ = TICK.1.wait_timeout(guard, Duration::from_millis(500));
    }
}

/// Makes a panic leave the alternate screen and turn the mouse modes off before the panic message
/// is printed, so the message is readable and the terminal is usable afterwards. A panic that the
/// code catches itself, inside `quiet_panics`, is not printed and leaves the screen alone.
fn install_panic_hook() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if QUIET_PANICS.with(|q| q.get()) {
                return;
            }
            leave_now();
            previous(info);
        }));
    });
}

/// Writes text to stdout at once.
/// @param text - the text, escape codes included
fn write_raw(text: &str) {
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.flush();
}

/// The columns a character takes: 2 for wide characters such as CJK and emoji, 0 for combining
/// marks and zero width characters, else 1.
/// @param c - the character
pub fn char_width(c: char) -> usize {
    let code = c as u32;
    match code {
        0x0300..=0x036f | 0x200b..=0x200f | 0xfe00..=0xfe0f => 0,
        0x1100..=0x115f | 0x2e80..=0xa4cf | 0xac00..=0xd7a3 | 0xf900..=0xfaff | 0xfe30..=0xfe4f | 0xff00..=0xff60 | 0xffe0..=0xffe6 | 0x1f300..=0x1faff | 0x20000..=0x3fffd => 2,
        _ => 1,
    }
}

/// The columns text takes on screen, not counting color codes.
/// @param text - the text
pub fn visible_width(text: &str) -> usize {
    let mut width = 0;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            super::ansi::copy_escape(&mut chars, &mut String::new());
            continue;
        }
        width += char_width(c);
    }
    width
}

/// Splits a line into rows of at most `width` columns. Color codes take no room and carry over:
/// each row ends with a reset, and the next row starts with the colors that were on. Tabs become
/// spaces to the next multiple of 8, and other control characters are dropped.
/// @param line - one line, with color codes
/// @param width - the most columns in a row
pub fn wrap_ansi(line: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut rows = Vec::new();
    let mut row = String::new();
    let mut used = 0;
    let mut colors = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            let mut sequence = String::from("\x1b");
            super::ansi::copy_escape(&mut chars, &mut sequence);
            if sequence.ends_with('m') && sequence.starts_with("\x1b[") {
                if sequence == "\x1b[0m" || sequence == "\x1b[m" { colors.clear() } else { colors.push_str(&sequence) }
            }
            row.push_str(&sequence);
            continue;
        }
        let (text, cells) = match c {
            '\t' => {
                let spaces = 8 - used % 8;
                (" ".repeat(spaces), spaces)
            }
            c if (c as u32) < 0x20 || c == '\x7f' => continue,
            c => (c.to_string(), char_width(c)),
        };
        if used + cells > width && used > 0 {
            if !colors.is_empty() {
                row.push_str("\x1b[0m");
            }
            rows.push(std::mem::take(&mut row));
            row.push_str(&colors);
            used = 0;
        }
        row.push_str(&text);
        used += cells;
    }
    rows.push(row);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::view::View;

    /// Removes color codes and the frame's control codes, keeping the row text.
    fn plain(frame: &str) -> Vec<String> {
        let body = frame.split("\x1b[H").nth(1).unwrap().split("\x1b[J").next().unwrap();
        body.split("\r\n").map(|row| regex::Regex::new("\x1b\\[[0-9;?]*[A-Za-z]").unwrap().replace_all(row, "").to_string()).collect()
    }

    fn view_with(lines: usize) -> View {
        let mut view = View::new(false);
        view.size = (40, 6);
        for n in 1..=lines {
            view.push_block(Block::Text(format!("line {n}")));
        }
        view.status = Some(StatusInfo { model: "m".into(), window: 100, ..Default::default() });
        view.prompt = "❯ ".into();
        view
    }

    #[test]
    fn wraps_by_columns_and_carries_colors_to_the_next_row() {
        assert_eq!(wrap_ansi("abcdef", 4), vec!["abcd", "ef"]);
        assert_eq!(wrap_ansi("", 4), vec![""]);
        assert_eq!(wrap_ansi("\x1b[36mabcdef\x1b[0m", 4), vec!["\x1b[36mabcd\x1b[0m", "\x1b[36mef\x1b[0m"]);
        assert_eq!(wrap_ansi("漢字漢", 4), vec!["漢字", "漢"]);
        assert_eq!(wrap_ansi("a\tb", 20), vec!["a       b"]);
        assert_eq!(visible_width("\x1b[1;33m❯\x1b[0m hi"), 4);
    }

    #[test]
    fn a_short_transcript_starts_at_the_top_and_the_input_is_at_the_bottom() {
        let mut view = view_with(2);
        view.insert("hello");
        let rows = plain(&render_frame(&mut view, 40, 6));
        assert_eq!(rows[..4], ["line 1", "line 2", "", ""]);
        assert!(rows[4].contains("0%") && rows[4].contains("0:00"), "{rows:?}");
        assert_eq!(rows[5], "❯ hello");
    }

    #[test]
    fn the_newest_lines_show_and_scrolling_up_keeps_its_place() {
        let mut view = view_with(20);
        assert_eq!(plain(&render_frame(&mut view, 40, 6))[..4], ["line 17", "line 18", "line 19", "line 20"]);
        view.apply_key(Key::PageUp);
        let rows = plain(&render_frame(&mut view, 40, 6));
        assert_eq!(rows[..3], ["line 15", "line 16", "line 17"]);
        assert!(rows[3].starts_with("↓ 3 more rows below"), "{rows:?}");
        view.push_block(Block::Text("line 21".into()));
        assert_eq!(plain(&render_frame(&mut view, 40, 6))[..3], ["line 15", "line 16", "line 17"], "new output does not move a scrolled view");
        view.apply_key(Key::Bottom);
        assert_eq!(plain(&render_frame(&mut view, 40, 6))[..4], ["line 18", "line 19", "line 20", "line 21"]);
        view.apply_key(Key::Top);
        assert_eq!(plain(&render_frame(&mut view, 40, 6))[0], "line 1");
    }

    #[test]
    fn the_input_line_edits_and_sends_on_enter() {
        let mut view = view_with(0);
        for key in [Key::Text("helo".into()), Key::Left, Key::Text("l".into()), Key::End, Key::Text("!".into())] {
            assert_eq!(view.apply_key(key).0, KeyOutcome::None);
        }
        assert_eq!(view.apply_key(Key::Enter).0, KeyOutcome::Line("hello!".into()));
        assert!(view.input.is_empty());
        assert_eq!(view.apply_key(Key::CtrlD).0, KeyOutcome::Eof);
        view.apply_key(Key::Paste("a\nb".into()));
        assert_eq!(plain(&render_frame(&mut view, 40, 6)).last().unwrap(), "❯ a↵b");
    }

    #[test]
    fn a_full_width_row_keeps_its_last_character() {
        // task-2590: each row ended with an erase to the end of the line. After a row as wide as
        // the screen the cursor waits on the last column, so that erase removed the last character.
        let mut view = view_with(0);
        view.push_block(Block::Text("x".repeat(40)));
        let frame = render_frame(&mut view, 40, 6);
        assert!(!frame.contains("\x1b[K"), "no row is erased after it is written: {frame:?}");
        assert!(frame.contains(&format!("\x1b[0m\x1b[2K{}\x1b[0m", "x".repeat(40))), "{frame:?}");
    }

    #[test]
    fn a_laid_out_block_follows_the_screen_width() {
        // task-2590: a new split pane made the screen narrower, and the banner drawn for the old
        // width wrapped into a jumble. A laid out block is drawn again for the new width.
        let mut view = view_with(0);
        view.push_block(Block::Laid(Arc::new(|width: usize| vec![format!("╭{}╮", "─".repeat(width - 2))])));
        let wide = plain(&render_frame(&mut view, 60, 6));
        assert_eq!(wide[0].chars().count(), 60);
        let narrow = plain(&render_frame(&mut view, 30, 6));
        assert_eq!(narrow[0], format!("╭{}╮", "─".repeat(28)));
        assert_eq!(narrow[1], "", "the block takes one row at the new width");
    }

    #[test]
    fn tab_moves_focus_to_a_group_and_enter_expands_it() {
        use crate::ui::block::{CommandRun, RunKind, RunState};
        let mut view = view_with(1);
        let mut run = CommandRun::running(RunKind::Shell { command: "ls".into() });
        run.state = RunState::Done;
        run.exit_code = Some(0);
        run.output = "a.txt".into();
        view.transcript.add_run(run);
        render_frame(&mut view, 40, 10);
        view.apply_key(Key::Tab);
        assert!(matches!(view.focus, Some(crate::ui::block::Target::Group(_))), "{:?}", view.focus);
        let rows = plain(&render_frame(&mut view, 40, 10));
        assert!(rows.iter().any(|r| r.starts_with("› ✔ Ran 1 shell command") || r.starts_with("›▸")), "{rows:?}");
        view.apply_key(Key::Enter);
        let rows = plain(&render_frame(&mut view, 40, 10));
        assert!(rows.iter().any(|r| r.contains("$ ls")), "{rows:?}");
    }
}
