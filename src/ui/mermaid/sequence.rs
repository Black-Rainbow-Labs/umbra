//! Sequence diagrams: a box and a lifeline for each participant, and for each message a label row
//! and an arrow row between two lifelines. Notes are boxes, and `loop`, `alt` and the other blocks
//! are dotted rows with their text.

use std::sync::OnceLock;

use regex::Regex;

use super::canvas::{draw_box, Canvas, Stroke};
use super::label_lines;
use crate::utils::screen::char_width;

/// A participant: its id in the messages, the label in its box, and whether it is an actor.
#[derive(Debug, Clone)]
struct Participant {
    id: String,
    label: String,
    actor: bool,
}

/// Where a note goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Left,
    Right,
    Over,
}

/// One row group of the diagram, in order.
#[derive(Debug, Clone)]
enum Event {
    Message { from: usize, to: usize, text: String, head: Option<(char, char)>, dashed: bool },
    Note { place: Place, a: usize, b: usize, text: String },
    Block(String),
}

/// The parsed diagram.
#[derive(Debug, Default)]
struct Diagram {
    participants: Vec<Participant>,
    events: Vec<Event>,
}

/// A message line: sender, arrow, receiver and text.
fn message_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(.+?)\s*(--?>>|--?>|--?x|--?\))\s*[+-]?\s*(.+?)\s*:\s*(.*)$").expect("valid regex"))
}

/// A note line.
fn note_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^note\s+(left of|right of|over)\s+([^:,]+?)\s*(?:,\s*([^:]+?))?\s*:\s*(.*)$").expect("valid regex"))
}

/// Draws a sequence diagram, or None when it has no participant.
/// @param body - the lines after `sequenceDiagram`
pub fn render(body: &[String]) -> Option<Vec<String>> {
    let diagram = parse(body);
    if diagram.participants.is_empty() {
        return None;
    }
    Some(draw(&diagram).rows())
}

/// Parses the participants and the events.
/// @param body - the lines after the first
fn parse(body: &[String]) -> Diagram {
    let mut diagram = Diagram::default();
    let mut number = None;
    for line in body {
        let lower = line.to_ascii_lowercase();
        let word = lower.split_whitespace().next().unwrap_or("");
        match word {
            "autonumber" => number = Some(1),
            "participant" | "actor" | "create" => add_declared(&mut diagram, line),
            "activate" | "deactivate" | "destroy" | "box" | "title" | "accTitle" => {}
            "loop" | "alt" | "else" | "opt" | "par" | "and" | "critical" | "option" | "break" | "rect" => diagram.events.push(Event::Block(line.clone())),
            "end" => diagram.events.push(Event::Block(String::new())),
            "note" => parse_note(&mut diagram, line),
            _ => parse_message(&mut diagram, line, &mut number),
        }
    }
    diagram
}

/// Adds a participant from a `participant` or `actor` line, with an `as` alias.
/// @param diagram - the diagram
/// @param line - the line
fn add_declared(diagram: &mut Diagram, line: &str) {
    let rest = line.trim_start_matches(|c: char| c.is_alphabetic()).trim();
    let (actor, rest) = if line.to_ascii_lowercase().starts_with("create") {
        let inner = rest.trim_start_matches(|c: char| c.is_alphabetic()).trim();
        (rest.to_ascii_lowercase().starts_with("actor"), inner)
    } else {
        (line.to_ascii_lowercase().starts_with("actor"), rest)
    };
    let (id, label) = match rest.split_once(" as ") {
        Some((id, label)) => (id.trim(), label.trim()),
        None => (rest, rest),
    };
    let index = participant(diagram, id);
    diagram.participants[index].label = label_lines(label).join(" ");
    diagram.participants[index].actor = actor;
}

/// The index of a participant, added when it is first named.
/// @param diagram - the diagram
/// @param id - the participant id
fn participant(diagram: &mut Diagram, id: &str) -> usize {
    let id = id.trim();
    if let Some(index) = diagram.participants.iter().position(|p| p.id == id) {
        return index;
    }
    diagram.participants.push(Participant { id: id.to_string(), label: id.to_string(), actor: false });
    diagram.participants.len() - 1
}

