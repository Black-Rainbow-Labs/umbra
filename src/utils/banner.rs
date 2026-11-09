//! The banner a run prints when it starts in a terminal: the UMBRA logo, the version, the model,
//! the folder, the mode and what was loaded. This file only builds the lines; the run prints them.

/// What the banner shows.
#[derive(Debug, Clone, Default)]
pub struct BannerInfo {
    pub version: String,
    /// The `provider/model` string.
    pub model: String,
    /// The working folder, as it should be shown.
    pub folder: String,
    /// confirm, yolo or human.
    pub mode: String,
    pub instruction_files: usize,
    pub skills: usize,
    pub mcp_servers: usize,
}

/// The logo, three rows of half block characters, one entry per letter.
const LOGO: [[&str; 5]; 3] = [["█  █", "█▄ ▄█", "█▀▀▄", "█▀▀▄", "▄▀▀▄"], ["█  █", "█ ▀ █", "█▀▀▄", "█▀█ ", "█▄▄█"], ["▀▄▄▀", "█   █", "█▄▄▀", "█  █", "█  █"]];

/// The 256 color codes of the letters, teal to violet.
const LETTER_COLORS: [&str; 5] = ["38;5;43", "38;5;37", "38;5;74", "38;5;69", "38;5;99"];

/// The color of the box lines.
const BOX: &str = "38;5;239";
/// The color of labels and counts.
const DIM: &str = "90";

/// Below this width the logo and the box are left out.
const NARROW: usize = 60;

/// The columns the logo takes: the letters and the spaces between them.
const LOGO_WIDTH: usize = 25;

/// Left margin inside the box, and the gap between the logo and the text.
const MARGIN: usize = 3;
const GAP: usize = 4;

/// Builds the banner's lines for a terminal `width` columns wide. Each line fits in the width.
/// @param info - what to show
/// @param width - the terminal width in columns
/// @param color - true to add ANSI colors
pub fn render_banner(info: &BannerInfo, width: usize, color: bool) -> Vec<String> {
    let paint = |code: &str, text: &str| if color && !code.is_empty() { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() };
    let rows = text_rows(info);
    if width < NARROW {
        return narrow_lines(&rows, width, &paint);
    }
    let room = width.saturating_sub(3 + MARGIN + LOGO_WIDTH + GAP + MARGIN);
    let text_width = rows.iter().map(row_width).max().unwrap_or(0).min(room);
    let inner = MARGIN + LOGO_WIDTH + GAP + text_width + MARGIN;
    let mut lines = vec![paint(BOX, &format!("╭{}╮", "─".repeat(inner))), boxed(&paint, String::new(), 0, inner)];
    for (index, row) in rows.iter().enumerate() {
        let logo = logo_row(index, &paint);
        let (text, used) = paint_row(row, text_width, &paint);
        let body = format!("{}{logo}{}{text}", " ".repeat(MARGIN), " ".repeat(GAP));
        lines.push(boxed(&paint, body, MARGIN + LOGO_WIDTH + GAP + used, inner));
    }
    lines.push(boxed(&paint, String::new(), 0, inner));
    lines.push(paint(BOX, &format!("╰{}╯", "─".repeat(inner))));
    lines.push(paint(DIM, &clip_end(HINT, width.saturating_sub(1))));
    lines
}

/// The line under the banner that says how to steer a run.
const HINT: &str = "  Ctrl+C interrupts · /compact shrinks the history · lines typed while it works go to the next step";

