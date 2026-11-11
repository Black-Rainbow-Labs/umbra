//! The fullscreen view's state and what keys and the mouse do to it: the input line with its
//! history, the command popup, keyboard focus, the overlays, mouse selection and copy, and the
//! panels that open above the status bar.

use std::time::{Duration, Instant};

use super::block::{Block, LayoutCtx, Target};
use super::frame::FrameInfo;
use super::overlay::{Effect, Outcome, Overlay};
use super::selection::{Pos, Selection};
use super::transcript::Transcript;
use super::{actions, live};
use crate::utils::keys::{Key, Mouse, MouseKind};
use crate::utils::status_bar::StatusInfo;

/// Rows the mouse wheel scrolls.
const WHEEL_ROWS: usize = 3;

/// Two clicks closer together than this are a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(450);

/// The most earlier prompts Up and Down go through.
const HISTORY: usize = 200;

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

/// The mouse between a press and its release.
#[derive(Debug, Default)]
struct MouseState {
    /// Where the button went down, as (column, row).
    press: Option<(usize, usize)>,
    dragged: bool,
    /// The last click: when, where, and how many clicks in a row.
    last_click: Option<(Instant, usize, usize, u8)>,
}

/// What the fullscreen view shows.
#[derive(Default)]
pub struct View {
    pub transcript: Transcript,
    /// Rows of the transcript below the bottom of the view. 0 follows new output.
    pub scroll: usize,
    /// The status bar's numbers. The frame adds the live parts: the agent time, the spinner,
    /// the MCP error count and the usage.
    pub status: Option<StatusInfo>,
    /// The prompt on the input line, with color codes. Empty while the agent works.
    pub prompt: String,
    /// What the person is typing.
    pub input: Vec<char>,
    /// The cursor's position in `input`.
    pub cursor: usize,
    /// The size the last frame was drawn at.
    pub size: (usize, usize),
    pub overlay: Option<Box<dyn Overlay>>,
    /// The element with keyboard focus, or None when the input line has it.
    pub focus: Option<Target>,
    pub selection: Option<Selection>,
    /// The MCP error list above the status bar is open.
    pub mcp_errors_open: bool,
    /// The weekly usage details above the status bar are open.
    pub usage_open: bool,
    /// The commands waiting for approval, shown in full above the input.
    pub pending: Vec<String>,
    /// A short note in the status bar, such as `copied 12 characters`, and when it was set.
    pub note: Option<(String, Instant)>,
    /// The command popup's selected row.
    pub popup_index: usize,
    /// The input text the popup was closed for with Esc; it opens again once the text changes.
    pub popup_closed_for: Option<String>,
    /// Earlier prompts, oldest first.
    pub history: Vec<String>,
    /// The position in `history` Up and Down are at, and the line being typed before that.
    pub history_pos: Option<(usize, Vec<char>)>,
    pub color: bool,
    /// The newer version the update prompt was opened for, so it opens once per session.
    pub update_offered: Option<String>,
    /// Where things were in the last frame, for mouse clicks and focus.
    pub frame: FrameInfo,
    mouse: MouseState,
}

impl View {
    /// A view with color on or off.
    /// @param color - true to draw ANSI colors
    pub fn new(color: bool) -> Self {
        View { color, ..Default::default() }
    }

    /// The frame context for layout at this moment.
    pub fn ctx(&self) -> LayoutCtx {
        let now = Instant::now();
        LayoutCtx { color: self.color, now, spinner: live::spinner_at(now) }
    }

    /// Adds a block, keeping the view on the same rows while it is scrolled up.
    /// @param block - the block
    pub fn push_block(&mut self, block: Block) -> u64 {
        let width = self.size.0.max(10);
        let id = self.transcript.push(block);
        if self.scroll > 0 {
            let ctx = self.ctx();
            let index = self.transcript.len() - 1;
            self.scroll += self.transcript.height_at(index, width, &ctx);
        }
        let ctx = self.ctx();
        self.transcript.enforce_cap(width, &ctx);
        id
    }

