//! A grid of character cells that diagrams are drawn on. Lines are kept as connection bits per
//! cell, so where two lines meet the cell gets the right corner, tee or crossing character. Text
//! and box borders are fixed characters that lines do not change.

use crate::utils::screen::char_width;

/// A line goes up out of the cell.
pub const UP: u8 = 1;
/// A line goes down out of the cell.
pub const DOWN: u8 = 2;
/// A line goes left out of the cell.
pub const LEFT: u8 = 4;
/// A line goes right out of the cell.
pub const RIGHT: u8 = 8;

/// How a straight line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stroke {
    #[default]
    Solid,
    Dotted,
    Thick,
}

/// One cell: a fixed character, or the lines through it.
#[derive(Debug, Clone, Copy, Default)]
struct Cell {
    fixed: Option<char>,
    bits: u8,
    stroke: Stroke,
}

/// The grid. It grows to the right and down as things are drawn.
#[derive(Debug, Default)]
pub struct Canvas {
    cells: Vec<Vec<Cell>>,
}

impl Canvas {
    /// The cell at a position, growing the grid to reach it.
    /// @param x - the column
    /// @param y - the row
    fn cell(&mut self, x: usize, y: usize) -> &mut Cell {
        if self.cells.len() <= y {
            self.cells.resize(y + 1, Vec::new());
        }
        let row = &mut self.cells[y];
        if row.len() <= x {
            row.resize(x + 1, Cell::default());
        }
        &mut row[x]
    }

    /// Puts a fixed character in a cell.
    /// @param x - the column
    /// @param y - the row
    /// @param c - the character
    pub fn put(&mut self, x: usize, y: usize, c: char) {
        self.cell(x, y).fixed = Some(c);
    }

    /// Writes text from a column, one cell per character. A wide character takes two cells.
    /// @param x - the first column
    /// @param y - the row
    /// @param text - the text
    pub fn text(&mut self, x: usize, y: usize, text: &str) {
        let mut col = x;
        for c in text.chars() {
            let w = char_width(c);
            if w == 0 {
                continue;
            }
            self.put(col, y, c);
            if w == 2 {
                self.put(col + 1, y, '\0');
            }
            col += w;
        }
    }

    /// Draws a horizontal line between two columns, both ends included.
    /// @param y - the row
    /// @param a - one end
    /// @param b - the other end
    /// @param stroke - how the line is drawn
    pub fn hline(&mut self, y: usize, a: usize, b: usize, stroke: Stroke) {
        let (from, to) = (a.min(b), a.max(b));
        for x in from..=to {
            let cell = self.cell(x, y);
            if x > from {
                cell.bits |= LEFT;
            }
            if x < to {
                cell.bits |= RIGHT;
            }
            if from == to {
                cell.bits |= LEFT | RIGHT;
            }
            cell.stroke = stroke;
        }
    }

    /// Draws a vertical line between two rows, both ends included.
    /// @param x - the column
    /// @param a - one end
    /// @param b - the other end
    /// @param stroke - how the line is drawn
    pub fn vline(&mut self, x: usize, a: usize, b: usize, stroke: Stroke) {
        let (from, to) = (a.min(b), a.max(b));
        for y in from..=to {
            let cell = self.cell(x, y);
            if y > from {
                cell.bits |= UP;
            }
            if y < to {
                cell.bits |= DOWN;
            }
            if from == to {
                cell.bits |= UP | DOWN;
            }
            cell.stroke = stroke;
        }
    }

    /// The canvas's width: the longest row.
    pub fn width(&self) -> usize {
        self.cells.iter().map(|r| r.len()).max().unwrap_or(0)
    }

    /// The rows as text, with trailing spaces removed.
    pub fn rows(&self) -> Vec<String> {
        self.cells.iter().map(|row| {
            let line: String = row.iter().filter_map(cell_char).collect();
            line.trim_end().to_string()
        }).collect()
    }
}

/// The character a cell shows, or None for the second half of a wide character.
/// @param cell - the cell
fn cell_char(cell: &Cell) -> Option<char> {
    match cell.fixed {
        Some('\0') => None,
        Some(c) => Some(c),
        None => Some(line_char(cell.bits, cell.stroke)),
    }
}

/// The box drawing character for the lines through a cell. Corners are rounded.
/// @param bits - the directions the lines go out of the cell
/// @param stroke - how a straight line is drawn
pub fn line_char(bits: u8, stroke: Stroke) -> char {
    let vertical = bits & (LEFT | RIGHT) == 0;
    let horizontal = bits & (UP | DOWN) == 0;
    if bits != 0 && vertical {
        return match stroke { Stroke::Solid => '│', Stroke::Dotted => '┆', Stroke::Thick => '┃' };
    }
    if bits != 0 && horizontal {
        return match stroke { Stroke::Solid => '─', Stroke::Dotted => '┄', Stroke::Thick => '━' };
    }
    match bits {
        b if b == DOWN | RIGHT => '╭',
        b if b == DOWN | LEFT => '╮',
        b if b == UP | RIGHT => '╰',
        b if b == UP | LEFT => '╯',
        b if b == UP | DOWN | RIGHT => '├',
        b if b == UP | DOWN | LEFT => '┤',
        b if b == LEFT | RIGHT | DOWN => '┬',
        b if b == LEFT | RIGHT | UP => '┴',
        b if b == UP | DOWN | LEFT | RIGHT => '┼',
        _ => ' ',
    }
}

/// The corner characters of a box, top left, top right, bottom left, bottom right.
pub type Corners = (char, char, char, char);

/// Draws a box with its label lines centered.
/// @param canvas - the canvas
/// @param at - the top left corner as (column, row)
/// @param size - the width and height
/// @param corners - the corner characters
/// @param label - the label lines
pub fn draw_box(canvas: &mut Canvas, at: (usize, usize), size: (usize, usize), corners: Corners, label: &[String]) {
    let ((x, y), (w, h)) = (at, size);
    if w < 2 || h < 2 {
        return;
    }
    canvas.put(x, y, corners.0);
    canvas.put(x + w - 1, y, corners.1);
    canvas.put(x, y + h - 1, corners.2);
    canvas.put(x + w - 1, y + h - 1, corners.3);
    for col in x + 1..x + w - 1 {
        canvas.put(col, y, '─');
        canvas.put(col, y + h - 1, '─');
    }
    for row in y + 1..y + h - 1 {
        canvas.put(x, row, '│');
        canvas.put(x + w - 1, row, '│');
        for col in x + 1..x + w - 1 {
            canvas.put(col, row, ' ');
        }
    }
    let top = y + 1 + (h - 2).saturating_sub(label.len()) / 2;
    for (index, line) in label.iter().enumerate() {
        let lw: usize = line.chars().map(char_width).sum();
        canvas.text(x + (w - lw) / 2, top + index, line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossing_lines_get_joined_characters() {
        let mut canvas = Canvas::default();
        canvas.vline(2, 0, 2, Stroke::Solid);
        canvas.hline(1, 0, 4, Stroke::Solid);
        canvas.hline(2, 2, 4, Stroke::Solid);
        assert_eq!(canvas.rows(), vec!["  │", "──┼──", "  ╰──"]);
    }
}
