//! Pie charts, drawn as one bar per slice: the label, a bar scaled to the largest slice, and the
//! slice's share in percent.

use std::sync::OnceLock;

use regex::Regex;

use crate::utils::screen::char_width;

/// The colors the bars take in turn, as SGR codes.
const COLORS: [&str; 6] = ["32", "33", "34", "35", "36", "31"];

/// The longest a bar may be.
const MAX_BAR: usize = 40;

/// A slice line: `"label" : value`.
fn slice_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"^"(.*?)"\s*:\s*([0-9]+(?:\.[0-9]+)?)"#).expect("valid regex"))
}

/// Draws a pie chart, or None when it has no slice or the bars would be too short to read.
/// @param header - the first line, which may hold `title` and `showData`
/// @param body - the other lines
/// @param width - the columns available
/// @param color - true to color the bars
pub fn render(header: &str, body: &[String], width: usize, color: bool) -> Option<Vec<String>> {
    let (mut title, mut show_data) = (title_of(header), header.contains("showData"));
    let mut slices: Vec<(String, f64)> = Vec::new();
    for line in body {
        if let Some(rest) = line.strip_prefix("title") {
            title = rest.trim().to_string();
        } else if line.trim() == "showData" {
            show_data = true;
        } else if let Some(caps) = slice_re().captures(line) {
            slices.push((caps[1].to_string(), caps[2].parse().ok()?));
        }
    }
    let total: f64 = slices.iter().map(|(_, v)| v).sum();
    if slices.is_empty() || total <= 0.0 {
        return None;
    }
    let mut rows = Vec::new();
    if !title.is_empty() {
        rows.push(if color { format!("\x1b[1m{title}\x1b[0m") } else { title.clone() });
    }
    rows.extend(bars(&slices, total, width, color, show_data)?);
    Some(rows)
}

/// The title after `pie title` on the first line.
/// @param header - the first line
fn title_of(header: &str) -> String {
    header.split_once("title").map(|(_, t)| t.replace("showData", "").trim().to_string()).unwrap_or_default()
}

/// The slice rows, or None when the bars would be shorter than 5 columns.
/// @param slices - the labels and values
/// @param total - the sum of the values
/// @param width - the columns available
/// @param color - true to color the bars
/// @param show_data - true to print each value after its percent
fn bars(slices: &[(String, f64)], total: f64, width: usize, color: bool, show_data: bool) -> Option<Vec<String>> {
    let label_width = slices.iter().map(|(l, _)| l.chars().map(char_width).sum::<usize>()).max().unwrap_or(0).min(width / 3);
    let tails: Vec<String> = slices.iter().map(|(_, v)| tail(*v, total, show_data)).collect();
    let tail_width = tails.iter().map(|t| t.chars().count()).max().unwrap_or(0);
    let room = width.checked_sub(label_width + 2 + tail_width + 1)?.min(MAX_BAR);
    if room < 5 {
        return None;
    }
    let largest = slices.iter().map(|(_, v)| *v).fold(0.0, f64::max);
    Some(slices.iter().zip(&tails).enumerate().map(|(index, ((label, value), tail))| {
        let length = ((value / largest) * room as f64).round().max(if *value > 0.0 { 1.0 } else { 0.0 }) as usize;
        let bar = "█".repeat(length);
        let bar = if color { format!("\x1b[{}m{bar}\x1b[0m", COLORS[index % COLORS.len()]) } else { bar };
        let label = fit(label, label_width);
        format!("{label}  {bar}{} {tail}", " ".repeat(room - length))
    }).collect())
}

/// The text after a bar: the percent, and the value when `showData` is on.
/// @param value - the slice's value
/// @param total - the sum of the values
/// @param show_data - true to add the value
fn tail(value: f64, total: f64, show_data: bool) -> String {
    let percent = format!("{:.1}%", value / total * 100.0);
    if show_data { format!("{percent} ({value})") } else { percent }
}

/// Pads or cuts a label to a width, ending a cut label with an ellipsis.
/// @param label - the label
/// @param width - the columns
fn fit(label: &str, width: usize) -> String {
    let used: usize = label.chars().map(char_width).sum();
    if used <= width {
        return format!("{label}{}", " ".repeat(width - used));
    }
    let mut out = String::new();
    let mut taken = 0;
    for c in label.chars() {
        if taken + char_width(c) + 1 > width {
            break;
        }
        out.push(c);
        taken += char_width(c);
    }
    format!("{out}…{}", " ".repeat(width.saturating_sub(taken + 1)))
}