    /// Applies one key or mouse report, and returns what to send and the work to do once the lock
    /// is released.
    /// @param key - the key
    pub fn apply_key(&mut self, key: Key) -> (KeyOutcome, Vec<Effect>) {
        if let Key::Mouse(mouse) = key {
            return (KeyOutcome::None, self.apply_mouse(mouse));
        }
        if !matches!(key, Key::WheelUp | Key::WheelDown) {
            self.selection = None;
        }
        if self.overlay.as_ref().is_some_and(|o| o.passes_typing()) && matches!(key, Key::Text(_) | Key::Paste(_) | Key::Backspace) {
            self.overlay = None;
        }
        if self.overlay.is_some() {
            return self.overlay_key(&key);
        }
        if self.focus.is_some() {
            if let Some(result) = self.focus_key(&key) {
                return result;
            }
        }
        if let Some(result) = self.popup_key(&key) {
            return result;
        }
        self.input_key(key)
    }

    /// Sends a key to the open overlay and acts on what it asks for.
    /// @param key - the key
    fn overlay_key(&mut self, key: &Key) -> (KeyOutcome, Vec<Effect>) {
        let outcome = match (key, self.overlay.as_mut()) {
            (Key::WheelUp, _) | (Key::WheelDown, _) => Outcome::Keep,
            (_, Some(overlay)) => overlay.key(key),
            (_, None) => Outcome::Keep,
        };
        self.apply_outcome(outcome)
    }

    /// Acts on what an overlay asked for.
    /// @param outcome - the overlay's answer
    pub fn apply_outcome(&mut self, outcome: Outcome) -> (KeyOutcome, Vec<Effect>) {
        match outcome {
            Outcome::Keep => (KeyOutcome::None, Vec::new()),
            Outcome::Close => {
                self.overlay = None;
                (KeyOutcome::None, Vec::new())
            }
            Outcome::Effects(effects) => (KeyOutcome::None, effects),
            Outcome::CloseWith(effects) => {
                self.overlay = None;
                (KeyOutcome::None, effects)
            }
            Outcome::Line(line) => {
                self.overlay = None;
                (KeyOutcome::Line(line), Vec::new())
            }
        }
    }

    /// Keys while an element has focus: Tab and Shift+Tab move, Enter and Space activate, Esc
    /// gives focus back to the input. Typing a character gives focus back and types it. Returns
    /// None for keys that go on to the input line.
    /// @param key - the key
    fn focus_key(&mut self, key: &Key) -> Option<(KeyOutcome, Vec<Effect>)> {
        match key {
            Key::Tab => self.move_focus(true),
            Key::BackTab => self.move_focus(false),
            Key::Enter | Key::Text(_) if matches!(key, Key::Enter) || matches!(key, Key::Text(t) if t == " ") => {
                let target = self.focus?;
                return Some((KeyOutcome::None, self.activate(target)));
            }
            Key::Escape => self.focus = None,
            Key::Up | Key::Down | Key::PageUp | Key::PageDown | Key::WheelUp | Key::WheelDown | Key::Top | Key::Bottom => return None,
            _ => {
                self.focus = None;
                return None;
            }
        }
        Some((KeyOutcome::None, Vec::new()))
    }

    /// Moves the focus to the next element, newest first, or back the other way. Past the last
    /// element the input line has focus again.
    /// @param forward - true for Tab, false for Shift+Tab
    pub fn move_focus(&mut self, forward: bool) {
        let targets = super::frame::focus_targets(self);
        if targets.is_empty() {
            self.focus = None;
            return;
        }
        let position = self.focus.and_then(|f| targets.iter().position(|t| *t == f));
        self.focus = match (position, forward) {
            (None, true) => targets.first().copied(),
            (None, false) => targets.last().copied(),
            (Some(i), true) => targets.get(i + 1).copied(),
            (Some(0), false) => None,
            (Some(i), false) => targets.get(i - 1).copied(),
        };
        if let Some(target) = self.focus {
            super::frame::scroll_to(self, target);
        }
    }

