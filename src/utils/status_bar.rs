//! The status bar drawn under a run's output: what the agent is doing, how full the context window
//! is, how much room is left before compaction, and how many compactions the session has had.
//! This file only turns the numbers into one line of text that fits the terminal. Drawing it and
//! keeping it under the output is done by `console`.

/// What the agent is doing while the bar is shown.
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
    /// The compactions in this session, of both kinds.
    pub compactions: u32,
    /// How many of those were summaries.
    pub summaries: u32,
    /// The model calls in this session.
    pub step: u32,
    pub cost: f64,
    pub elapsed_secs: u64,
}

/// ANSI codes used by the bar.
const DIM: &str = "90";
const GREEN: &str = "32";
const YELLOW: &str = "33";
const RED: &str = "31";
const CYAN: &str = "36";
const MAGENTA: &str = "35";
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
    let segments = vec![phase_segment(&info.phase), context_segment(info), room_segment(info), compaction_segment(info), step_segment(info), cost_segment(info), elapsed_segment(info), model_segment(info)];
    let chosen = fit(&segments, width.saturating_sub(1));
    let mut line = vec![(PLAIN, " ".to_string())];
    let mut first = true;
    for pieces in chosen.into_iter().filter(|p| !p.is_empty()) {
        if !first {
            line.push((DIM, " │ ".into()));
        }
        first = false;
        line.extend(pieces);
    }
    paint_pieces(&line, color)
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

/// The dot and word for what the agent is doing.
/// @param phase - the current phase
fn phase_segment(phase: &Phase) -> Segment {
    let (code, word) = match phase {
        Phase::Starting => (DIM, "starting".to_string()),
        Phase::Thinking => (CYAN, "thinking".to_string()),
        Phase::Running(what) if what.is_empty() => (MAGENTA, "running".to_string()),
        Phase::Running(what) => (MAGENTA, format!("running {what}")),
        Phase::Compacting => (YELLOW, "compacting".to_string()),
        Phase::Waiting => (GREEN, "waiting for you".to_string()),
        Phase::Finished => (DIM, "finished".to_string()),
    };
    let short = word.split(' ').next().unwrap_or("").to_string();
    segment(vec![(6, vec![(code, format!("● {word}"))]), (9, vec![(code, format!("● {short}"))]), (0, vec![(code, "●".into())])])
}

/// The context bar, the share of the window in use, and the token counts.
/// @param info - the numbers to show
fn context_segment(info: &StatusInfo) -> Segment {
    let code = fill_color(info);
    let share = (code, format!("{:>3}%", percent_used(info)));
    let tokens = (PLAIN, format!("{} / {}", short_tokens(info.used), short_tokens(info.window)));
    let label = (DIM, "context ".to_string());
    let mut full = vec![label];
    full.extend(bar(info, 24));
    full.extend([(PLAIN, " ".into()), share.clone(), (DIM, " · ".into()), tokens.clone()]);
    let mut medium = bar(info, 16);
    medium.extend([(PLAIN, " ".into()), share.clone(), (PLAIN, " ".into()), tokens]);
    let mut small = bar(info, 10);
    small.extend([(PLAIN, " ".into()), share.clone()]);
    segment(vec![(7, full), (9, medium), (10, small), (0, vec![share])])
}

/// The room left before compaction starts.
/// @param info - the numbers to show
fn room_segment(info: &StatusInfo) -> Segment {
    let Some(trigger) = info.trigger else {
        return segment(vec![(8, vec![(DIM, "compaction off".into())]), (0, vec![])]);
    };
    if info.phase == Phase::Compacting {
        return segment(vec![(8, vec![(YELLOW, "compacting now".into())]), (0, vec![])]);
    }
    if info.used >= trigger {
        return segment(vec![(5, vec![(RED, "compacts before the next call".into())]), (8, vec![(RED, "compacting next".into())]), (0, vec![])]);
    }
    let left = short_tokens(trigger - info.used);
    let at = (trigger as f64 / info.window.max(1) as f64 * 100.0).round() as u64;
    let code = fill_color(info);
    segment(vec![
        (5, vec![(code, left.clone()), (PLAIN, format!(" left before compaction at {at}%"))]),
        (8, vec![(code, left.clone()), (PLAIN, " until compaction".into())]),
        (8, vec![(code, left), (DIM, " left".into())]),
        (0, vec![]),
    ])
}

/// The session's compaction count.
/// @param info - the numbers to show
fn compaction_segment(info: &StatusInfo) -> Segment {
    let count = info.compactions;
    let code = if count == 0 { DIM } else { YELLOW };
    let noun = if count == 1 { "compaction" } else { "compactions" };
    let detail = match info.summaries {
        0 => String::new(),
        1 => " (1 summary)".to_string(),
        s => format!(" ({s} summaries)"),
    };
    segment(vec![(4, vec![(code, format!("↻ {count} {noun}")), (DIM, detail)]), (8, vec![(code, format!("↻ {count} {noun}"))]), (0, vec![(code, format!("↻ {count}"))])])
}

/// The model call count.
/// @param info - the numbers to show
fn step_segment(info: &StatusInfo) -> Segment {
    segment(vec![(4, vec![(DIM, "step ".into()), (PLAIN, info.step.to_string())]), (0, vec![])])
}