/// One text row: pieces of (color, text). The first piece is the label.
type Row = Vec<(&'static str, String)>;

/// The four text rows beside the logo: name and version, model, folder, and mode with counts.
/// @param info - what to show
fn text_rows(info: &BannerInfo) -> Vec<Row> {
    let mut counts = vec![format!("{} instruction {}", info.instruction_files, plural(info.instruction_files, "file", "files")), format!("{} {}", info.skills, plural(info.skills, "skill", "skills"))];
    if info.mcp_servers > 0 {
        counts.push(format!("{} MCP {}", info.mcp_servers, plural(info.mcp_servers, "server", "servers")));
    }
    vec![
        vec![("1", "umbra".to_string()), (DIM, format!(" v{}", info.version))],
        vec![(DIM, "model   ".to_string()), ("", info.model.clone())],
        vec![(DIM, "folder  ".to_string()), ("", info.folder.clone())],
        vec![(DIM, "mode    ".to_string()), ("", info.mode.clone()), (DIM, format!(" · {}", counts.join(" · ")))],
    ]
}

/// Picks the singular or plural word.
/// @param count - how many
/// @param one - the singular
/// @param many - the plural
fn plural(count: usize, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}

/// The columns a row takes.
/// @param row - the row's pieces
fn row_width(row: &Row) -> usize {
    row.iter().map(|(_, t)| t.chars().count()).sum()
}

/// Paints a row, cutting it to `limit` columns with an ellipsis. Returns the text and its width.
/// @param row - the row's pieces
/// @param limit - the most columns it may take
/// @param paint - adds a color code to text
fn paint_row(row: &Row, limit: usize, paint: &dyn Fn(&str, &str) -> String) -> (String, usize) {
    let mut out = String::new();
    let mut used = 0;
    for (code, text) in row {
        let left = limit.saturating_sub(used);
        let piece = clip_end(text, left);
        used += piece.chars().count();
        out.push_str(&paint(code, &piece));
    }
    (out, used)
}

/// The logo's row `index` with each letter in its color, or blanks below the logo.
/// @param index - the text row beside it
/// @param paint - adds a color code to text
fn logo_row(index: usize, paint: &dyn Fn(&str, &str) -> String) -> String {
    let Some(letters) = LOGO.get(index) else { return " ".repeat(LOGO_WIDTH) };
    letters.iter().zip(LETTER_COLORS).map(|(letter, code)| paint(code, letter)).collect::<Vec<_>>().join(" ")
}

/// Puts a line of the box together: the side lines, the body and the padding up to `inner`.
/// @param paint - adds a color code to text
/// @param body - the painted text
/// @param used - the columns the body takes
/// @param inner - the columns between the side lines
fn boxed(paint: &dyn Fn(&str, &str) -> String, body: String, used: usize, inner: usize) -> String {
    let side = paint(BOX, "│");
    format!("{side}{body}{}{side}", " ".repeat(inner.saturating_sub(used)))
}

/// The banner for a narrow terminal: the text rows alone.
/// @param rows - the text rows
/// @param width - the terminal width
/// @param paint - adds a color code to text
fn narrow_lines(rows: &[Row], width: usize, paint: &dyn Fn(&str, &str) -> String) -> Vec<String> {
    rows.iter().map(|row| paint_row(row, width.saturating_sub(1), paint).0).collect()
}

/// Cuts text to `limit` characters, ending with an ellipsis when it was cut.
/// @param text - the text
/// @param limit - the most characters to keep
fn clip_end(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    if limit == 0 {
        return String::new();
    }
    let mut kept: String = text.chars().take(limit - 1).collect();
    kept.push('…');
    kept
}

/// Shows a folder with the home folder as `~` and forward slashes.
/// @param folder - the folder
/// @param home - the home folder, if known
pub fn display_folder(folder: &str, home: Option<&str>) -> String {
    let folder = folder.replace('\\', "/");
    let Some(home) = home.map(|h| h.replace('\\', "/")).filter(|h| !h.is_empty()) else { return folder };
    let (Some(head), Some(rest)) = (folder.get(..home.len()), folder.get(home.len()..)) else { return folder };
    if head.eq_ignore_ascii_case(&home) && (rest.is_empty() || rest.starts_with('/')) { format!("~{rest}") } else { folder }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A banner with typical values.
    fn info() -> BannerInfo {
        BannerInfo { version: "0.4.1".into(), model: "anthropic/claude-sonnet-5-5".into(), folder: "~/dev/colligo".into(), mode: "confirm".into(), instruction_files: 3, skills: 12, mcp_servers: 2 }
    }

    #[test]
    fn every_line_fits_and_the_box_is_square() {
        for width in [60, 80, 100, 200] {
            let lines = render_banner(&info(), width, false);
            let box_lines = &lines[..lines.len() - 1];
            let first = box_lines[0].chars().count();
            assert!(first < width, "width {width}: {first}");
            for line in box_lines {
                assert_eq!(line.chars().count(), first, "width {width}: {line}");
            }
            assert!(lines.last().unwrap().chars().count() < width);
        }
    }

    #[test]
    fn shows_the_version_model_folder_and_counts() {
        let text = render_banner(&info(), 120, false).join("\n");
        for wanted in ["umbra v0.4.1", "anthropic/claude-sonnet-5-5", "~/dev/colligo", "confirm · 3 instruction files · 12 skills · 2 MCP servers", "█▄ ▄█"] {
            assert!(text.contains(wanted), "{wanted} in\n{text}");
        }
    }

    #[test]
    fn a_narrow_terminal_gets_no_logo() {
        let lines = render_banner(&info(), 40, false);
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().all(|l| l.chars().count() < 40 && !l.contains('█')), "{lines:#?}");
    }

    #[test]
    fn a_long_model_name_is_cut_to_fit() {
        let mut long = info();
        long.model = "x".repeat(300);
        let lines = render_banner(&long, 90, false);
        assert!(lines.iter().all(|l| l.chars().count() < 90));
        assert!(lines.iter().any(|l| l.contains('…')));
    }

    #[test]
    fn color_codes_do_not_change_the_text() {
        let plain = render_banner(&info(), 100, false).join("\n");
        let colored = render_banner(&info(), 100, true).join("\n");
        let stripped = regex::Regex::new("\x1b\\[[0-9;]*m").unwrap().replace_all(&colored, "");
        assert_eq!(plain, stripped);
    }

    #[test]
    fn the_home_folder_is_shown_as_a_tilde() {
        assert_eq!(display_folder("C:\\Users\\jason\\dev\\x", Some("C:\\Users\\jason")), "~/dev/x");
        assert_eq!(display_folder("/home/jasonb/x", Some("/home/jason")), "/home/jasonb/x");
        assert_eq!(display_folder("/srv/x", None), "/srv/x");
    }
}
