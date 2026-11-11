//! Draws a whole frame of the fullscreen view and records where everything went: the transcript
//! at the top, the panels and overlays above the status bar, the status bar, and the input line,
//! with a centered overlay drawn over the transcript. The record (the hit map) is what turns a
//! mouse click into a target and a drag into transcript positions.

use std::time::Duration;

use super::block::{paint, LayoutCtx, Row, Target};
use super::overlay::Placement;
use super::selection::{self, Pos};
use super::view::View;
use super::live;
use crate::services::mcp::ServerState;
use crate::utils::ansi;
use crate::utils::screen::{visible_width, wrap_ansi};
use crate::utils::status_bar::{render_status_parts, StatusPart, UsageView};

/// The most rows the input line may take at the bottom of the screen.
const MAX_INPUT_ROWS: usize = 5;

/// The most rows the command popup shows.
const POPUP_ROWS: usize = 8;

/// The prompt shown on the input line while the agent works and no question is asked.
const WORKING_PROMPT: &str = "› ";

/// A clickable range of a screen row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub row: usize,
    pub from: usize,
    pub to: usize,
    pub target: Target,
}

/// Where things were in the last frame.
#[derive(Debug, Clone, Default)]
pub struct FrameInfo {
    /// The screen rows the transcript took, from the top.
    pub room: usize,
    /// For each transcript row on screen, the block and the row inside it.
    pub lines: Vec<Option<(u64, usize)>>,
    pub hits: Vec<Hit>,
    /// The screen rows the overlay or panel took, as a start and an end after the last.
    pub overlay: (usize, usize),
}

impl FrameInfo {
    /// The transcript position under a cell, or None outside the transcript.
    /// @param col - the column
    /// @param row - the screen row
    pub fn position(&self, col: usize, row: usize) -> Option<Pos> {
        if self.in_overlay(row) {
            return None;
        }
        let (id, inner) = (*self.lines.get(row)?)?;
        Some(Pos { id, row: inner, col })
    }

    /// The target under a cell.
    /// @param col - the column
    /// @param row - the screen row
    pub fn hit(&self, col: usize, row: usize) -> Option<Target> {
        self.hits.iter().rev().find(|h| h.row == row && col >= h.from && col < h.to).map(|h| h.target)
    }

    /// True when a screen row belongs to the open overlay or panel.
    /// @param row - the screen row
    pub fn in_overlay(&self, row: usize) -> bool {
        row >= self.overlay.0 && row < self.overlay.1
    }
}