    /// Does what an element does when clicked or activated with Enter or Space.
    /// @param target - the element
    pub fn activate(&mut self, target: Target) -> Vec<Effect> {
        match target {
            Target::Group(id) => {
                let before = self.target_place(target);
                if let Some(group) = self.transcript.group_mut(id) {
                    group.expanded = !group.expanded;
                }
                self.keep_place(target, before);
            }
            Target::Run(id, index) => {
                let before = self.target_place(target);
                if let Some(run) = self.transcript.group_mut(id).and_then(|g| g.runs.get_mut(index)) {
                    run.expanded = !run.expanded;
                }
                self.keep_place(target, before);
            }
            Target::McpErrors => self.mcp_errors_open = !self.mcp_errors_open,
            Target::Usage => self.usage_open = !self.usage_open,
            Target::HeaderModel => self.overlay = Some(super::overlay::pickers::model_picker()),
            Target::HeaderMcp => self.overlay = Some(Box::new(super::overlay::mcp_panel::McpPanel::new())),
            Target::Popup(index) => {
                if let Some(entry) = self.popup_matches().get(index) {
                    let name = entry.name.clone();
                    self.complete_to(&name);
                }
            }
            Target::Item(item) => {
                let outcome = match self.overlay.as_mut() {
                    Some(overlay) => overlay.click(item),
                    None => Outcome::Keep,
                };
                return self.apply_outcome(outcome).1;
            }
        }
        Vec::new()
    }

    /// Where a target's row is: the rows from it to the bottom of the transcript, counting its own
    /// row, and the transcript's total rows. None when no row has the target.
    /// @param target - the element
    fn target_place(&mut self, target: Target) -> Option<(usize, usize)> {
        let width = self.size.0.max(10);
        let ctx = self.ctx();
        let mut position = None;
        let mut total = 0;
        for index in 0..self.transcript.len() {
            let rows = self.transcript.rows_at(index, width, &ctx);
            if position.is_none() {
                position = rows.iter().position(|r| r.target == Some(target)).map(|at| total + at);
            }
            total += rows.len();
        }
        position.map(|p| (total - p, total))
    }

    /// After an expand or a collapse, scrolls so the toggled header stays on the screen row it was
    /// on. A collapse while scrolled up takes the removed rows off the scroll, so the view never
    /// points past the top of the shorter transcript and the screen is never left empty
    /// (task-2625). An expand at the bottom keeps following the bottom unless the header would
    /// leave the top of the screen, and then the header becomes the top row.
    /// @param target - the toggled element
    /// @param before - its place before the toggle
    fn keep_place(&mut self, target: Target, before: Option<(usize, usize)>) {
        let (Some((_, total_before)), Some((offset_after, total_after))) = (before, self.target_place(target)) else { return };
        let room = self.frame.room.max(1);
        if self.scroll > 0 || total_after < total_before {
            self.scroll = (self.scroll + total_after).saturating_sub(total_before);
        } else if offset_after > room {
            self.scroll = offset_after - room;
        }
        self.scroll = self.scroll.min(total_after.saturating_sub(room));
    }

    /// Scrolls so a target's row is on screen.
    /// @param target - the element
    #[cfg(test)]
    pub fn scroll_to_target(&mut self, target: Target) {
        super::frame::scroll_to(self, target);
    }

    /// The commands the popup shows for the input, when it is open: the input starts with `/`,
    /// has no space yet, and was not closed with Esc.
    pub fn popup_matches(&self) -> Vec<live::CommandEntry> {
        let text: String = self.input.iter().collect();
        if !text.starts_with('/') || text.contains(char::is_whitespace) || self.popup_closed_for.as_deref() == Some(text.as_str()) || !self.prompt_takes_commands() {
            return Vec::new();
        }
        crate::services::command_service::complete(&text[1..], &live::commands())
    }

    /// True when the input line takes slash commands: always, except at the confirm prompt and the
    /// human mode prompt, whose own short commands start with `/` too.
    fn prompt_takes_commands(&self) -> bool {
        let prompt = crate::utils::ansi::plain(&self.prompt);
        !(prompt.starts_with("Run?") || prompt.starts_with("command ("))
    }

