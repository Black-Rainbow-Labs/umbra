//! The items of the fullscreen transcript and how each lays itself out at a screen width. A block
//! keeps what it shows as data, so it can be drawn again at another width, collapsed, expanded or
//! updated while a command runs.

use std::time::{Duration, Instant};

use crate::services::interaction_service::Tone;
use crate::services::work_clock::clock_text;
use crate::utils::ansi;
use crate::utils::screen::{char_width, visible_width, wrap_ansi, Layout};

/// Something on screen a click or the keyboard can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The header's model row: opens the model picker.
    HeaderModel,
    /// The header's mcp row: opens the MCP panel.
    HeaderMcp,
    /// A command group's header row, by block id: expands or collapses it.
    Group(u64),
    /// One run in an expanded group, by block id and position: expands or collapses its details.
    Run(u64, usize),
    /// The status bar's MCP error box: shows or hides the errors.
    McpErrors,
    /// The status bar's weekly usage: shows when it resets.
    Usage,
    /// An item of the open overlay, by position.
    Item(usize),
    /// A row of the command popup, by position.
    Popup(usize),
}

/// One laid out row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Row {
    /// The text, with color codes, at most the screen width.
    pub text: String,
    /// True when the row continues the logical line above it, so a copy joins them.
    pub cont: bool,
    /// Columns at the start that are decoration and are left out of a copy.
    pub gutter: usize,
    /// What a click on this row does.
    pub target: Option<Target>,
    /// The column where the focus marker `›` is drawn when the row has focus.
    pub focus_col: Option<usize>,
}

impl Row {
    /// A row of text with nothing to click.
    /// @param text - the text, with color codes
    pub fn text(text: impl Into<String>) -> Self {
        Row { text: text.into(), ..Default::default() }
    }
}

/// What a laid out block needs to know about the frame it is drawn in.
#[derive(Debug, Clone, Copy)]
pub struct LayoutCtx {
    pub color: bool,
    pub now: Instant,
    /// The spinner character for this frame.
    pub spinner: char,
}

/// One item in the transcript.
pub enum Block {
    /// The header: logo, version, model, folder, mode, skills and MCP state, from live state.
    Header,
    /// What the person typed, after a marker: `❯` for a task, `›` for a line sent while the agent works.
    UserPrompt { marker: String, text: String },
    /// A reply from the model, rendered from markdown.
    Reply { markdown: String },
    /// Commands run in a row, shown as one collapsible box.
    CommandGroup(CommandGroup),
    /// A warning, a compaction note or why the agent stopped.
    Notice { tone: Tone, marker: String, text: String },
    /// A printed line that has no structure of its own.
    Text(String),
    /// Rows laid out again for each width by a function.
    Laid(Layout),
}

/// Commands run in a row.
#[derive(Debug, Clone, Default)]
pub struct CommandGroup {
    pub runs: Vec<CommandRun>,
    pub expanded: bool,
}

/// What a run is.
#[derive(Debug, Clone, PartialEq)]
pub enum RunKind {
    Shell { command: String },
    Mcp { server: String, tool: String, args: String },
}

/// Where a run is.
#[derive(Debug, Clone, PartialEq)]
pub enum RunState {
    Pending,
    Running { since: Instant },
    Done,
    NotRun { reason: String },
}

/// One command or MCP call in a group.
#[derive(Debug, Clone)]
pub struct CommandRun {
    pub kind: RunKind,
    pub state: RunState,
    pub output: String,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub duration: Option<Duration>,
    pub expanded: bool,
}

impl CommandRun {
    /// A run that has started now.
    /// @param kind - the command or the MCP call
    pub fn running(kind: RunKind) -> Self {
        CommandRun { kind, state: RunState::Running { since: Instant::now() }, output: String::new(), exit_code: None, error: None, duration: None, expanded: false }
    }

    /// True when the run finished with a nonzero exit code or an error.
    pub fn failed(&self) -> bool {
        self.state == RunState::Done && (self.exit_code.is_some_and(|c| c != 0) || self.error.as_ref().is_some_and(|e| !e.is_empty()))
    }

    /// The time the run took, or has taken so far while it runs.
    /// @param now - the frame's time
    pub fn time(&self, now: Instant) -> Duration {
        match self.state {
            RunState::Running { since } => now.saturating_duration_since(since),
            _ => self.duration.unwrap_or_default(),
        }
    }
}