/// The session's cost, left out when nothing was spent, as with a local model.
/// @param info - the numbers to show
fn cost_segment(info: &StatusInfo) -> Segment {
    if info.cost <= 0.0 {
        return segment(vec![(0, vec![])]);
    }
    segment(vec![(3, vec![(PLAIN, format!("${:.2}", info.cost))]), (0, vec![])])
}

/// The time since the run started.
/// @param info - the numbers to show
fn elapsed_segment(info: &StatusInfo) -> Segment {
    segment(vec![(2, vec![(DIM, short_duration(info.elapsed_secs))]), (0, vec![])])
}

/// The model name.
/// @param info - the numbers to show
fn model_segment(info: &StatusInfo) -> Segment {
    let name: String = info.model.rsplit(['/', '\\']).next().unwrap_or("").chars().take(28).collect();
    if name.is_empty() {
        return segment(vec![(0, vec![])]);
    }
    segment(vec![(1, vec![(DIM, name)]), (0, vec![])])
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

/// Writes a duration the short way: 42s, 12m04s, 1h05m.
/// @param secs - the duration in seconds
pub fn short_duration(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m{:02}s", secs / 60, secs % 60),
        _ => format!("{}h{:02}m", secs / 3600, secs % 3600 / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bar a third of the way to a 128k window, with compaction at 80%.
    fn sample() -> StatusInfo {
        StatusInfo { phase: Phase::Thinking, model: "local/qwen-3.8".into(), used: 42_600, window: 128_000, trigger: Some(102_400), compactions: 2, summaries: 1, step: 14, cost: 0.0, elapsed_secs: 724 }
    }

    #[test]
    fn a_wide_terminal_shows_every_part() {
        let line = render_status(&sample(), 200, false);
        assert!(line.contains("● thinking"), "{line}");
        assert!(line.contains("context ████████"), "{line}");
        assert!(line.contains(" 33% · 42.6k / 128k"), "{line}");
        assert!(line.contains("59.8k left before compaction at 80%"), "{line}");
        assert!(line.contains("↻ 2 compactions (1 summary)"), "{line}");
        assert!(line.contains("step 14"), "{line}");
        assert!(line.contains("12m04s"), "{line}");
        assert!(line.contains("qwen-3.8"), "{line}");
        assert!(!line.contains('$'), "a free model shows no cost: {line}");
    }

    #[test]
    fn the_line_always_fits_and_keeps_the_percent() {
        for width in [20, 40, 60, 80, 100, 120, 160] {
            let line = render_status(&sample(), width, false);
            let used = line.chars().count();
            assert!(used < width || width < 30, "width {width}: {used} columns: {line}");
            assert!(line.contains("33%"), "width {width}: {line}");
        }
        assert!(render_status(&sample(), 120, false).contains("● thinking │"), "the long phrases shorten before the phase word");
        let narrow = render_status(&sample(), 60, false);
        assert!(narrow.contains("↻ 2"), "the compaction count outlasts the step and model: {narrow}");
        assert!(!narrow.contains("qwen"), "{narrow}");
    }

    /// Prints the bar at several widths, in color. Run with `--ignored --nocapture` to look at it.
    #[test]
    #[ignore]
    fn shows_examples() {
        let mut info = sample();
        for width in [200, 150, 120, 100, 80, 60, 40] {
            println!("{width:>3} |{}", render_status(&info, width, true));
        }
        info.used = 96_000;
        info.cost = 0.4321;
        info.phase = Phase::Running("cargo +1".into());
        println!("    |{}", render_status(&info, 200, true));
        info.used = 3_000;
        info.compactions = 0;
        info.summaries = 0;
        info.phase = Phase::Waiting;
        println!("    |{}", render_status(&info, 200, true));
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
    fn colors_follow_the_distance_to_compaction() {
        let mut info = sample();
        assert_eq!(fill_color(&info), GREEN);
        info.used = 80_000;
        assert_eq!(fill_color(&info), YELLOW);
        info.used = 100_000;
        assert_eq!(fill_color(&info), RED);
        info.used = 110_000;
        assert!(render_status(&info, 200, false).contains("compacts before the next call"));
        info.phase = Phase::Compacting;
        assert!(render_status(&info, 200, false).contains("● compacting │ context"));
        assert!(render_status(&info, 200, false).contains("│ compacting now │"));
        info.phase = Phase::Thinking;
        info.trigger = None;
        assert!(render_status(&info, 200, false).contains("compaction off"));
    }

    #[test]
    fn counts_and_times_read_short() {
        assert_eq!(short_tokens(950), "950");
        assert_eq!(short_tokens(5_300), "5.3k");
        assert_eq!(short_tokens(128_000), "128k");
        assert_eq!(short_tokens(1_000_000), "1M");
        assert_eq!(short_tokens(1_250_000), "1.2M");
        assert_eq!(short_duration(42), "42s");
        assert_eq!(short_duration(3_900), "1h05m");
        let mut info = sample();
        info.compactions = 0;
        assert!(render_status(&info, 200, false).contains("↻ 0 compactions"));
        info.compactions = 1;
        info.summaries = 0;
        assert!(render_status(&info, 200, false).contains("↻ 1 compaction │"));
    }
}