/// Parses a `Note` line.
/// @param diagram - the diagram
/// @param line - the line
fn parse_note(diagram: &mut Diagram, line: &str) {
    let Some(caps) = note_re().captures(line) else { return };
    let place = match caps[1].to_ascii_lowercase().as_str() {
        "left of" => Place::Left,
        "right of" => Place::Right,
        _ => Place::Over,
    };
    let a = participant(diagram, &caps[2]);
    let b = caps.get(3).map_or(a, |m| participant(diagram, m.as_str()));
    diagram.events.push(Event::Note { place, a: a.min(b), b: a.max(b), text: label_lines(&caps[4]).join(" ") });
}

/// Parses a message line, numbering it when `autonumber` is on.
/// @param diagram - the diagram
/// @param line - the line
/// @param number - the next message number, when numbering is on
fn parse_message(diagram: &mut Diagram, line: &str, number: &mut Option<usize>) {
    let Some(caps) = message_re().captures(line) else { return };
    let from = participant(diagram, &caps[1]);
    let to = participant(diagram, caps[3].trim_start_matches(['+', '-']));
    let arrow = &caps[2];
    let head = if arrow.ends_with(">>") {
        Some(('▶', '◀'))
    } else if arrow.ends_with('x') {
        Some(('x', 'x'))
    } else if arrow.ends_with(')') {
        Some(('▷', '◁'))
    } else {
        None
    };
    let mut text = label_lines(&caps[4]).join(" ");
    if let Some(n) = number.as_mut() {
        text = format!("{n}. {text}");
        *n += 1;
    }
    diagram.events.push(Event::Message { from, to, text, head, dashed: arrow.starts_with("--") });
}