/// The most output rows an expanded run shows.
pub const MAX_OUTPUT_ROWS: usize = 400;

impl Block {
    /// True when the block's rows change with time or live state, so they are never cached.
    pub fn is_live(&self) -> bool {
        match self {
            Block::Header | Block::Laid(_) => true,
            Block::CommandGroup(group) => group.runs.iter().any(|r| matches!(r.state, RunState::Running { .. })),
            _ => false,
        }
    }

    /// Lays the block out at a width.
    /// @param id - the block's id, for click targets
    /// @param width - the screen width
    /// @param ctx - the frame's color setting, time and spinner
    pub fn layout(&self, id: u64, width: usize, ctx: &LayoutCtx) -> Vec<Row> {
        let width = width.max(10);
        match self {
            Block::Header => super::header::header_rows(width, ctx),
            Block::UserPrompt { marker, text } => user_rows(marker, text, width, ctx),
            Block::Reply { markdown } => reply_rows(markdown, width, ctx),
            Block::CommandGroup(group) => group_rows(group, id, width, ctx),
            Block::Notice { tone, marker, text } => notice_rows(*tone, marker, text, width, ctx),
            Block::Text(line) => hard_wrap(line, width),
            Block::Laid(layout) => layout(width).iter().flat_map(|row| hard_wrap(row, width)).collect(),
        }
    }
}

/// Wraps a line at the width, marking the rows after the first as continuations.
/// @param line - the line, with color codes
/// @param width - the screen width
pub fn hard_wrap(line: &str, width: usize) -> Vec<Row> {
    wrap_ansi(line, width).into_iter().enumerate().map(|(i, text)| Row { text, cont: i > 0, ..Default::default() }).collect()
}

/// Paints text with a color code when color is on.
/// @param ctx - the frame
/// @param code - the SGR code
/// @param text - the text
pub fn paint(ctx: &LayoutCtx, code: &str, text: &str) -> String {
    if ctx.color && !code.is_empty() { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
}

/// Wraps plain text at spaces into lines of at most `room` columns. Each line of the text is
/// wrapped on its own; a word longer than the room is broken. Returns each row and whether it
/// continues the line above.
/// @param text - the text, with no color codes
/// @param room - the most columns in a row
pub fn word_wrap(text: &str, room: usize) -> Vec<(String, bool)> {
    let room = room.max(1);
    let mut rows = Vec::new();
    for line in text.split('\n') {
        let mut current = String::new();
        let mut used = 0;
        let mut first = true;
        for word in line.split(' ') {
            let width: usize = word.chars().map(char_width).sum();
            let gap = usize::from(used > 0);
            if used + gap + width > room && used > 0 {
                rows.push((std::mem::take(&mut current), !first));
                first = false;
                used = 0;
            } else if gap == 1 {
                current.push(' ');
                used += 1;
            }
            for c in word.chars() {
                let w = char_width(c);
                if used + w > room && used > 0 {
                    rows.push((std::mem::take(&mut current), !first));
                    first = false;
                    used = 0;
                }
                current.push(c);
                used += w;
            }
        }
        rows.push((current, !first));
    }
    rows
}

/// Removes escape sequences and control characters from text a command printed, keeping color
/// codes, newlines and tabs.
/// @param text - the output
pub fn clean_output(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            let mut sequence = String::from("\x1b");
            ansi::copy_escape(&mut chars, &mut sequence);
            if sequence.starts_with("\x1b[") && sequence.ends_with('m') && sequence[2..sequence.len() - 1].chars().all(|c| c.is_ascii_digit() || c == ';') {
                out.push_str(&sequence);
            }
        } else if c == '\n' || c == '\t' || (c as u32 >= 0x20 && c != '\x7f') {
            out.push(c);
        }
    }
    out.replace("\r\n", "\n")
}

