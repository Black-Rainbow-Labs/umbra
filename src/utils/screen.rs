//! The fullscreen view, like Claude Code's fullscreen mode. Umbra opens on the terminal's
//! alternate screen, so the shell's earlier output is not in view and is not reached by scrolling
//! up, and leaving puts the shell's screen back as it was. Everything Umbra prints is kept in a
//! transcript drawn above the status bar and the input line. PageUp, PageDown and the mouse wheel
//! scroll the transcript; new output does not move the view while it is scrolled up.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::keys::Key;
use super::terminal_mode::{enable_raw, reassert_raw, restore, terminal_height};

/// The most lines the transcript keeps. Older lines are dropped.
const MAX_LINES: usize = 20_000;

/// The most rows the input line may take at the bottom of the screen.
const MAX_INPUT_ROWS: usize = 5;

/// Rows the mouse wheel scrolls.
const WHEEL_ROWS: usize = 3;

/// Alternate screen, alternate scroll (mode 1007: the terminal sends the mouse wheel as Up and Down
/// arrows, which scroll the transcript), bracketed paste, then a clear screen. Mouse reporting
/// (mode 1000) is left off, because with it on a click goes to Umbra and the person cannot select
/// text without holding Shift.
const ENTER: &str = "\x1b[?1049h\x1b[?1007h\x1b[?2004h\x1b[H\x1b[2J";

/// The reverse of `ENTER`, with the cursor shown, back on the main screen.
const LEAVE: &str = "\x1b[?2004l\x1b[?1007l\x1b[0m\x1b[?25h\x1b[?1049l";

/// The prompt shown on the input line while the agent works and no question is asked.
const WORKING_PROMPT: &str = "› ";

/// True while the fullscreen view is on.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// The view's state while it is on.
static VIEW: Mutex<Option<View>> = Mutex::new(None);

/// Lays out rows for a screen width. The banner and the status bar are drawn this way, so a
/// resize (a split pane, a narrower window) lays them out again instead of wrapping the rows that
/// were drawn for the old width.
pub type Layout = Arc<dyn Fn(usize) -> Vec<String> + Send + Sync>;

/// One entry of the transcript.
enum Entry {
    /// A printed line, with color codes, wrapped to the screen width when drawn.
    Text(String),
    /// Rows laid out again for each screen width.
    Laid(Layout),
}

impl Entry {
    /// The rows this entry takes at a screen width. Rows wider than the screen are wrapped.
    /// @param width - the screen width
    fn rows(&self, width: usize) -> Vec<String> {
        match self {
            Entry::Text(line) => wrap_ansi(line, width),
            Entry::Laid(layout) => layout(width).iter().flat_map(|row| wrap_ansi(row, width)).collect(),
        }
    }
}

/// What the fullscreen view shows.
#[derive(Default)]
pub struct View {
    /// The transcript, one entry per printed line or laid out block.
    lines: VecDeque<Entry>,
    /// Lines dropped from the front of the transcript, so line numbers stay stable.
    dropped: usize,
    /// Rows of the transcript below the bottom of the view. 0 follows new output.
    scroll: usize,
    /// The status bar, laid out for the screen width when drawn. Only its first row is shown.
    status: Option<Layout>,
    /// The prompt on the input line, with color codes. Empty while the agent works.
    prompt: String,
    /// What the person is typing.
    input: Vec<char>,
    /// The cursor's position in `input`.
    cursor: usize,
    /// The line number where the last turn's summary starts, printed again when the view closes.
    epilogue: Option<usize>,
    /// The line number just after the last turn's summary.
    epilogue_end: Option<usize>,
    /// The size the last frame was drawn at.
    size: (usize, usize),
}

/// What a key did to the input line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Nothing to send.
    None,
    /// The person pressed Enter on this line.
    Line(String),
    /// The person ended input with Ctrl+D on an empty line.
    Eof,
}

/// Returns true while the fullscreen view is on.
pub fn is_active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

/// Opens the fullscreen view: raw input, the alternate screen, and a thread that redraws after a
/// resize. Returns false, and changes nothing, when raw input mode cannot be turned on.
pub fn enter() -> bool {
    if is_active() {
        return true;
    }
    if !enable_raw() {
        return false;
    }
    *VIEW.lock().unwrap_or_else(|e| e.into_inner()) = Some(View::default());
    write_raw(ENTER);
    ACTIVE.store(true, Ordering::SeqCst);
    install_panic_hook();
    std::thread::spawn(watch_size);
    redraw();
    true
}

