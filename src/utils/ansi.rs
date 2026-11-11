//! Text with ANSI codes, measured and cut by screen columns: the plain text of a row, part of a
//! row, a style laid over some columns, and padding to a width. The fullscreen view uses these to
//! draw a selection, a focus marker and boxes drawn over the transcript.

use super::screen::char_width;

/// One piece of a row: an escape sequence, which takes no columns, or a character and its width.
enum Piece {
    Escape(String),
    Char(char, usize),
}

/// Splits a row into escape sequences and characters.
/// @param row - the row, with color codes
fn pieces(row: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut chars = row.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            let mut sequence = String::from("\x1b");
            copy_escape(&mut chars, &mut sequence);
            out.push(Piece::Escape(sequence));
        } else if (c as u32) >= 0x20 && c != '\x7f' {
            out.push(Piece::Char(c, char_width(c)));
        }
    }
    out
}

/// Copies one escape sequence, after its ESC, from `chars` to `out`: a CSI sequence up to its
/// final byte, an OSC sequence up to BEL or ESC backslash, or one more character.
/// @param chars - the characters after the ESC
/// @param out - where the sequence's characters go
pub fn copy_escape(chars: &mut std::iter::Peekable<std::str::Chars>, out: &mut String) {
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

/// Returns true for a color or style code (SGR), the kind that carries over between columns.
/// @param sequence - one escape sequence
fn is_sgr(sequence: &str) -> bool {
    sequence.starts_with("\x1b[") && sequence.ends_with('m')
}

/// The text of a row without escape codes.
/// @param row - the row, with color codes
pub fn plain(row: &str) -> String {
    pieces(row).into_iter().filter_map(|p| match p { Piece::Char(c, _) => Some(c), Piece::Escape(_) => None }).collect()
}

/// The columns from `from` up to `to` of a row, with the colors that were on at `from`, ending
/// with a reset. A wide character cut in half is replaced by a space.
/// @param row - the row, with color codes
/// @param from - the first column
/// @param to - the column after the last
pub fn slice(row: &str, from: usize, to: usize) -> String {
    let mut out = String::new();
    let mut active = String::new();
    let mut column = 0;
    let mut started = false;
    for piece in pieces(row) {
        match piece {
            Piece::Escape(sequence) if is_sgr(&sequence) => {
                if sequence == "\x1b[0m" || sequence == "\x1b[m" { active.clear() } else { active.push_str(&sequence) }
                if started && column < to {
                    out.push_str(&sequence);
                }
            }
            Piece::Escape(sequence) => {
                if started && column < to {
                    out.push_str(&sequence);
                }
            }
            Piece::Char(c, width) => {
                if column >= from && column + width <= to {
                    if !started {
                        out.push_str(&active);
                        started = true;
                    }
                    out.push(c);
                } else if column < to && column + width > from {
                    if !started {
                        out.push_str(&active);
                        started = true;
                    }
                    out.push_str(&" ".repeat((column + width).min(to) - column.max(from)));
                }
                column += width;
            }
        }
        if column >= to && started {
            break;
        }
    }
    let used = super::screen::visible_width(&out);
    let wanted = to.saturating_sub(from).min(super::screen::visible_width(row).saturating_sub(from));
    out.push_str(&" ".repeat(wanted.saturating_sub(used)));
    out.push_str("\x1b[0m");
    out
}

/// Pads a row with spaces to `width` columns, or cuts it there.
/// @param row - the row, with color codes
/// @param width - the columns wanted
pub fn fit(row: &str, width: usize) -> String {
    let used = super::screen::visible_width(row);
    if used > width {
        return slice(row, 0, width);
    }
    format!("{row}\x1b[0m{}", " ".repeat(width - used))
}

/// Lays a style over the columns from `from` up to `to`, such as reverse video for a selection.
/// The style is turned on again after any reset inside the range, so the row's own colors do not
/// end it.
/// @param row - the row, with color codes
/// @param from - the first column
/// @param to - the column after the last
/// @param on - the code that turns the style on, such as `\x1b[7m`
pub fn overlay_style(row: &str, from: usize, to: usize, on: &str) -> String {
    if from >= to {
        return row.to_string();
    }
    let total = super::screen::visible_width(row);
    let mut out = String::new();
    let mut active = String::new();
    let mut column = 0;
    let mut inside = false;
    for piece in pieces(row) {
        match piece {
            Piece::Escape(sequence) => {
                if is_sgr(&sequence) {
                    if sequence == "\x1b[0m" || sequence == "\x1b[m" { active.clear() } else { active.push_str(&sequence) }
                }
                out.push_str(&sequence);
                if inside && is_sgr(&sequence) {
                    out.push_str(on);
                }
            }
            Piece::Char(c, width) => {
                if column == from {
                    out.push_str(on);
                    inside = true;
                }
                if column == to && inside {
                    out.push_str("\x1b[0m");
                    out.push_str(&active);
                    inside = false;
                }
                out.push(c);
                column += width;
            }
        }
    }
    if from >= total {
        out.push_str(on);
        out.push_str(&" ".repeat(to - from));
        inside = true;
    } else if inside && to > total {
        out.push_str(&" ".repeat(to - total));
    }
    if inside {
        out.push_str("\x1b[0m");
    }
    out
}

/// Replaces the character at one column with another character of width 1, keeping the colors.
/// Does nothing when the column is past the end or holds a wide character.
/// @param row - the row, with color codes
/// @param col - the column
/// @param replacement - the new character
pub fn replace_column(row: &str, col: usize, replacement: &str) -> String {
    let mut out = String::new();
    let mut column = 0;
    let mut done = false;
    for piece in pieces(row) {
        match piece {
            Piece::Escape(sequence) => out.push_str(&sequence),
            Piece::Char(c, width) => {
                if column == col && width == 1 && !done {
                    out.push_str(replacement);
                    done = true;
                } else {
                    out.push(c);
                }
                column += width;
            }
        }
    }
    out
}

/// The text of the columns from `from` up to `to`, without escape codes.
/// @param row - the row, with color codes
/// @param from - the first column
/// @param to - the column after the last
pub fn plain_columns(row: &str, from: usize, to: usize) -> String {
    let mut out = String::new();
    let mut column = 0;
    for piece in pieces(row) {
        if let Piece::Char(c, width) = piece {
            if column >= from && column < to {
                out.push(c);
            }
            column += width;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_by_columns_and_keeps_the_colors() {
        let row = "ab\x1b[36mcdef\x1b[0mgh";
        assert_eq!(plain(&slice(row, 1, 4)), "bcd");
        assert!(slice(row, 3, 5).starts_with("\x1b[36md"), "{:?}", slice(row, 3, 5));
        assert_eq!(plain(&slice("漢字", 1, 4)), " 字");
        assert_eq!(plain(&fit("abc", 5)), "abc  ");
        assert_eq!(plain(&fit("abcdef", 3)), "abc");
    }

    #[test]
    fn a_style_covers_the_range_even_across_a_reset() {
        let row = "a\x1b[31mbc\x1b[0md";
        let styled = overlay_style(row, 1, 3, "\x1b[7m");
        assert_eq!(plain(&styled), "abcd");
        assert!(styled.contains("\x1b[7mb") || styled.contains("\x1b[7m\x1b[31mb"), "{styled:?}");
        assert_eq!(plain(&overlay_style("ab", 1, 4, "\x1b[7m")), "ab  ");
        assert_eq!(plain(&replace_column("  x", 1, "›")), " ›x");
        assert_eq!(plain_columns("\x1b[1mhello\x1b[0m world", 2, 7), "llo w");
    }
}