/// The rows of what the person typed: a blank row, then the marker and the text.
/// @param marker - `❯` or `›`
/// @param text - what they typed
/// @param width - the screen width
/// @param ctx - the frame
fn user_rows(marker: &str, text: &str, width: usize, ctx: &LayoutCtx) -> Vec<Row> {
    let mut rows = vec![Row::default()];
    let clean = super::markdown::sanitize(text.trim());
    for (i, (line, cont)) in word_wrap(&clean, width.saturating_sub(2)).into_iter().enumerate() {
        let lead = if i == 0 { format!("{} ", paint(ctx, "1;33", marker)) } else { "  ".to_string() };
        rows.push(Row { text: format!("{lead}{line}"), cont, gutter: 2, ..Default::default() });
    }
    rows
}

/// The rows of a reply: a blank row, then the rendered markdown after a `●`, indented under it.
/// @param markdown - the reply source
/// @param width - the screen width
/// @param ctx - the frame
fn reply_rows(markdown: &str, width: usize, ctx: &LayoutCtx) -> Vec<Row> {
    let mut rows = vec![Row::default()];
    let rendered = super::markdown::render(markdown.trim(), width.saturating_sub(2), ctx.color);
    for (i, row) in rendered.into_iter().enumerate() {
        let lead = if i == 0 { format!("{} ", paint(ctx, "36", "●")) } else { "  ".to_string() };
        rows.push(Row { text: format!("{lead}{}", row.text), cont: row.cont, gutter: 2 + row.gutter, ..Default::default() });
    }
    rows
}

/// The rows of a notice, in the same form the inline view prints: dim for routine events, yellow
/// with `▲` for warnings, red with `✖` for failures.
/// @param tone - how much attention it asks for
/// @param marker - the marker of a routine event
/// @param text - the notice
/// @param width - the screen width
/// @param ctx - the frame
fn notice_rows(tone: Tone, marker: &str, text: &str, width: usize, ctx: &LayoutCtx) -> Vec<Row> {
    let clean = super::markdown::sanitize(text.trim());
    let (lead, indent, code, blank) = match tone {
        Tone::Info => (format!("  {marker} "), 4, "90", false),
        Tone::Warning => ("▲ ".to_string(), 2, "33", true),
        Tone::Failure => ("✖ ".to_string(), 2, "31", true),
    };
    let mut rows = if blank { vec![Row::default()] } else { Vec::new() };
    let lead_width = visible_width(&lead);
    for (i, (line, cont)) in word_wrap(&clean, width.saturating_sub(lead_width.max(indent))).into_iter().enumerate() {
        let start = if i == 0 { lead.clone() } else { " ".repeat(indent) };
        rows.push(Row { text: paint(ctx, code, &format!("{start}{line}")), cont, gutter: if i == 0 { lead_width } else { indent }, ..Default::default() });
    }
    rows
}

/// The header row of a command group: the arrow, the state icon, what ran and the total time.
/// @param group - the group
/// @param ctx - the frame
pub fn group_title(group: &CommandGroup, ctx: &LayoutCtx) -> String {
    let running = group.runs.iter().any(|r| matches!(r.state, RunState::Running { .. }));
    let not_run = group.runs.iter().filter(|r| matches!(r.state, RunState::NotRun { .. })).count();
    let failed = group.runs.iter().filter(|r| r.failed()).count();
    let counted: Vec<&CommandRun> = group.runs.iter().filter(|r| !matches!(r.state, RunState::NotRun { .. })).collect();
    let shells = counted.iter().filter(|r| matches!(r.kind, RunKind::Shell { .. })).count();
    let calls = counted.len() - shells;
    let mut what = Vec::new();
    if shells > 0 || calls == 0 {
        what.push(format!("{shells} shell {}", if shells == 1 { "command" } else { "commands" }));
    }
    if calls > 0 {
        what.push(format!("{calls} MCP {}", if calls == 1 { "call" } else { "calls" }));
    }
    let mut text = format!("{} {}", if running { "Running" } else { "Ran" }, what.join(" and "));
    if failed > 0 {
        text.push_str(&format!(", {failed} failed"));
    }
    if not_run > 0 {
        text.push_str(&format!(", {not_run} not run"));
    }
    let total: Duration = group.runs.iter().map(|r| r.time(ctx.now)).sum();
    let icon = if running { paint(ctx, "36", &ctx.spinner.to_string()) } else if failed > 0 { paint(ctx, "31", "✖") } else { paint(ctx, "32", "✔") };
    let arrow = if group.expanded { "▾" } else { "▸" };
    format!(" {} {icon} {text} {}", paint(ctx, "90", arrow), paint(ctx, "90", &format!("· {}", clock_text(total))))
}