    /// Keys while the command popup is open: Up and Down move, Tab completes, Enter completes or
    /// runs an exact match, Esc closes the popup and keeps the text. Returns None for other keys.
    /// @param key - the key
    fn popup_key(&mut self, key: &Key) -> Option<(KeyOutcome, Vec<Effect>)> {
        let matches = self.popup_matches();
        if matches.is_empty() {
            self.popup_index = 0;
            return None;
        }
        self.popup_index = self.popup_index.min(matches.len() - 1);
        let typed: String = self.input.iter().skip(1).collect();
        match key {
            Key::Up => self.popup_index = self.popup_index.saturating_sub(1),
            Key::Down => self.popup_index = (self.popup_index + 1).min(matches.len() - 1),
            Key::Escape => self.popup_closed_for = Some(self.input.iter().collect()),
            Key::Tab => self.complete_to(&matches[self.popup_index].name),
            Key::Enter if matches.iter().any(|m| m.name == typed) => return None,
            Key::Enter => self.complete_to(&matches[self.popup_index].name),
            _ => return None,
        }
        Some((KeyOutcome::None, Vec::new()))
    }

    /// Replaces the input with `/name ` and puts the cursor at the end.
    /// @param name - the command name
    fn complete_to(&mut self, name: &str) {
        self.input = format!("/{name} ").chars().collect();
        self.cursor = self.input.len();
        self.popup_index = 0;
    }