/// Closes the fullscreen view and gives the shell its screen back, then prints the last turn's
/// summary on the main screen so the shell keeps how to resume the session. Safe to call twice.
pub fn leave() {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    let epilogue = VIEW.lock().unwrap_or_else(|e| e.into_inner()).take().map(|view| view.epilogue_lines()).unwrap_or_default();
    write_raw(LEAVE);
    restore();
    if !epilogue.is_empty() {
        write_raw(&format!("{}\n", epilogue.join("\n")));
    }
}

/// Leaves the alternate screen without taking any lock, for a panic or a forced exit, where the
/// lock may be held by the thread that stopped.
pub fn leave_now() {
    if ACTIVE.swap(false, Ordering::SeqCst) {
        write_raw(LEAVE);
        restore();
    }
}

/// Adds printed text to the transcript and draws the screen again.
/// @param text - one or more lines, with color codes
pub fn push_text(text: &str) {
    with_view(|view, width| view.push(text, width));
}

/// Shows a new status bar line, or none.
/// @param status - the status bar, already drawn as one line
pub fn set_status(status: Option<String>) {
    set_status_layout(status.map(|line| Arc::new(move |_: usize| vec![line.clone()]) as Layout));
}

/// Shows a status bar that is laid out again for each screen width, or none.
/// @param status - lays out the status bar for a width; the first row is shown
pub fn set_status_layout(status: Option<Layout>) {
    with_view(|view, _| view.status = status);
}

/// Adds rows to the transcript that are laid out again whenever the screen width changes, such
/// as the banner, and draws the screen again.
/// @param layout - lays out the rows for a width
pub fn push_layout(layout: Layout) {
    with_view(|view, width| {
        if view.scroll > 0 {
            view.scroll += Entry::Laid(layout.clone()).rows(width).len();
        }
        view.lines.push_back(Entry::Laid(layout));
    });
}

/// Shows a prompt on the input line. An empty prompt shows the prompt for typing while the
/// agent works.
/// @param prompt - the prompt, with color codes
pub fn set_prompt(prompt: &str) {
    with_view(|view, _| view.prompt = prompt.to_string());
}

/// Marks where the last turn's summary starts; those lines are printed again when the view closes.
pub fn mark_epilogue() {
    with_view(|view, _| {
        view.epilogue = Some(view.dropped + view.lines.len());
        view.epilogue_end = None;
    });
}

/// Marks where the last turn's summary ends, so the prompt that follows it is not printed again.
pub fn end_epilogue() {
    with_view(|view, _| view.epilogue_end = Some(view.dropped + view.lines.len()));
}

/// Applies a key to the input line or the scroll position, and draws the screen again.
/// @param key - the key the person pressed
pub fn handle_key(key: Key) -> KeyOutcome {
    let mut outcome = KeyOutcome::None;
    with_view(|view, _| {
        let (width, height) = view.size;
        outcome = view.apply_key(key, width, height);
    });
    outcome
}

/// Runs a change on the view and draws it again. Does nothing when the view is off.
/// @param change - the change, given the view and the screen width
fn with_view(change: impl FnOnce(&mut View, usize)) {
    let mut guard = VIEW.lock().unwrap_or_else(|e| e.into_inner());
    let Some(view) = guard.as_mut() else { return };
    let size = screen_size();
    view.size = size;
    change(view, size.0);
    write_raw(&render_frame(view, size.0, size.1));
}

/// Draws the screen again with no change.
fn redraw() {
    with_view(|_, _| {});
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

/// Makes a panic leave the alternate screen before the panic message is printed, so the message
/// is readable and the terminal is usable afterwards.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        leave_now();
        previous(info);
    }));
}

/// Writes text to stdout at once.
/// @param text - the text, escape codes included
fn write_raw(text: &str) {
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.flush();
}

impl View {
    /// Adds lines to the transcript. While scrolled up, the view stays on the same rows.
    /// @param text - one or more lines
    /// @param width - the screen width, to count the rows the new lines take
    fn push(&mut self, text: &str, width: usize) {
        for line in text.split('\n') {
            let line = line.trim_end_matches('\r').to_string();
            if self.scroll > 0 {
                self.scroll += wrap_ansi(&line, width).len();
            }
            self.lines.push_back(Entry::Text(line));
        }
        while self.lines.len() > MAX_LINES {
            self.lines.pop_front();
            self.dropped += 1;
        }
    }