/// The rows of a command group: a blank row, the header, and when expanded one row per run with
/// the details of each expanded run under it.
/// @param group - the group
/// @param id - the block id
/// @param width - the screen width
/// @param ctx - the frame
fn group_rows(group: &CommandGroup, id: u64, width: usize, ctx: &LayoutCtx) -> Vec<Row> {
    let mut rows = vec![Row::default()];
    let title = ansi::fit(&group_title(group, ctx), width);
    rows.push(Row { text: title.trim_end().to_string(), target: Some(Target::Group(id)), focus_col: Some(0), ..Default::default() });
    if !group.expanded {
        return rows;
    }
    for (index, run) in group.runs.iter().enumerate() {
        rows.push(Row { text: run_row(run, width, ctx), target: Some(Target::Run(id, index)), focus_col: Some(2), ..Default::default() });
        if run.expanded {
            rows.extend(run_details(run, width, ctx));
        }
    }
    rows
}

/// One run's row: the arrow, the command cut to one row, the time and the exit code.
/// @param run - the run
/// @param width - the screen width
/// @param ctx - the frame
fn run_row(run: &CommandRun, width: usize, ctx: &LayoutCtx) -> String {
    let what = match &run.kind {
        RunKind::Shell { command } => format!("$ {}", command.trim().lines().next().unwrap_or("")),
        RunKind::Mcp { server, tool, args } => format!("→ {server}.{tool} {args}"),
    };
    let right = match &run.state {
        RunState::Pending => "waiting".to_string(),
        RunState::Running { .. } => format!("{} {}", ctx.spinner, clock_text(run.time(ctx.now))),
        RunState::Done => format!("{}  exit {}", clock_text(run.time(ctx.now)), run.exit_code.unwrap_or(0)),
        RunState::NotRun { .. } => "not run".to_string(),
    };
    let arrow = if run.expanded { "▾" } else { "▸" };
    let room = width.saturating_sub(5 + visible_width(&right) + 2).max(8);
    let what = super::markdown::sanitize(&what);
    let cut: String = cut_to(&what, room);
    let pad = room.saturating_sub(visible_width(&cut));
    let code = if run.failed() { "31" } else { "90" };
    format!("   {} {}{}  {}", paint(ctx, "90", arrow), paint(ctx, "35", &cut), " ".repeat(pad), paint(ctx, code, &right))
}

