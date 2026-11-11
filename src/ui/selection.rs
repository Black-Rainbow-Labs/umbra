//! Text selection with the mouse. A selection is kept in transcript positions (a block, a row in
//! it, a column), so it stays on the same text while the transcript scrolls. The selected cells are
//! drawn in reverse video, and the text is copied with the rows of one wrapped line joined.

use super::block::LayoutCtx;
use super::transcript::Transcript;
use crate::utils::ansi;

/// A cell of the transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    /// The block id.
    pub id: u64,
    /// The row inside the block.
    pub row: usize,
    pub col: usize,
}

/// A selection from where the drag started to where the pointer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Pos,
    pub head: Pos,
}

/// The order of a position in the transcript, or None when its block is gone.
/// @param transcript - the transcript
/// @param pos - the position
fn key(transcript: &Transcript, pos: Pos) -> Option<(usize, usize, usize)> {
    Some((transcript.index_of(pos.id)?, pos.row, pos.col))
}

/// The selection's start and end, in transcript order, as sort keys.
/// @param transcript - the transcript
/// @param selection - the selection
fn bounds(transcript: &Transcript, selection: &Selection) -> Option<((usize, usize, usize), (usize, usize, usize))> {
    let (a, b) = (key(transcript, selection.anchor)?, key(transcript, selection.head)?);
    Some(if a <= b { (a, b) } else { (b, a) })
}

/// The columns selected on one row, as a start and an end after the last, or None.
/// @param transcript - the transcript
/// @param selection - the selection
/// @param id - the row's block id
/// @param row - the row inside the block
/// @param width - the screen width
pub fn columns_on(transcript: &Transcript, selection: &Selection, id: u64, row: usize, width: usize) -> Option<(usize, usize)> {
    let (start, end) = bounds(transcript, selection)?;
    let here = (transcript.index_of(id)?, row);
    if here < (start.0, start.1) || here > (end.0, end.1) {
        return None;
    }
    let from = if here == (start.0, start.1) { start.2 } else { 0 };
    let to = if here == (end.0, end.1) { end.2 + 1 } else { width };
    (from < to).then_some((from, to.min(width)))
}

/// Draws the selected columns of a row in reverse video.
/// @param text - the row
/// @param columns - the selected columns
pub fn highlight(text: &str, columns: (usize, usize)) -> String {
    ansi::overlay_style(text, columns.0, columns.1, "\x1b[7m")
}

/// The selected text: each row's selected cells without the row's decoration, trailing spaces
/// removed, wrapped rows of one line joined, and other rows joined with line breaks.
/// @param transcript - the transcript
/// @param selection - the selection
/// @param width - the screen width
/// @param ctx - the frame
pub fn selected_text(transcript: &mut Transcript, selection: &Selection, width: usize, ctx: &LayoutCtx) -> String {
    let Some((start, end)) = bounds(transcript, selection) else { return String::new() };
    let mut out = String::new();
    let mut first = true;
    for index in start.0..=end.0 {
        let rows = transcript.rows_at(index, width, ctx);
        let Some(id) = transcript.entries().nth(index).map(|e| e.id) else { continue };
        for (number, row) in rows.iter().enumerate() {
            let Some((from, to)) = columns_on(transcript, selection, id, number, width) else { continue };
            let piece = ansi::plain_columns(&row.text, from.max(row.gutter), to);
            if !first {
                out.push_str(if row.cont { "" } else { "\n" });
            }
            first = false;
            out.push_str(piece.trim_end());
        }
    }
    out
}

/// The selection of the word around a position, or of its whole row.
/// @param transcript - the transcript
/// @param pos - the clicked cell
/// @param line - true for the whole row
/// @param width - the screen width
/// @param ctx - the frame
pub fn around(transcript: &mut Transcript, pos: Pos, line: bool, width: usize, ctx: &LayoutCtx) -> Option<Selection> {
    let index = transcript.index_of(pos.id)?;
    let rows = transcript.rows_at(index, width, ctx);
    let row = rows.get(pos.row)?;
    let cells: Vec<char> = ansi::plain(&row.text).chars().collect();
    if line {
        let end = cells.len().saturating_sub(1);
        return Some(Selection { anchor: Pos { col: row.gutter, ..pos }, head: Pos { col: end, ..pos } });
    }
    let is_word = |c: char| c.is_alphanumeric() || "_-./:".contains(c);
    if !cells.get(pos.col).is_some_and(|c| is_word(*c)) {
        return None;
    }
    let mut start = pos.col;
    while start > 0 && is_word(cells[start - 1]) {
        start -= 1;
    }
    let mut end = pos.col;
    while end + 1 < cells.len() && is_word(cells[end + 1]) {
        end += 1;
    }
    Some(Selection { anchor: Pos { col: start, ..pos }, head: Pos { col: end, ..pos } })
}

#[cfg(test)]
mod tests {
    use super::super::block::Block;
    use super::*;

    fn ctx() -> LayoutCtx {
        LayoutCtx { color: false, now: std::time::Instant::now(), spinner: '⠋' }
    }

    #[test]
    fn a_selection_across_a_wrapped_line_copies_it_joined() {
        let mut transcript = Transcript::default();
        let a = transcript.push(Block::Text("hello wonderful world".into()));
        let b = transcript.push(Block::Text("second line".into()));
        let selection = Selection { anchor: Pos { id: a, row: 0, col: 2 }, head: Pos { id: b, row: 0, col: 5 } };
        assert_eq!(selected_text(&mut transcript, &selection, 10, &ctx()), "llo wonderful world\nsecond");
        assert_eq!(columns_on(&transcript, &selection, a, 1, 10), Some((0, 10)));
        let backwards = Selection { anchor: selection.head, head: selection.anchor };
        assert_eq!(selected_text(&mut transcript, &backwards, 10, &ctx()), "llo wonderful world\nsecond");
    }

    #[test]
    fn a_double_click_selects_a_word() {
        let mut transcript = Transcript::default();
        let a = transcript.push(Block::Text("run cargo test now".into()));
        let selection = around(&mut transcript, Pos { id: a, row: 0, col: 6 }, false, 40, &ctx()).unwrap();
        assert_eq!(selected_text(&mut transcript, &selection, 40, &ctx()), "cargo");
        let whole = around(&mut transcript, Pos { id: a, row: 0, col: 6 }, true, 40, &ctx()).unwrap();
        assert_eq!(selected_text(&mut transcript, &whole, 40, &ctx()), "run cargo test now");
    }
}
