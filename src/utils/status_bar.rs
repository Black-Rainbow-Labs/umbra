//! The status bar drawn under a run's output: how full the context window is, the agent time, the
//! cost, the model, the weekly usage of a Claude login and, at the right edge, a box that counts
//! the MCP servers that failed. This file only turns the numbers into one line of text that fits
//! the terminal. Drawing it is done by `console` and by the fullscreen view.

/// What the agent is doing while the bar is shown. The bar itself only shows a compaction; the
/// rest is used by the inline view and by the spinner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// The run is being set up.
    Starting,
    /// A model call is in flight.
    Thinking,
    /// An action is running. The text names it, such as `cargo` or `github.search`.
    Running(String),
    /// The history is being compacted.
    Compacting,
    /// Umbra is waiting at a prompt for the person.
    Waiting,
    /// The run has ended.
    Finished,
}

/// The weekly usage of a subscription login, as the bar shows it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UsageView {
    /// From 0 to 100.
    pub percent: f32,
    /// True when the last poll failed and this is an older value, drawn dim.
    pub stale: bool,
}

/// The numbers the bar shows.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusInfo {
    pub phase: Phase,
    /// The model name, as the provider reports it.
    pub model: String,
    /// The estimated size of the next request in tokens.
    pub used: u64,
    /// The model's context window in tokens.
    pub window: u64,
    /// The request size that starts a compaction. None when compaction is off.
    pub trigger: Option<u64>,
    pub cost: f64,
    /// The agent time in seconds.
    pub work_secs: u64,
    /// True while the agent works: the agent time has a spinner in front of it.
    pub working: bool,
    /// The spinner character for this frame.
    pub spinner: char,
    /// The weekly usage, when the session uses a Claude login.
    pub usage: Option<UsageView>,
    /// How many MCP servers failed to start.
    pub mcp_errors: usize,
    /// A short note, such as `copied 12 characters`, shown before the error box.
    pub note: Option<String>,
}

impl Default for StatusInfo {
    /// An empty bar.
    fn default() -> Self {
        StatusInfo { phase: Phase::Starting, model: String::new(), used: 0, window: 0, trigger: None, cost: 0.0, work_secs: 0, working: false, spinner: '⠋', usage: None, mcp_errors: 0, note: None }
    }
}

/// A part of the bar that a click acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusPart {
    Usage,
    McpErrors,
}

/// ANSI codes used by the bar.
const DIM: &str = "90";
const GREEN: &str = "32";
const YELLOW: &str = "33";
const RED: &str = "31";
const CYAN: &str = "36";
const PLAIN: &str = "";

/// The partial block characters, from one eighth of a cell to seven eighths.
const EIGHTHS: [char; 7] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉'];