/// Cuts plain text to at most `room` columns, with an ellipsis when cut.
/// @param text - the text
/// @param room - the most columns
pub fn cut_to(text: &str, room: usize) -> String {
    if visible_width(text) <= room {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = char_width(c);
        if used + w + 1 > room {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

/// The details of an expanded run: the whole command, the output, the exit code and the time.
/// @param run - the run
/// @param width - the screen width
/// @param ctx - the frame
fn run_details(run: &CommandRun, width: usize, ctx: &LayoutCtx) -> Vec<Row> {
    const INDENT: usize = 6;
    let room = width.saturating_sub(INDENT).max(4);
    let mut rows = Vec::new();
    let command = match &run.kind {
        RunKind::Shell { command } => format!("$ {}", command.trim()),
        RunKind::Mcp { server, tool, args } => format!("→ {server}.{tool} {args}"),
    };
    for (line, cont) in word_wrap(&super::markdown::sanitize(&command), room) {
        rows.push(Row { text: format!("{}{}", " ".repeat(INDENT), paint(ctx, "35", &line)), cont, gutter: INDENT, ..Default::default() });
    }
    let output = clean_output(run.output.trim_end());
    let mut output_rows: Vec<Row> = output.lines().flat_map(|line| hard_wrap(line, room)).collect();
    let total = output_rows.len();
    output_rows.truncate(MAX_OUTPUT_ROWS);
    for row in output_rows {
        rows.push(Row { text: format!("{}{}", " ".repeat(INDENT), row.text), cont: row.cont, gutter: INDENT, ..Default::default() });
    }
    if total > MAX_OUTPUT_ROWS {
        rows.push(Row::text(format!("{}{}", " ".repeat(INDENT), paint(ctx, "90", &format!("… {} more rows; the trajectory file has the whole output", total - MAX_OUTPUT_ROWS)))));
    }
    if output.is_empty() && run.state == RunState::Done {
        rows.push(Row::text(format!("{}{}", " ".repeat(INDENT), paint(ctx, "90", "(no output)"))));
    }
    for line in run.error.iter().flat_map(|e| e.lines()) {
        rows.push(Row { text: format!("{}{}", " ".repeat(INDENT), paint(ctx, "31", &super::markdown::sanitize(line))), gutter: INDENT, ..Default::default() });
    }
    let end = match &run.state {
        RunState::Done => format!("exit {} · {}", run.exit_code.unwrap_or(0), clock_text(run.time(ctx.now))),
        RunState::NotRun { reason } => format!("not run: {}", super::markdown::sanitize(reason)),
        _ => format!("running · {}", clock_text(run.time(ctx.now))),
    };
    let code = if run.failed() { "31" } else { "90" };
    for (line, cont) in word_wrap(&end, room) {
        rows.push(Row { text: format!("{}{}", " ".repeat(INDENT), paint(ctx, code, &line)), cont, gutter: INDENT, ..Default::default() });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> LayoutCtx {
        LayoutCtx { color: false, now: Instant::now(), spinner: '⠋' }
    }

    fn done(command: &str, code: i32, secs: u64) -> CommandRun {
        CommandRun { kind: RunKind::Shell { command: command.into() }, state: RunState::Done, output: "ok\n".into(), exit_code: Some(code), error: None, duration: Some(Duration::from_secs(secs)), expanded: false }
    }

    #[test]
    fn a_group_header_counts_the_runs_and_adds_their_times() {
        let mut group = CommandGroup { runs: vec![done("ls", 0, 30), done("cargo test", 0, 12)], expanded: false };
        assert_eq!(group_title(&group, &ctx()), " ▸ ✔ Ran 2 shell commands · 0:42");
        group.runs.push(done("false", 1, 0));
        assert_eq!(group_title(&group, &ctx()), " ▸ ✖ Ran 3 shell commands, 1 failed · 0:42");
        group.runs.push(CommandRun { kind: RunKind::Mcp { server: "gh".into(), tool: "search".into(), args: "{}".into() }, ..done("", 0, 0) });
        assert!(group_title(&group, &ctx()).contains("Ran 3 shell commands and 1 MCP call, 1 failed"));
        group.runs.push(CommandRun::running(RunKind::Shell { command: "sleep 9".into() }));
        assert!(group_title(&group, &ctx()).starts_with(" ▸ ⠋ Running 4 shell commands"), "{}", group_title(&group, &ctx()));
    }

    #[test]
    fn an_expanded_group_shows_a_row_per_run_and_the_details_of_an_expanded_run() {
        let mut group = CommandGroup { runs: vec![done("cargo test --lib", 0, 31), done("ls", 0, 1)], expanded: true };
        group.runs[0].expanded = true;
        let rows = group_rows(&group, 7, 60, &ctx());
        let text: Vec<String> = rows.iter().map(|r| ansi::plain(&r.text)).collect();
        assert!(text[2].starts_with("   ▾ $ cargo test --lib") && text[2].ends_with("0:31  exit 0"), "{text:#?}");
        assert_eq!(rows[2].target, Some(Target::Run(7, 0)));
        assert!(text.iter().any(|t| t == "      ok"), "{text:#?}");
        assert!(text.iter().any(|t| t == "      exit 0 · 0:31"), "{text:#?}");
        assert!(text.iter().all(|t| visible_width(t) <= 60));
    }

    #[test]
    fn output_keeps_colors_and_drops_other_escapes() {
        assert_eq!(clean_output("a\x1b[31mred\x1b[0m\x1b]0;title\x07\x1b[2Jb\r\n"), "a\x1b[31mred\x1b[0mb\n");
    }

    #[test]
    fn word_wrap_breaks_at_spaces_and_marks_continuations() {
        assert_eq!(word_wrap("aaa bbb ccc", 7), vec![("aaa bbb".to_string(), false), ("ccc".to_string(), true)]);
        assert_eq!(word_wrap("abcdefgh", 3), vec![("abc".to_string(), false), ("def".to_string(), true), ("gh".to_string(), true)]);
        assert_eq!(word_wrap("a\nb", 9), vec![("a".to_string(), false), ("b".to_string(), false)]);
    }
}