    /// Keys on the input line: editing, history, scrolling, Tab into focus, and Enter, which runs
    /// a command the view handles itself or sends the line.
    /// @param key - the key
    fn input_key(&mut self, key: Key) -> (KeyOutcome, Vec<Effect>) {
        let page = self.frame.room.saturating_sub(1).max(1);
        match key {
            Key::Text(text) | Key::Paste(text) => self.insert(&text),
            Key::Enter => return self.enter(),
            Key::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.input.remove(self.cursor);
            }
            Key::Delete | Key::CtrlD if self.cursor < self.input.len() => {
                self.input.remove(self.cursor);
            }
            Key::CtrlD if self.input.is_empty() => return (KeyOutcome::Eof, Vec::new()),
            Key::CtrlU => {
                self.input.clear();
                self.cursor = 0;
            }
            Key::CtrlK => self.input.truncate(self.cursor),
            Key::CtrlW => self.delete_word(),
            Key::Left => self.cursor = self.cursor.saturating_sub(1),
            Key::Right => self.cursor = (self.cursor + 1).min(self.input.len()),
            Key::Home | Key::CtrlA => self.cursor = 0,
            Key::End | Key::CtrlE => self.cursor = self.input.len(),
            Key::WordLeft => self.cursor = word_start(&self.input, self.cursor),
            Key::WordRight => self.cursor = word_end(&self.input, self.cursor),
            Key::Tab => self.move_focus(true),
            Key::BackTab => self.move_focus(false),
            Key::Up if self.scroll == 0 && self.input_row_of_cursor() == 0 => self.history_step(true),
            Key::Down if self.scroll == 0 && self.history_pos.is_some() => self.history_step(false),
            Key::Up => self.scroll_by(1),
            Key::Down => self.scroll = self.scroll.saturating_sub(1),
            Key::PageUp => self.scroll_by(page),
            Key::PageDown => self.scroll = self.scroll.saturating_sub(page),
            Key::WheelUp => self.scroll_by(WHEEL_ROWS),
            Key::WheelDown => self.scroll = self.scroll.saturating_sub(WHEEL_ROWS),
            Key::Top => self.scroll_by(usize::MAX / 2),
            Key::Bottom | Key::Escape => {
                self.scroll = 0;
                self.mcp_errors_open = false;
                self.usage_open = false;
            }
            _ => {}
        }
        (KeyOutcome::None, Vec::new())
    }

    /// Enter on the input line: a command the view handles itself opens what it opens; any other
    /// line is sent, and kept in the history.
    fn enter(&mut self) -> (KeyOutcome, Vec<Effect>) {
        let line: String = self.input.drain(..).collect();
        self.cursor = 0;
        self.history_pos = None;
        self.popup_closed_for = None;
        if !line.trim().is_empty() && self.history.last() != Some(&line) {
            self.history.push(line.clone());
            if self.history.len() > HISTORY {
                self.history.remove(0);
            }
        }
        if self.prompt_takes_commands() {
            if let Some(handled) = actions::view_command(self, &line) {
                return handled;
            }
        }
        (KeyOutcome::Line(line), Vec::new())
    }

    /// Goes one step through the earlier prompts.
    /// @param back - true for an older prompt
    fn history_step(&mut self, back: bool) {
        if self.history.is_empty() {
            return;
        }
        let (position, draft) = match self.history_pos.take() {
            Some((p, d)) => (Some(p), d),
            None => (None, self.input.clone()),
        };
        let next = match (position, back) {
            (None, true) => Some(self.history.len() - 1),
            (None, false) => None,
            (Some(p), true) => Some(p.saturating_sub(1)),
            (Some(p), false) if p + 1 < self.history.len() => Some(p + 1),
            (Some(_), false) => None,
        };
        match next {
            Some(p) => {
                self.input = self.history[p].chars().collect();
                self.history_pos = Some((p, draft));
            }
            None => self.input = draft,
        }
        self.cursor = self.input.len();
    }

    /// The row of the input the cursor is on, counted from the input's first row.
    fn input_row_of_cursor(&self) -> usize {
        super::frame::input_rows(self, self.size.0.max(10)).1
    }

    /// Deletes the word before the cursor.
    fn delete_word(&mut self) {
        let start = word_start(&self.input, self.cursor);
        self.input.drain(start..self.cursor);
        self.cursor = start;
    }

    /// Inserts text at the cursor.
    /// @param text - the text
    pub fn insert(&mut self, text: &str) {
        for c in text.chars() {
            self.input.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    /// Scrolls up, no further than the top of the transcript.
    /// @param rows - rows to scroll
    pub fn scroll_by(&mut self, rows: usize) {
        let width = self.size.0.max(10);
        let ctx = self.ctx();
        let total = self.transcript.total_rows(width, &ctx);
        let room = self.frame.room.saturating_sub(usize::from(self.scroll == 0)).max(1);
        self.scroll = self.scroll.saturating_add(rows).min(total.saturating_sub(room));
    }

    /// Handles a mouse report: the wheel scrolls, a press and release on one cell is a click, a
    /// press, drag and release selects text and copies it.
    /// @param mouse - the report
    fn apply_mouse(&mut self, mouse: Mouse) -> Vec<Effect> {
        match mouse.kind {
            MouseKind::Press if mouse.button == 0 => {
                self.mouse.press = Some((mouse.col, mouse.row));
                self.mouse.dragged = false;
                Vec::new()
            }
            MouseKind::Drag if self.mouse.press.is_some() => {
                self.drag_to(mouse.col, mouse.row);
                Vec::new()
            }
            MouseKind::Release if self.mouse.press.is_some() => {
                let dragged = self.mouse.dragged;
                self.mouse.press = None;
                if dragged { self.finish_selection() } else { self.click(mouse.col, mouse.row) }
            }
            _ => Vec::new(),
        }
    }

    /// Extends the selection to a cell, starting it at the press when this is the first drag.
    /// Dragging onto the first or last transcript row scrolls.
    /// @param col - the column
    /// @param row - the screen row
    fn drag_to(&mut self, col: usize, row: usize) {
        let (press_col, press_row) = self.mouse.press.expect("a button is down");
        if !self.mouse.dragged {
            let Some(anchor) = self.frame.position(press_col, press_row) else { return };
            self.selection = Some(Selection { anchor, head: anchor });
            self.mouse.dragged = true;
        }
        if row == 0 {
            self.scroll_by(1);
        } else if row + 1 >= self.frame.room && self.scroll > 0 {
            self.scroll -= 1;
        }
        let clamped = row.min(self.frame.room.saturating_sub(1));
        if let (Some(head), Some(selection)) = (self.frame.position(col, clamped), self.selection.as_mut()) {
            selection.head = head;
        }
    }

    /// Copies the selected text when the button is released after a drag.
    fn finish_selection(&mut self) -> Vec<Effect> {
        let Some(selection) = self.selection else { return Vec::new() };
        let ctx = self.ctx();
        let text = super::selection::selected_text(&mut self.transcript, &selection, self.size.0.max(10), &ctx);
        if text.is_empty() {
            return Vec::new();
        }
        self.copy(text)
    }

    /// Copies text to the clipboard and shows a note saying so.
    /// @param text - the text
    fn copy(&mut self, text: String) -> Vec<Effect> {
        let count = text.chars().count();
        let note = match crate::utils::clipboard::copy(&text) {
            Ok(()) => format!("copied {count} {}", if count == 1 { "character" } else { "characters" }),
            Err(_) => format!("copy failed; hold {} to select with the terminal", crate::utils::clipboard::terminal_modifier()),
        };
        self.note = Some((note, Instant::now()));
        Vec::new()
    }

    /// A click with no drag: a double click selects a word, a triple click a line, and a single
    /// click does what the element under it does. A click outside an open overlay closes it.
    /// @param col - the column
    /// @param row - the screen row
    fn click(&mut self, col: usize, row: usize) -> Vec<Effect> {
        let now = Instant::now();
        let count = match self.mouse.last_click {
            Some((at, c, r, n)) if now.duration_since(at) < DOUBLE_CLICK && c == col && r == row => n % 3 + 1,
            _ => 1,
        };
        self.mouse.last_click = Some((now, col, row, count));
        if count > 1 {
            if let Some(pos) = self.frame.position(col, row) {
                return self.select_around(pos, count == 3);
            }
        }
        self.selection = None;
        match self.frame.hit(col, row) {
            Some(target) => {
                self.focus = None;
                self.activate(target)
            }
            None => {
                if self.overlay.as_ref().is_some_and(|o| o.placement() == super::overlay::Placement::AboveStatus) && !self.frame.in_overlay(row) {
                    self.overlay = None;
                }
                Vec::new()
            }
        }
    }

    /// Selects the word or the row around a position and copies it.
    /// @param pos - where the double or triple click was
    /// @param line - true to select the whole row
    fn select_around(&mut self, pos: Pos, line: bool) -> Vec<Effect> {
        let width = self.size.0.max(10);
        let ctx = self.ctx();
        let Some(selection) = super::selection::around(&mut self.transcript, pos, line, width, &ctx) else { return Vec::new() };
        self.selection = Some(selection);
        self.finish_selection()
    }

    /// Opens the update prompt when a check found a newer release, the agent is idle, nothing else
    /// is open, and the person is not typing. It opens once per session.
    pub fn offer_update(&mut self) {
        let Some(version) = crate::services::update_service::available() else { return };
        let idle = !crate::services::work_clock::is_running() && !self.prompt.is_empty() && self.prompt_takes_commands();
        if !idle || self.overlay.is_some() || !self.input.is_empty() || self.update_offered.as_deref() == Some(version.as_str()) {
            return;
        }
        self.update_offered = Some(version.clone());
        self.overlay = Some(Box::new(super::overlay::update_prompt::UpdatePrompt::new(&version)));
    }

    /// True when anything on screen changes with time, so the ticker keeps drawing.
    /// @param now - the time
    pub fn animating(&self, now: Instant) -> bool {
        self.transcript.animating() || self.overlay.as_ref().is_some_and(|o| o.animating()) || self.note.as_ref().is_some_and(|(_, at)| now.duration_since(*at) < Duration::from_secs(3)) || super::header::animating(now) || crate::services::work_clock::is_running()
    }
}

/// The start of the word before a position.
/// @param text - the input
/// @param at - the position
fn word_start(text: &[char], at: usize) -> usize {
    let mut i = at.min(text.len());
    while i > 0 && text[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !text[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

/// The end of the word after a position.
/// @param text - the input
/// @param at - the position
fn word_end(text: &[char], at: usize) -> usize {
    let mut i = at.min(text.len());
    while i < text.len() && text[i].is_whitespace() {
        i += 1;
    }
    while i < text.len() && !text[i].is_whitespace() {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_found_for_alt_arrows_and_ctrl_w() {
        let text: Vec<char> = "fix the  test".chars().collect();
        assert_eq!(word_start(&text, 13), 9);
        assert_eq!(word_start(&text, 9), 4);
        assert_eq!(word_end(&text, 0), 3);
        assert_eq!(word_end(&text, 3), 7);
    }

    #[test]
    fn up_and_down_go_through_earlier_prompts() {
        let mut view = View::new(false);
        view.size = (40, 10);
        view.prompt = "❯ ".into();
        for line in ["first", "second"] {
            view.insert(line);
            assert_eq!(view.apply_key(Key::Enter).0, KeyOutcome::Line(line.into()));
        }
        view.insert("draft");
        view.apply_key(Key::Up);
        assert_eq!(view.input.iter().collect::<String>(), "second");
        view.apply_key(Key::Up);
        assert_eq!(view.input.iter().collect::<String>(), "first");
        view.apply_key(Key::Down);
        view.apply_key(Key::Down);
        assert_eq!(view.input.iter().collect::<String>(), "draft");
    }

    /// A view with a few lines of text, then one finished command whose output is `lines` rows,
    /// then a few more lines, drawn once at 40 by 20.
    fn view_with_long_output(lines: usize) -> (View, u64) {
        use super::super::block::{CommandRun, RunKind, RunState};
        let mut view = View::new(false);
        view.size = (40, 20);
        view.prompt = "❯ ".into();
        for i in 0..30 {
            view.push_block(Block::Text(format!("before {i}")));
        }
        let output: String = (0..lines).map(|i| format!("output line {i}\n")).collect();
        let run = CommandRun { kind: RunKind::Shell { command: "cat big".into() }, state: RunState::Done, output, exit_code: Some(0), error: None, duration: Some(Duration::from_secs(1)), expanded: false };
        let (id, _) = view.transcript.add_run(run);
        view.push_block(Block::Text("after".into()));
        super::super::frame::render_frame(&mut view, 40, 20);
        (view, id)
    }

    /// The screen rows of the transcript part of a frame, as plain text.
    fn transcript_rows(view: &mut View) -> Vec<String> {
        let frame = super::super::frame::render_frame(view, 40, 20);
        let rows: Vec<String> = frame.split("\r\n").map(crate::utils::ansi::plain).collect();
        rows[..view.frame.room].to_vec()
    }

    #[test]
    fn collapsing_a_long_output_while_scrolled_into_it_keeps_the_screen_full() {
        let (mut view, id) = view_with_long_output(200);
        view.activate(Target::Group(id));
        view.activate(Target::Run(id, 0));
        super::super::frame::render_frame(&mut view, 40, 20);
        view.scroll_by(150);
        super::super::frame::render_frame(&mut view, 40, 20);
        view.scroll_to_target(Target::Group(id));
        view.activate(Target::Group(id));
        let rows = transcript_rows(&mut view);
        assert!(rows.iter().all(|r| !r.trim().is_empty() || r.is_empty()), "{rows:?}");
        let filled = rows.iter().filter(|r| !r.trim().is_empty()).count();
        assert!(filled >= rows.len() - 1, "only {filled} of {} rows drawn: {rows:?}", rows.len());
    }

    #[test]
    fn a_collapse_keeps_the_header_on_its_row() {
        let (mut view, id) = view_with_long_output(200);
        view.activate(Target::Group(id));
        view.activate(Target::Run(id, 0));
        super::super::frame::render_frame(&mut view, 40, 20);
        view.scroll_by(150);
        view.scroll_to_target(Target::Run(id, 0));
        transcript_rows(&mut view);
        let row_of = |view: &View| view.frame.hits.iter().find(|h| h.target == Target::Run(id, 0)).map(|h| h.row);
        let before = row_of(&view).expect("the run header is on screen");
        view.activate(Target::Run(id, 0));
        transcript_rows(&mut view);
        assert_eq!(row_of(&view), Some(before));
    }

    #[test]
    fn the_status_bar_is_the_last_row_and_the_input_is_above_it() {
        let mut view = View::new(false);
        view.size = (60, 12);
        view.prompt = "❯ ".into();
        view.status = Some(StatusInfo { model: "test-model".into(), ..Default::default() });
        view.push_block(Block::Text("hello".into()));
        view.insert("typed text");
        let frame = super::super::frame::render_frame(&mut view, 60, 12);
        let rows: Vec<String> = frame.split("\r\n").map(crate::utils::ansi::plain).collect();
        assert!(rows[rows.len() - 1].contains("test-model"), "{rows:?}");
        assert!(rows[rows.len() - 2].starts_with("❯ typed text"), "{rows:?}");
    }
}