/// A run of text with one color.
type Piece = (&'static str, String);

/// One part of the bar. `variants` go from the longest form to the shortest; an empty variant
/// means the segment is left out. Each form has a priority, and when the line is too long the
/// form with the lowest priority is replaced by the next shorter one first.
struct Segment {
    variants: Vec<(u8, Vec<Piece>)>,
}

/// Builds a segment from its forms, longest first.
/// @param variants - each form with the priority of keeping it
fn segment(variants: Vec<(u8, Vec<Piece>)>) -> Segment {
    Segment { variants }
}

/// Builds the bar for a terminal `width` columns wide. The line is kept one column short of the
/// width so the terminal never wraps it.
/// @param info - the numbers to show
/// @param width - the terminal width in columns
/// @param color - true to add ANSI colors
pub fn render_status(info: &StatusInfo, width: usize, color: bool) -> String {
    render_status_parts(info, width, color).0
}

/// Builds the bar and says which columns the clickable parts take, counted in characters.
/// @param info - the numbers to show
/// @param width - the terminal width in columns
/// @param color - true to add ANSI colors
pub fn render_status_parts(info: &StatusInfo, width: usize, color: bool) -> (String, Vec<(usize, usize, StatusPart)>) {
    let right = right_pieces(info);
    let right_width = pieces_width(&right);
    let room = width.saturating_sub(1).saturating_sub(right_width + usize::from(right_width > 0));
    let segments = vec![context_segment(info), time_segment(info), cost_segment(info), model_segment(info), usage_segment(info)];
    let chosen = fit(&segments, room);
    let mut line = vec![(PLAIN, " ".to_string())];
    let mut parts = Vec::new();
    let mut first = true;
    for (index, pieces) in chosen.into_iter().enumerate() {
        if pieces.is_empty() {
            continue;
        }
        if !first {
            line.push((DIM, " │ ".into()));
        }
        first = false;
        let at = pieces_width(&line);
        let used = pieces_width(&pieces);
        line.extend(pieces);
        if index == 4 {
            parts.push((at, at + used, StatusPart::Usage));
        }
    }
    if right_width > 0 {
        let at = width.saturating_sub(1 + right_width).max(pieces_width(&line) + 1);
        line.push((PLAIN, " ".repeat(at - pieces_width(&line))));
        if info.mcp_errors > 0 {
            parts.push((at + right_width - error_box(info).chars().count(), at + right_width, StatusPart::McpErrors));
        }
        line.extend(right);
    }
    (paint_pieces(&line, color), parts)
}

/// The right aligned group: the note, then the MCP error box.
/// @param info - the numbers to show
fn right_pieces(info: &StatusInfo) -> Vec<Piece> {
    let mut pieces = Vec::new();
    if let Some(note) = &info.note {
        pieces.push((DIM, note.clone()));
        if info.mcp_errors > 0 {
            pieces.push((DIM, " │ ".into()));
        }
    }
    if info.mcp_errors > 0 {
        pieces.push((RED, error_box(info)));
    }
    pieces
}

/// The MCP error box's text: `▲ 1 MCP error`.
/// @param info - the numbers to show
fn error_box(info: &StatusInfo) -> String {
    format!("▲ {} MCP {}", info.mcp_errors, if info.mcp_errors == 1 { "error" } else { "errors" })
}

/// Picks a variant of each segment so the line fits. Starts from the longest forms and shrinks the
/// form with the lowest priority that has a shorter one, until the line fits or nothing can shrink.
/// @param segments - the bar's parts, in display order
/// @param room - the columns available
fn fit(segments: &[Segment], room: usize) -> Vec<Vec<Piece>> {
    let mut picks = vec![0usize; segments.len()];
    while line_width(segments, &picks) > room {
        let shrinkable = (0..segments.len()).filter(|&i| picks[i] + 1 < segments[i].variants.len()).min_by_key(|&i| segments[i].variants[picks[i]].0);
        match shrinkable {
            Some(index) => picks[index] += 1,
            None => break,
        }
    }
    segments.iter().zip(&picks).map(|(segment, &pick)| segment.variants[pick].1.clone()).collect()
}

/// The columns a choice of variants takes, separators and the leading space included.
/// @param segments - the bar's parts
/// @param picks - the chosen variant of each
fn line_width(segments: &[Segment], picks: &[usize]) -> usize {
    let widths: Vec<usize> = segments.iter().zip(picks).map(|(s, &p)| pieces_width(&s.variants[p].1)).filter(|w| *w > 0).collect();
    1 + widths.iter().sum::<usize>() + widths.len().saturating_sub(1) * 3
}

/// The columns a list of pieces takes.
/// @param pieces - colored text runs
fn pieces_width(pieces: &[Piece]) -> usize {
    pieces.iter().map(|(_, text)| text.chars().count()).sum()
}

/// Joins pieces into one string, with color codes when color is on.
/// @param pieces - colored text runs
/// @param color - true to add ANSI colors
fn paint_pieces(pieces: &[Piece], color: bool) -> String {
    pieces.iter().filter(|(_, text)| !text.is_empty()).map(|(code, text)| if color && !code.is_empty() { format!("\x1b[{code}m{text}\x1b[0m") } else { text.clone() }).collect()
}

/// The context bar, the share of the window in use, and the token counts. During a compaction
/// the label reads `compacting`.
/// @param info - the numbers to show
fn context_segment(info: &StatusInfo) -> Segment {
    if info.window == 0 {
        return segment(vec![(0, vec![])]);
    }
    let code = fill_color(info);
    let share = (code, format!("{:>3}%", percent_used(info)));
    let tokens = (PLAIN, format!("{} / {}", short_tokens(info.used), short_tokens(info.window)));
    let label = if info.phase == Phase::Compacting { (YELLOW, "compacting ".to_string()) } else { (DIM, "context ".to_string()) };
    let mut full = vec![label.clone()];
    full.extend(bar(info, 24));
    full.extend([(PLAIN, " ".into()), share.clone(), (DIM, " · ".into()), tokens.clone()]);
    let mut medium = vec![label];
    medium.extend(bar(info, 16));
    medium.extend([(PLAIN, " ".into()), share.clone(), (PLAIN, " ".into()), tokens]);
    let mut small = bar(info, 10);
    small.extend([(PLAIN, " ".into()), share.clone()]);
    segment(vec![(7, full), (8, medium), (9, small), (10, vec![share])])
}

/// The agent time, with a spinner in front while the agent works. It is never left out.
/// @param info - the numbers to show
fn time_segment(info: &StatusInfo) -> Segment {
    let time = crate::services::work_clock::clock_text(std::time::Duration::from_secs(info.work_secs));
    let pieces = if info.working { vec![(CYAN, format!("{} ", info.spinner)), (PLAIN, time)] } else { vec![(DIM, time)] };
    segment(vec![(255, pieces)])
}

/// The session's cost, left out when nothing was spent, as with a local model.
/// @param info - the numbers to show
fn cost_segment(info: &StatusInfo) -> Segment {
    if info.cost <= 0.0 {
        return segment(vec![(0, vec![])]);
    }
    segment(vec![(3, vec![(PLAIN, format!("${:.2}", info.cost))]), (0, vec![])])
}

/// The model name.
/// @param info - the numbers to show
fn model_segment(info: &StatusInfo) -> Segment {
    let name: String = info.model.rsplit(['/', '\\']).next().unwrap_or("").chars().take(28).collect();
    if name.is_empty() {
        return segment(vec![(0, vec![])]);
    }
    segment(vec![(4, vec![(DIM, name)]), (0, vec![])])
}

/// The weekly usage: a short bar and the percent, green below 70%, yellow below 90%, red from
/// 90%, dim when the value is old. It shortens to the percent alone before anything else does.
/// @param info - the numbers to show
fn usage_segment(info: &StatusInfo) -> Segment {
    let Some(usage) = info.usage else { return segment(vec![(0, vec![])]) };
    let code = if usage.stale { DIM } else if usage.percent < 70.0 { GREEN } else if usage.percent < 90.0 { YELLOW } else { RED };
    let cells = 6;
    let filled = ((usage.percent.clamp(0.0, 100.0) / 100.0) * cells as f32).round() as usize;
    let percent = format!("{:.0}%", usage.percent);
    let full = vec![(DIM, "week ".to_string()), (code, "█".repeat(filled)), (DIM, "░".repeat(cells - filled)), (code, format!(" {percent}"))];
    segment(vec![(1, full), (6, vec![(DIM, "week ".to_string()), (code, percent)]), (0, vec![])])
}

/// Draws the bar itself: filled cells up to the share in use, with eighth blocks for the last
/// cell, and a marker where compaction starts.
/// @param info - the numbers to show
/// @param cells - the bar's width in columns
fn bar(info: &StatusInfo, cells: usize) -> Vec<Piece> {
    let window = info.window.max(1) as f64;
    let eighths = ((info.used as f64 / window).clamp(0.0, 1.0) * (cells * 8) as f64).round() as usize;
    let marker = info.trigger.map(|t| ((t as f64 / window) * cells as f64).floor() as usize).filter(|m| *m < cells);
    let fill = fill_color(info);
    let mut filled = String::new();
    let mut rest: Vec<Piece> = Vec::new();
    for cell in 0..cells {
        let amount = eighths.saturating_sub(cell * 8).min(8);
        match amount {
            8 => filled.push('█'),
            1..=7 => filled.push(EIGHTHS[amount - 1]),
            _ if Some(cell) == marker => rest.push((YELLOW, "┊".into())),
            _ => rest.push((DIM, "░".into())),
        }
    }
    let mut pieces = vec![(fill, filled)];
    pieces.extend(merge_runs(rest));
    pieces
}

/// Joins neighboring pieces of the same color, so the colored line has fewer escape codes.
/// @param pieces - colored text runs
fn merge_runs(pieces: Vec<Piece>) -> Vec<Piece> {
    let mut merged: Vec<Piece> = Vec::new();
    for (code, text) in pieces {
        match merged.last_mut() {
            Some((last, run)) if *last == code => run.push_str(&text),
            _ => merged.push((code, text)),
        }
    }
    merged
}

/// The bar's color: green while far from compaction, yellow as it nears, red close to it. With
/// compaction off the window itself is the limit.
/// @param info - the numbers to show
fn fill_color(info: &StatusInfo) -> &'static str {
    let limit = info.trigger.unwrap_or(info.window).max(1) as f64;
    let share = info.used as f64 / limit;
    if share < 0.6 {
        GREEN
    } else if share < 0.85 {
        YELLOW
    } else {
        RED
    }
}