/// The columns text takes.
/// @param text - the text
fn width_of(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

/// The box width of each participant.
/// @param diagram - the diagram
fn box_widths(diagram: &Diagram) -> Vec<usize> {
    diagram.participants.iter().map(|p| width_of(&p.label) + 4).collect()
}

/// The column of each lifeline: far enough apart for the boxes, and for every label and note
/// between two lifelines.
/// @param diagram - the diagram
fn lifelines(diagram: &Diagram) -> Vec<usize> {
    let widths = box_widths(diagram);
    let count = widths.len();
    let mut gaps: Vec<usize> = (0..count.saturating_sub(1)).map(|i| widths[i].div_ceil(2) + widths[i + 1].div_ceil(2) + 2).collect();
    let mut margin = widths[0].div_ceil(2);
    for event in &diagram.events {
        match event {
            Event::Message { from, to, text, .. } if from == to => widen(&mut gaps, *from, *from + 1, width_of(text) + 7),
            Event::Message { from, to, text, .. } => widen(&mut gaps, *from.min(to), *from.max(to), width_of(text) + 4),
            Event::Note { place: Place::Right, a, text, .. } => widen(&mut gaps, *a, *a + 1, width_of(text) + 6),
            Event::Note { place: Place::Left, a, text, .. } if *a == 0 => margin = margin.max(width_of(text) + 6),
            Event::Note { place: Place::Left, a, text, .. } => widen(&mut gaps, *a - 1, *a, width_of(text) + 6),
            Event::Note { place: Place::Over, a, b, text } if a == b && *a == 0 => margin = margin.max(width_of(text).div_ceil(2) + 3),
            Event::Note { place: Place::Over, a, b, text } => widen(&mut gaps, *a, *b, width_of(text) + 4),
            Event::Block(_) => {}
        }
    }
    let mut columns = vec![margin];
    for gap in gaps {
        columns.push(columns.last().copied().unwrap_or(0) + gap);
    }
    columns
}

/// Makes the gaps between lifelines `lo` and `hi` add up to at least `need`, growing the last one.
/// @param gaps - the gaps between neighboring lifelines
/// @param lo - the left lifeline
/// @param hi - the right lifeline
/// @param need - the columns needed
fn widen(gaps: &mut [usize], lo: usize, hi: usize, need: usize) {
    if lo >= hi || hi > gaps.len() {
        return;
    }
    let have: usize = gaps[lo..hi].iter().sum();
    if have < need {
        gaps[hi - 1] += need - have;
    }
}

/// Draws the diagram.
/// @param diagram - the diagram
fn draw(diagram: &Diagram) -> Canvas {
    let columns = lifelines(diagram);
    let widths = box_widths(diagram);
    let mut canvas = Canvas::default();
    draw_heads(&mut canvas, diagram, &columns, &widths, 0);
    let mut y = 3;
    let mut blocks = Vec::new();
    for event in &diagram.events {
        y = match event {
            Event::Message { from, to, text, head, dashed } => draw_message(&mut canvas, (columns[*from], columns[*to]), text, *head, *dashed, y),
            Event::Note { place, a, b, text } => draw_note(&mut canvas, *place, (columns[*a], columns[*b]), text, y),
            Event::Block(text) => {
                blocks.push((y, text.clone()));
                y + 1
            }
        };
    }
    for &x in &columns {
        canvas.vline(x, 3, y, Stroke::Dotted);
    }
    draw_heads(&mut canvas, diagram, &columns, &widths, y + 1);
    let width = canvas.width();
    for (row, text) in blocks {
        draw_block(&mut canvas, &columns, row, &text, width);
    }
    canvas
}

/// Draws a row of participant boxes.
/// @param canvas - the canvas
/// @param diagram - the diagram
/// @param columns - the lifeline columns
/// @param widths - the box widths
/// @param y - the top row
fn draw_heads(canvas: &mut Canvas, diagram: &Diagram, columns: &[usize], widths: &[usize], y: usize) {
    for (index, p) in diagram.participants.iter().enumerate() {
        let corners = if p.actor { ('╭', '╮', '╰', '╯') } else { ('┌', '┐', '└', '┘') };
        let left = columns[index] - widths[index] / 2;
        draw_box(canvas, (left, y), (widths[index], 3), corners, std::slice::from_ref(&p.label));
    }
}

/// Draws a message: its text centered over an arrow between two lifelines, or a loop on the right
/// for a message to itself. Returns the next free row.
/// @param canvas - the canvas
/// @param ends - the sender's and the receiver's columns
/// @param text - the label
/// @param head - the head pointing right and left, if any
/// @param dashed - true for a dashed line
/// @param y - the first row
fn draw_message(canvas: &mut Canvas, ends: (usize, usize), text: &str, head: Option<(char, char)>, dashed: bool, y: usize) -> usize {
    let stroke = if dashed { Stroke::Dotted } else { Stroke::Solid };
    let (from, to) = ends;
    if from == to {
        canvas.hline(y, from, from + 3, stroke);
        canvas.vline(from + 3, y, y + 1, stroke);
        canvas.hline(y + 1, from + 1, from + 3, stroke);
        if let Some((_, left)) = head {
            canvas.put(from + 1, y + 1, left);
        }
        canvas.text(from + 5, y, text);
        return y + 2;
    }
    let (lo, hi) = (from.min(to), from.max(to));
    let start = lo + (hi - lo).saturating_sub(width_of(text)) / 2;
    canvas.text(start.max(lo + 1), y, text);
    if to > from {
        canvas.hline(y + 1, from, to - 1, stroke);
        if let Some((right, _)) = head {
            canvas.put(to - 1, y + 1, right);
        }
    } else {
        canvas.hline(y + 1, to + 1, from, stroke);
        if let Some((_, left)) = head {
            canvas.put(to + 1, y + 1, left);
        }
    }
    y + 2
}

/// Draws a note box beside or over lifelines. Returns the next free row.
/// @param canvas - the canvas
/// @param place - where the note goes
/// @param ends - the columns of the first and last lifeline it names
/// @param text - the note text
/// @param y - the top row
fn draw_note(canvas: &mut Canvas, place: Place, ends: (usize, usize), text: &str, y: usize) -> usize {
    let width = width_of(text) + 4;
    let left = match place {
        Place::Right => ends.0 + 2,
        Place::Left => ends.0.saturating_sub(width + 1),
        Place::Over => ((ends.0 + ends.1) / 2).saturating_sub(width / 2),
    };
    let width = if place == Place::Over && ends.1 > ends.0 { width.max(ends.1 - ends.0 + 4) } else { width };
    let left = if place == Place::Over && ends.1 > ends.0 { ends.0.saturating_sub(2) } else { left };
    draw_box(canvas, (left, y), (width, 3), ('┌', '┐', '└', '┘'), &[text.to_string()]);
    y + 3
}

/// Draws a block row: a dotted line across, broken by the lifelines, with the block's text.
/// @param canvas - the canvas
/// @param columns - the lifeline columns
/// @param y - the row
/// @param text - the block text, empty for `end`
/// @param width - the diagram width
fn draw_block(canvas: &mut Canvas, columns: &[usize], y: usize, text: &str, width: usize) {
    for x in 0..width {
        if !columns.contains(&x) {
            canvas.put(x, y, '┄');
        }
    }
    if !text.is_empty() {
        canvas.text(1, y, &format!(" {} ", text.trim()));
    }
}