/// The rows the input line takes: the prompt and the typed text, wrapped, at most
/// `MAX_INPUT_ROWS`. Returns the rows and the cursor's row and column within them.
/// @param view - the view
/// @param width - the screen width
pub fn input_rows(view: &View, width: usize) -> (Vec<String>, usize, usize) {
    let prompt = if view.prompt.is_empty() { paint(&view.ctx(), "2", WORKING_PROMPT) } else { view.prompt.clone() };
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

/// The status bar's numbers with the live parts filled in: the agent time, the spinner, the MCP
/// error count, the usage and the note.
/// @param view - the view
/// @param ctx - the frame
fn live_status(view: &View, ctx: &LayoutCtx) -> Option<crate::utils::status_bar::StatusInfo> {
    let mut info = view.status.clone()?;
    info.work_secs = crate::services::work_clock::elapsed().as_secs();
    info.working = crate::services::work_clock::is_running();
    info.spinner = ctx.spinner;
    info.mcp_errors = live::mcp().map_or(0, |m| m.failures().len());
    info.usage = crate::services::usage_service::current().map(|u| UsageView { percent: u.percent, stale: u.stale });
    info.note = view.note.as_ref().filter(|(_, at)| ctx.now.duration_since(*at) < Duration::from_secs(2)).map(|(text, _)| text.clone());
    Some(info)
}

/// The status bar row and its clickable parts, with the focused part in reverse video.
/// @param view - the view
/// @param width - the screen width
/// @param ctx - the frame
fn status_row(view: &View, width: usize, ctx: &LayoutCtx) -> Option<(String, Vec<(usize, usize, Target)>)> {
    let info = live_status(view, ctx)?;
    let (mut line, parts) = render_status_parts(&info, width, ctx.color);
    let mut targets = Vec::new();
    for (from, to, part) in parts {
        let target = match part {
            StatusPart::McpErrors => Target::McpErrors,
            StatusPart::Usage => Target::Usage,
        };
        if view.focus == Some(target) {
            line = ansi::overlay_style(&line, from, to, "\x1b[7m");
        }
        targets.push((from, to, target));
    }
    Some((line, targets))
}

/// The rows of the command popup.
/// @param view - the view
/// @param width - the screen width
/// @param ctx - the frame
fn popup_rows(view: &View, width: usize, ctx: &LayoutCtx) -> Vec<Row> {
    let matches = view.popup_matches();
    let top = view.popup_index.saturating_sub(POPUP_ROWS - 1);
    matches.iter().enumerate().skip(top).take(POPUP_ROWS).map(|(i, entry)| {
        let pointer = if i == view.popup_index { "›" } else { " " };
        let name = format!(" {pointer} /{:<18}", entry.name);
        let kind = format!(" {:<8} ", entry.kind);
        let room = width.saturating_sub(visible_width(&name) + kind.len() + 1);
        let description = super::block::cut_to(entry.description.lines().next().unwrap_or(""), room);
        let text = format!("{name}{}{}", paint(ctx, "90", &kind), paint(ctx, "90", &description));
        let text = if i == view.popup_index { paint(ctx, "1", &ansi::fit(&text, width)) } else { ansi::fit(&text, width) };
        Row { text, target: Some(Target::Popup(i)), ..Default::default() }
    }).collect()
}

/// The rows of the MCP error list: each failed server with its full error and its log file.
/// @param width - the screen width
/// @param ctx - the frame
fn error_rows(width: usize, ctx: &LayoutCtx) -> Vec<Row> {
    let Some(manager) = live::mcp() else { return Vec::new() };
    let mut rows = vec![Row::text(ansi::fit(&format!("{}{}", paint(ctx, "1;31", " MCP errors"), paint(ctx, "90", "   Enter, a click or Esc closes · /mcp restarts")), width))];
    for (name, state) in manager.snapshot() {
        if let ServerState::Failed { error, log_path, .. } = state {
            rows.push(Row { text: paint(ctx, "31", &format!(" ▲ {name}")), target: Some(Target::McpErrors), ..Default::default() });
            for (line, _) in super::block::word_wrap(&super::markdown::sanitize(&error), width.saturating_sub(5)) {
                rows.push(Row { text: format!("     {line}"), target: Some(Target::McpErrors), ..Default::default() });
            }
            rows.push(Row { text: paint(ctx, "90", &format!("     log {}", log_path.display())), target: Some(Target::McpErrors), ..Default::default() });
        }
    }
    rows
}

/// The rows of the weekly usage details.
/// @param width - the screen width
/// @param ctx - the frame
fn usage_rows(width: usize, ctx: &LayoutCtx) -> Vec<Row> {
    let Some(usage) = crate::services::usage_service::current() else { return Vec::new() };
    vec![Row { text: ansi::fit(&paint(ctx, "90", &format!(" {}", crate::services::usage_service::describe(&usage))), width), target: Some(Target::Usage), ..Default::default() }]
}

/// The rows drawn above the status bar: the commands waiting for approval, then the open overlay,
/// the command popup, the MCP errors or the usage details, at most half the screen.
/// @param view - the view
/// @param width - the screen width
/// @param height - the screen height
/// @param ctx - the frame
fn panel_rows(view: &mut View, width: usize, height: usize, ctx: &LayoutCtx) -> Vec<Row> {
    let limit = (height / 2).max(3);
    let mut rows: Vec<Row> = view.pending.iter().flat_map(|line| wrap_ansi(&paint(ctx, "35", line), width)).map(Row::text).collect();
    let above = view.overlay.as_ref().is_some_and(|o| o.placement() == Placement::AboveStatus);
    if above {
        let room = limit.saturating_sub(rows.len()).max(2);
        if let Some(overlay) = view.overlay.as_mut() {
            rows.extend(overlay.rows(width, room, ctx));
        }
    } else if view.overlay.is_none() && !view.popup_matches().is_empty() {
        rows.extend(popup_rows(view, width, ctx));
    } else if view.mcp_errors_open {
        rows.extend(error_rows(width, ctx));
    } else if view.usage_open {
        rows.extend(usage_rows(width, ctx));
    }
    let skip = rows.len().saturating_sub(limit);
    rows.split_off(skip)
}

/// The transcript rows that fit in `room` rows, ending `scroll` rows above the newest, with the
/// block and row each one came from.
/// @param view - the view
/// @param width - the screen width
/// @param room - the rows available
/// @param ctx - the frame
fn visible_rows(view: &mut View, width: usize, room: usize, ctx: &LayoutCtx) -> Vec<(Row, Option<(u64, usize)>)> {
    let needed = room + view.scroll;
    let mut rows: Vec<(Row, Option<(u64, usize)>)> = Vec::new();
    for index in (0..view.transcript.len()).rev() {
        let id = view.transcript.entries().nth(index).map(|e| e.id).unwrap_or(0);
        let laid = view.transcript.rows_at(index, width, ctx);
        for (number, row) in laid.into_iter().enumerate().rev() {
            rows.push((row, Some((id, number))));
        }
        if rows.len() >= needed {
            break;
        }
    }
    let end = view.scroll.min(rows.len());
    let start = (view.scroll + room).min(rows.len());
    let mut shown: Vec<(Row, Option<(u64, usize)>)> = rows.drain(end..start).collect();
    shown.reverse();
    shown
}

/// A transcript row as drawn: the selection in reverse video and the focus marker.
/// @param view - the view
/// @param row - the laid out row
/// @param origin - the block and row it came from
/// @param width - the screen width
fn decorate(view: &View, row: &Row, origin: Option<(u64, usize)>, width: usize) -> String {
    let mut text = row.text.clone();
    if row.target.is_some() && row.target == view.focus {
        text = match row.focus_col {
            Some(col) => ansi::overlay_style(&ansi::replace_column(&text, col, "›"), col, visible_width(&text), "\x1b[1m"),
            None => format!("\x1b[1m{text}"),
        };
    }
    if let (Some(selection), Some((id, number))) = (&view.selection, origin) {
        if let Some(columns) = selection::columns_on(&view.transcript, selection, id, number, width) {
            text = selection::highlight(&text, (columns.0, columns.1.min(visible_width(&text).max(columns.0 + 1))));
        }
    }
    text
}

/// Draws a centered overlay over the transcript rows and records its click targets.
/// @param view - the view
/// @param rows - the transcript rows as drawn, changed in place
/// @param width - the screen width
/// @param ctx - the frame
/// @param hits - the frame's targets
fn draw_centered(view: &mut View, rows: &mut [String], width: usize, ctx: &LayoutCtx, hits: &mut Vec<Hit>) {
    let Some(overlay) = view.overlay.as_mut().filter(|o| o.placement() == Placement::Centered) else { return };
    let box_width = (width * 8 / 10).max(70).min(width);
    let box_rows = overlay.rows(box_width, rows.len(), ctx);
    let top = rows.len().saturating_sub(box_rows.len()) / 2;
    let left = (width - box_width) / 2;
    for (offset, row) in box_rows.iter().enumerate().take(rows.len()) {
        let at = top + offset;
        let base = ansi::fit(&rows[at], width);
        let line = format!("{}{}{}", ansi::slice(&base, 0, left), ansi::fit(&row.text, box_width), ansi::slice(&base, left + box_width, width));
        rows[at] = line;
        if let Some(target) = row.target {
            hits.push(Hit { row: at, from: left, to: left + box_width, target });
        }
    }
}

/// Draws a whole frame and records the hit map in `view.frame`. The frame is wrapped in
/// synchronized output so a terminal that supports it shows no flicker.
/// @param view - the view
/// @param width - the screen width
/// @param height - the screen height
pub fn render_frame(view: &mut View, width: usize, height: usize) -> String {
    let ctx = view.ctx();
    let (input, cursor_row, cursor_col) = input_rows(view, width);
    let status = status_row(view, width, &ctx);
    let panel = panel_rows(view, width, height, &ctx);
    let fixed = input.len() + usize::from(status.is_some()) + panel.len();
    let note_row = view.scroll > 0;
    let room = height.saturating_sub(fixed + usize::from(note_row)).max(1);
    let visible = visible_rows(view, width, room, &ctx);
    let mut hits = Vec::new();
    let mut lines = Vec::new();
    let mut rows: Vec<String> = Vec::new();
    for (index, (row, origin)) in visible.iter().enumerate() {
        rows.push(decorate(view, row, *origin, width));
        lines.push(*origin);
        if let Some(target) = row.target {
            hits.push(Hit { row: index, from: 0, to: width, target });
        }
    }
    rows.resize(room, String::new());
    lines.resize(room, None);
    draw_centered(view, &mut rows, width, &ctx, &mut hits);
    if note_row {
        rows.push(paint(&ctx, "2", &format!("↓ {} more rows below · PageDown, the mouse wheel or Ctrl+End to return", view.scroll)));
    }
    let panel_top = rows.len();
    for row in &panel {
        if let Some(target) = row.target {
            hits.push(Hit { row: rows.len(), from: 0, to: width, target });
        }
        rows.push(wrap_ansi(&row.text, width).swap_remove(0));
    }
    let panel_end = rows.len();
    if let Some((line, targets)) = status {
        for (from, to, target) in targets {
            hits.push(Hit { row: rows.len(), from, to, target });
        }
        rows.push(wrap_ansi(&line, width).swap_remove(0));
    }
    let input_top = rows.len();
    rows.extend(input);
    rows.truncate(height);
    let overlay_open = view.overlay.is_some();
    view.frame = FrameInfo { room, lines, hits, overlay: (panel_top, panel_end) };
    compose(&rows, height, input_top + cursor_row, cursor_col, !overlay_open)
}

/// Joins the rows into the bytes of a frame: each row cleared before it is written, then the
/// cursor placed in the input line.
/// @param rows - the rows, top first
/// @param height - the screen height
/// @param cursor_row - the cursor's screen row
/// @param cursor_col - the cursor's column
/// @param show_cursor - false while an overlay has the keyboard
fn compose(rows: &[String], height: usize, cursor_row: usize, cursor_col: usize, show_cursor: bool) -> String {
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
    out.push_str(&format!("\x1b[{};{}H", (cursor_row + 1).min(height), cursor_col + 1));
    if show_cursor {
        out.push_str("\x1b[?25h");
    }
    out.push_str("\x1b[?2026l");
    out
}

/// Every element focus can move to: the status bar's error box and usage first, then the
/// transcript's targets from the newest row to the oldest.
/// @param view - the view
pub fn focus_targets(view: &mut View) -> Vec<Target> {
    let ctx = view.ctx();
    let width = view.size.0.max(10);
    let mut targets = Vec::new();
    if live::mcp().is_some_and(|m| !m.failures().is_empty()) && view.status.is_some() {
        targets.push(Target::McpErrors);
    }
    if crate::services::usage_service::current().is_some() && view.status.is_some() {
        targets.push(Target::Usage);
    }
    for index in (0..view.transcript.len()).rev() {
        for row in view.transcript.rows_at(index, width, &ctx).into_iter().rev() {
            if let Some(target) = row.target {
                if !targets.contains(&target) {
                    targets.push(target);
                }
            }
        }
    }
    targets
}

/// Scrolls the transcript so the row of a target is on screen.
/// @param view - the view
/// @param target - the target
pub fn scroll_to(view: &mut View, target: Target) {
    let ctx = view.ctx();
    let width = view.size.0.max(10);
    let mut position = None;
    let mut total = 0;
    for index in 0..view.transcript.len() {
        let rows = view.transcript.rows_at(index, width, &ctx);
        if position.is_none() {
            if let Some(at) = rows.iter().position(|r| r.target == Some(target)) {
                position = Some(total + at);
            }
        }
        total += rows.len();
    }
    let Some(position) = position else { return };
    let room = view.frame.room.max(1);
    let start = total.saturating_sub(view.scroll + room);
    let end = total.saturating_sub(view.scroll);
    if position < start {
        view.scroll = total.saturating_sub(position + room);
    } else if position >= end {
        view.scroll = total.saturating_sub(position + 1);
    }
}