/// The share of the window in use, as a whole percent.
/// @param info - the numbers to show
fn percent_used(info: &StatusInfo) -> u64 {
    ((info.used as f64 / info.window.max(1) as f64) * 100.0).round() as u64
}

/// Writes a token count the short way: 950, 5.3k, 54k, 128k, 1.2M.
/// @param tokens - the count
pub fn short_tokens(tokens: u64) -> String {
    match tokens {
        0..=999 => tokens.to_string(),
        1_000..=99_999 => trim_zero(format!("{:.1}k", tokens as f64 / 1_000.0)),
        100_000..=999_999 => format!("{}k", (tokens as f64 / 1_000.0).round() as u64),
        _ => trim_zero(format!("{:.1}M", tokens as f64 / 1_000_000.0)),
    }
}

/// Removes a `.0` before the unit, so 128.0k reads 128k.
/// @param text - a formatted count
fn trim_zero(text: String) -> String {
    text.replace(".0k", "k").replace(".0M", "M")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bar a third of the way to a 128k window, with compaction at 80%.
    fn sample() -> StatusInfo {
        StatusInfo { phase: Phase::Thinking, model: "local/qwen-3.8".into(), used: 42_600, window: 128_000, trigger: Some(102_400), work_secs: 187, working: true, ..Default::default() }
    }

    #[test]
    fn a_wide_terminal_shows_every_part_and_no_removed_one() {
        let mut info = sample();
        info.cost = 0.43;
        info.usage = Some(UsageView { percent: 23.0, stale: false });
        info.mcp_errors = 1;
        let line = render_status(&info, 200, false);
        for wanted in ["context ████████", " 33% · 42.6k / 128k", "⠋ 3:07", "$0.43", "qwen-3.8", "week █░░░░░ 23%"] {
            assert!(line.contains(wanted), "{wanted}: {line}");
        }
        assert!(line.ends_with("▲ 1 MCP error"), "the error box is at the right edge: {line}");
        assert_eq!(line.chars().count(), 199, "{line}");
        for gone in ["thinking", "waiting", "left before compaction", "compactions", "step"] {
            assert!(!line.contains(gone), "{gone}: {line}");
        }
    }

    #[test]
    fn the_line_always_fits_and_keeps_the_time_and_the_error_box() {
        let mut info = sample();
        info.mcp_errors = 2;
        info.usage = Some(UsageView { percent: 91.0, stale: false });
        for width in [40, 60, 80, 100, 120, 160] {
            let line = render_status(&info, width, false);
            let used = line.chars().count();
            assert!(used < width, "width {width}: {used} columns: {line}");
            assert!(line.contains("3:07") && line.contains("▲ 2 MCP errors"), "width {width}: {line}");
        }
        let narrow = render_status(&info, 80, false);
        assert!(narrow.contains("week 91%") || !narrow.contains("week"), "usage shortens to the percent first: {narrow}");
        let idle = StatusInfo { working: false, ..sample() };
        assert!(!render_status(&idle, 120, false).contains('⠋'));
    }

    #[test]
    fn the_clickable_parts_report_their_columns() {
        let mut info = sample();
        info.mcp_errors = 1;
        info.usage = Some(UsageView { percent: 50.0, stale: false });
        let (line, parts) = render_status_parts(&info, 120, false);
        let chars: Vec<char> = line.chars().collect();
        assert_eq!(parts.len(), 2, "{line}");
        for (from, to, part) in parts {
            let text: String = chars[from..to].iter().collect();
            match part {
                StatusPart::McpErrors => assert_eq!(text, "▲ 1 MCP error"),
                StatusPart::Usage => assert!(text.starts_with("week"), "{text}"),
            }
        }
    }

    #[test]
    fn the_bar_marks_the_compaction_point() {
        let mut info = sample();
        info.used = 0;
        let cells: String = bar(&info, 10).into_iter().map(|(_, t)| t).collect();
        assert_eq!(cells, "░░░░░░░░┊░");
        info.used = 64_000;
        let cells: String = bar(&info, 10).into_iter().map(|(_, t)| t).collect();
        assert_eq!(cells, "█████░░░┊░");
        info.used = 70_400;
        let cells: String = bar(&info, 10).into_iter().map(|(_, t)| t).collect();
        assert_eq!(cells, "█████▌░░┊░");
    }

    #[test]
    fn colors_follow_the_distance_to_compaction_and_a_compaction_is_named() {
        let mut info = sample();
        assert_eq!(fill_color(&info), GREEN);
        info.used = 80_000;
        assert_eq!(fill_color(&info), YELLOW);
        info.used = 100_000;
        assert_eq!(fill_color(&info), RED);
        info.phase = Phase::Compacting;
        assert!(render_status(&info, 200, false).contains(" compacting ███"));
    }

    #[test]
    fn counts_read_short() {
        assert_eq!(short_tokens(950), "950");
        assert_eq!(short_tokens(5_300), "5.3k");
        assert_eq!(short_tokens(128_000), "128k");
        assert_eq!(short_tokens(1_000_000), "1M");
        assert_eq!(short_tokens(1_250_000), "1.2M");
    }
}