    /// The lines of the last turn summary, or none.
    fn epilogue_lines(&self) -> Vec<String> {
        let Some(start) = self.epilogue else { return Vec::new() };
        let from = start.saturating_sub(self.dropped);
        let to = self.epilogue_end.map_or(self.lines.len(), |end| end.saturating_sub(self.dropped));
        let width = self.size.0.max(10);
        let lines = self.lines.iter().skip(from).take(to.saturating_sub(from));
        lines.flat_map(|entry| match entry { Entry::Text(line) => vec![line.clone()], Entry::Laid(layout) => layout(width) }).filter(|l| !l.trim().is_empty()).collect()
    }

    /// Applies one key.
    /// @param key - the key
    /// @param width - the screen width
    /// @param height - the screen height
    fn apply_key(&mut self, key: Key, width: usize, height: usize) -> KeyOutcome {
        let page = transcript_height(self, width, height).saturating_sub(1).max(1);
        match key {
            Key::Text(text) | Key::Paste(text) => self.insert(&text),
            Key::Enter => {
                let line: String = self.input.drain(..).collect();
                self.cursor = 0;
                return KeyOutcome::Line(line);
            }
            Key::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.input.remove(self.cursor);
            }
            Key::Delete if self.cursor < self.input.len() => {
                self.input.remove(self.cursor);
            }
            Key::CtrlD if self.input.is_empty() => return KeyOutcome::Eof,
            Key::CtrlD if self.cursor < self.input.len() => {
                self.input.remove(self.cursor);
            }
            Key::CtrlU => {
                self.input.clear();
                self.cursor = 0;
            }
            Key::Left => self.cursor = self.cursor.saturating_sub(1),
            Key::Right => self.cursor = (self.cursor + 1).min(self.input.len()),
            Key::Home => self.cursor = 0,
            Key::End => self.cursor = self.input.len(),
            Key::Up => self.scroll_by(1, width, height),
            Key::Down => self.scroll = self.scroll.saturating_sub(1),
            Key::PageUp => self.scroll_by(page, width, height),
            Key::PageDown => self.scroll = self.scroll.saturating_sub(page),
            Key::WheelUp => self.scroll_by(WHEEL_ROWS, width, height),
            Key::WheelDown => self.scroll = self.scroll.saturating_sub(WHEEL_ROWS),
            Key::Top => self.scroll_by(usize::MAX / 2, width, height),
            Key::Bottom | Key::Escape => self.scroll = 0,
            _ => {}
        }
        KeyOutcome::None
    }

    /// Inserts text at the cursor.
    /// @param text - the text
    fn insert(&mut self, text: &str) {
        for c in text.chars() {
            self.input.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    /// Scrolls up, no further than the top of the transcript.
    /// @param rows - rows to scroll
    /// @param width - the screen width
    /// @param height - the screen height
    fn scroll_by(&mut self, rows: usize, width: usize, height: usize) {
        let total: usize = self.lines.iter().map(|entry| entry.rows(width).len()).sum();
        // Once scrolled, the note under the transcript takes one more row.
        let room = transcript_height(self, width, height).saturating_sub(usize::from(self.scroll == 0)).max(1);
        self.scroll = self.scroll.saturating_add(rows).min(total.saturating_sub(room));
    }
}

/// The rows the input line takes: the prompt and the typed text, wrapped, at most
/// `MAX_INPUT_ROWS`. Returns the rows and the cursor's row and column within them.
/// @param view - the view
/// @param width - the screen width
fn input_rows(view: &View, width: usize) -> (Vec<String>, usize, usize) {
    let prompt = if view.prompt.is_empty() { super::console::paint("2", WORKING_PROMPT) } else { view.prompt.clone() };
    let shown = |chars: &[char]| chars.iter().map(|c| if *c == '\n' { '↵' } else { *c }).collect::<String>();
    let rows = wrap_ansi(&format!("{prompt}{}", shown(&view.input)), width);
    let before = wrap_ansi(&format!("{prompt}{}", shown(&view.input[..view.cursor])), width);
    let mut cursor_row = before.len() - 1;
    let mut cursor_col = visible_width(&before[cursor_row]);
    if cursor_col >= width {
        cursor_row += 1;
        cursor_col = 0;
    }
    let mut rows = rows;
    while rows.len() <= cursor_row {
        rows.push(String::new());
    }
    let skip = rows.len().saturating_sub(MAX_INPUT_ROWS);
    (rows.split_off(skip), cursor_row.saturating_sub(skip), cursor_col)
}

/// The rows left for the transcript once the status bar, the scroll note and the input line are
/// placed.
/// @param view - the view
/// @param width - the screen width
/// @param height - the screen height
fn transcript_height(view: &View, width: usize, height: usize) -> usize {
    let footer = input_rows(view, width).0.len() + usize::from(view.status.is_some()) + usize::from(view.scroll > 0);
    height.saturating_sub(footer).max(1)
}

/// The transcript rows that fit in `room` rows, ending `scroll` rows above the newest. Only the
/// lines needed are wrapped, newest first.
/// @param view - the view
/// @param width - the screen width
/// @param room - the rows available
fn visible_rows(view: &View, width: usize, room: usize) -> Vec<String> {
    let needed = room + view.scroll;
    let mut rows: Vec<String> = Vec::new();
    for entry in view.lines.iter().rev() {
        let mut wrapped = entry.rows(width);
        wrapped.reverse();
        rows.extend(wrapped);
        if rows.len() >= needed {
            break;
        }
    }
    let end = view.scroll.min(rows.len());
    let start = (view.scroll + room).min(rows.len());
    let mut shown: Vec<String> = rows[end..start].to_vec();
    shown.reverse();
    shown
}

/// Draws a whole frame: the transcript at the top, then the scroll note, the status bar and the
/// input line at the bottom, with the cursor placed in the input line. The frame is wrapped in
/// synchronized output so a terminal that supports it shows no flicker.
/// @param view - the view
/// @param width - the screen width
/// @param height - the screen height
pub fn render_frame(view: &View, width: usize, height: usize) -> String {
    let (input, cursor_row, cursor_col) = input_rows(view, width);
    let room = transcript_height(view, width, height);
    let mut rows = visible_rows(view, width, room);
    // Short transcripts start at the top, the way a fresh screen fills.
    rows.resize(room, String::new());
    if view.scroll > 0 {
        rows.push(super::console::paint("2", &format!("↓ {} more rows below · PageDown, the mouse wheel or Ctrl+End to return", view.scroll)));
    }
    if let Some(status) = &view.status {
        let line = status(width).into_iter().next().unwrap_or_default();
        rows.push(wrap_ansi(&line, width).swap_remove(0));
    }
    let input_top = rows.len();
    rows.extend(input);
    rows.truncate(height);
    let mut out = String::from("\x1b[?2026h\x1b[?25l\x1b[H");
    // Each row is cleared before it is written, not after. A row as wide as the screen leaves the
    // cursor on the last column with a wrap pending, and an erase to the end of the line from
    // there erases that last character (task-2590: "haiku" showed as "aiku" after a split).
    for (index, row) in rows.iter().enumerate() {
        out.push_str("\x1b[0m\x1b[2K");
        out.push_str(row);
        out.push_str("\x1b[0m");
        if index + 1 < rows.len() {
            out.push_str("\r\n");
        }
    }
    out.push_str("\x1b[J");
    out.push_str(&format!("\x1b[{};{}H\x1b[?25h\x1b[?2026l", (input_top + cursor_row + 1).min(height), cursor_col + 1));
    out
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
            skip_escape(&mut chars, &mut String::new());
            continue;
        }
        width += char_width(c);
    }
    width
}

/// Copies one escape sequence, after its ESC, from `chars` to `out`: a CSI sequence up to its
/// final byte, an OSC sequence up to BEL or ESC backslash, or one more character.
/// @param chars - the characters after the ESC
/// @param out - where the sequence's characters go
fn skip_escape(chars: &mut std::iter::Peekable<std::str::Chars>, out: &mut String) {
    match chars.next() {
        Some('[') => {
            out.push('[');
            for c in chars.by_ref() {
                out.push(c);
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
        }
        Some(']') => {
            out.push(']');
            while let Some(c) = chars.next() {
                out.push(c);
                if c == '\x07' || (c == '\x1b' && chars.peek() == Some(&'\\')) {
                    if c == '\x1b' {
                        out.push(chars.next().unwrap_or('\\'));
                    }
                    break;
                }
            }
        }
        Some(c) => out.push(c),
        None => {}
    }
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
            skip_escape(&mut chars, &mut sequence);
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

    /// Removes color codes and the frame's control codes, keeping the row text.
    fn plain(frame: &str) -> Vec<String> {
        let body = frame.split("\x1b[H").nth(1).unwrap().split("\x1b[J").next().unwrap();
        body.split("\r\n").map(|row| regex::Regex::new("\x1b\\[[0-9;?]*[A-Za-z]").unwrap().replace_all(row, "").to_string()).collect()
    }

    fn view_with(lines: usize) -> View {
        let mut view = View::default();
        for n in 1..=lines {
            view.push(&format!("line {n}"), 40);
        }
        view.status = Some(Arc::new(|_| vec!["status".to_string()]));
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
        let rows = plain(&render_frame(&view, 40, 6));
        assert_eq!(rows, vec!["line 1", "line 2", "", "", "status", "❯ hello"]);
    }

    #[test]
    fn the_newest_lines_show_and_scrolling_up_keeps_its_place() {
        let mut view = view_with(20);
        assert_eq!(plain(&render_frame(&view, 40, 6))[..4], ["line 17", "line 18", "line 19", "line 20"]);
        view.apply_key(Key::PageUp, 40, 6);
        let rows = plain(&render_frame(&view, 40, 6));
        assert_eq!(rows[..3], ["line 15", "line 16", "line 17"]);
        assert!(rows[3].starts_with("↓ 3 more rows below"), "{rows:?}");
        view.push("line 21", 40);
        assert_eq!(plain(&render_frame(&view, 40, 6))[..3], ["line 15", "line 16", "line 17"], "new output does not move a scrolled view");
        view.apply_key(Key::Bottom, 40, 6);
        assert_eq!(plain(&render_frame(&view, 40, 6))[..4], ["line 18", "line 19", "line 20", "line 21"]);
        view.apply_key(Key::Top, 40, 6);
        assert_eq!(plain(&render_frame(&view, 40, 6))[0], "line 1");
    }

    #[test]
    fn the_input_line_edits_and_sends_on_enter() {
        let mut view = view_with(0);
        for key in [Key::Text("helo".into()), Key::Left, Key::Text("l".into()), Key::End, Key::Text("!".into())] {
            assert_eq!(view.apply_key(key, 40, 6), KeyOutcome::None);
        }
        assert_eq!(view.apply_key(Key::Enter, 40, 6), KeyOutcome::Line("hello!".into()));
        assert!(view.input.is_empty());
        assert_eq!(view.apply_key(Key::CtrlD, 40, 6), KeyOutcome::Eof);
        view.apply_key(Key::Paste("a\nb".into()), 40, 6);
        assert_eq!(plain(&render_frame(&view, 40, 6)).last().unwrap(), "❯ a↵b");
    }

    #[test]
    fn a_full_width_row_keeps_its_last_character() {
        // task-2590: each row ended with an erase to the end of the line. After a row as wide as
        // the screen the cursor waits on the last column, so that erase removed the last character.
        let mut view = view_with(0);
        view.push(&"x".repeat(40), 40);
        let frame = render_frame(&view, 40, 6);
        assert!(!frame.contains("\x1b[K"), "no row is erased after it is written: {frame:?}");
        assert!(frame.contains(&format!("\x1b[0m\x1b[2K{}\x1b[0m", "x".repeat(40))), "{frame:?}");
    }

    #[test]
    fn a_laid_out_block_and_the_status_bar_follow_the_screen_width() {
        // task-2590: a new split pane made the screen narrower, and the banner drawn for the old
        // width wrapped into a jumble. A laid out block is drawn again for the new width.
        let mut view = view_with(0);
        view.lines.push_back(Entry::Laid(Arc::new(|width: usize| vec![format!("╭{}╮", "─".repeat(width - 2))])));
        view.status = Some(Arc::new(|width: usize| vec![format!("bar {width}{}", "!".repeat(100))]));
        let wide = plain(&render_frame(&view, 60, 6));
        assert_eq!(wide[0].chars().count(), 60);
        let narrow = plain(&render_frame(&view, 30, 6));
        assert_eq!(narrow[0], format!("╭{}╮", "─".repeat(28)));
        assert_eq!(narrow[1], "", "the block takes one row at the new width");
        assert!(narrow[4].starts_with("bar 30") && narrow[4].chars().count() == 30, "{narrow:?}");
    }

    #[test]
    fn the_last_turn_summary_is_kept_for_the_main_screen() {
        let mut view = view_with(3);
        view.epilogue = Some(view.dropped + view.lines.len());
        view.push("", 40);
        view.push("✔ Done · 1 step", 40);
        view.push("  resume with umbra --resume 1", 40);
        view.epilogue_end = Some(view.dropped + view.lines.len());
        view.push("  Type a follow up task, or press Enter to quit", 40);
        assert_eq!(view.epilogue_lines(), vec!["✔ Done · 1 step", "  resume with umbra --resume 1"]);
    }
}
