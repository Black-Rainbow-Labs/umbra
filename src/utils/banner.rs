//! The banner a run prints when it starts in a terminal: the umbra logo with "by Black Rainbow Labs"
//! under it, the version, the model, the folder, the mode and what was loaded. This file only builds
//! the lines; the run prints them. The banner can be built one step at a time, so the run can show the
//! logo's colored stripes appearing one after another.

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
    pub skills: usize,
    /// The mcp row after its label, as colored pieces, such as `3 running` and a red `1 failed`.
    pub mcp: Vec<(&'static str, String)>,
    /// True in the fullscreen view, where the model row opens the model picker: it ends with `▾`.
    pub model_menu: bool,
}

/// A 24 bit color.
type Rgb = (u8, u8, u8);

/// The logo as pixels: a color, or nothing where the terminal's own background shows.
type Pixels = Vec<Vec<Option<Rgb>>>;

/// The logo's letters, u m b r a. X is a filled pixel. Strokes are 3 pixels thick, the holes are 1
/// pixel wide, the x height is 9 pixels and the b rises 3 pixels above it. The m has a notch between
/// its humps, the r has a drop at the end of its arm and the a has a notch on its left side.
const LETTERS: [&[&str]; 5] = [
    &["XXX.XXX", "XXX.XXX", "XXX.XXX", "XXX.XXX", "XXX.XXX", "XXX.XXX", "XXXXXXX", "XXXXXXX", "XXXXXXX"],
    &["XXXXX.XXXXX", "XXXXXXXXXXX", "XXXXXXXXXXX", "XXX.XXX.XXX", "XXX.XXX.XXX", "XXX.XXX.XXX", "XXX.XXX.XXX", "XXX.XXX.XXX", "XXX.XXX.XXX"],
    &["XXX....", "XXX....", "XXX....", "XXXXXXX", "XXXXXXX", "XXXXXXX", "XXX.XXX", "XXX.XXX", "XXX.XXX", "XXXXXXX", "XXXXXXX", "XXXXXXX"],
    &["XXXXXX", "XXXXXX", "XXXXXX", "XXX.XX", "XXX...", "XXX...", "XXX...", "XXX...", "XXX..."],
    &["XXXXXXX", "XXXXXXX", "....XXX", "XXXXXXX", "XXXXXXX", "XXX.XXX", "XXX.XXX", "XXXXXXX", "XXXXXXX"],
];

/// The stripes behind the letters, front to back: green, yellow, orange, red, blue.
const STRIPES: [Rgb; 5] = [(0x2b, 0xa6, 0x40), (0xf7, 0xc2, 0x1a), (0xff, 0x7f, 0x11), (0xe8, 0x26, 0x1c), (0x1d, 0x9b, 0xe0)];

/// The letters' cream fill.
const FILL: Rgb = (0xf0, 0xe3, 0xcd);

/// The black outline around the letters and around each stripe.
const INK: Rgb = (0x05, 0x05, 0x05);

/// How far each stripe sits below and to the left of the one in front of it, in pixels: one pixel of
/// color and one of outline show.
const STRIPE_STEP: usize = 2;

/// The colors "Black Rainbow Labs" runs through, one letter at a time.
const TAGLINE_STOPS: [Rgb; 6] = [(0xe8, 0x26, 0x1c), (0xff, 0x7f, 0x11), (0xf7, 0xc2, 0x1a), (0x2b, 0xa6, 0x40), (0x1d, 0x9b, 0xe0), (0x7d, 0x6b, 0xff)];

/// The company the tagline names.
const TAGLINE_NAME: &str = "Black Rainbow Labs";

/// How long each step of the stripe reveal stays on screen, in milliseconds.
pub const STEP_MS: u64 = 280;

/// The steps of the reveal after the first: one for each stripe.
pub const REVEAL_STEPS: usize = STRIPES.len();

/// The color of the box lines.
const BOX: &str = "38;5;239";
/// The color of labels and counts.
const DIM: &str = "90";

/// Below this width the logo and the box are left out.
const NARROW: usize = 60;

/// Left margin inside the box, and the gap between the logo and the text.
const MARGIN: usize = 3;
const GAP: usize = 4;

/// Where the logo goes: beside the text, above it, or nowhere when it does not fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layout {
    Side,
    Stacked,
    TextOnly,
}

/// One painted line of the box's body and the columns it takes.
type BodyLine = (String, usize);

/// Builds the finished banner's lines for a terminal `width` columns wide. Each line fits in the width.
/// The run uses `render_banner_frames` and `render_banner_step`; this is for the tests.
/// @param info - what to show
/// @param width - the terminal width in columns
/// @param color - true to add ANSI colors
#[cfg(test)]
pub fn render_banner(info: &BannerInfo, width: usize, color: bool) -> Vec<String> {
    render_banner_frames(info, width, color).pop().unwrap_or_default()
}

/// Builds one step of the banner's reveal. Step 0 shows the letters alone and step `REVEAL_STEPS`
/// is the finished banner; a later step is the finished banner too.
/// @param info - what to show
/// @param width - the terminal width in columns
/// @param color - true to add ANSI colors
/// @param step - the step to build
pub fn render_banner_step(info: &BannerInfo, width: usize, color: bool, step: usize) -> Vec<String> {
    let mut frames = render_banner_frames(info, width, color);
    let index = step.min(frames.len().saturating_sub(1));
    frames.swap_remove(index)
}

/// Builds every step of the banner's reveal: the logo's letters alone first, then one more stripe at
/// each step. The last step is the finished banner, and every step has the same number of lines.
/// There is a single step without color or when the logo does not fit.
/// @param info - what to show
/// @param width - the terminal width in columns
/// @param color - true to add ANSI colors
pub fn render_banner_frames(info: &BannerInfo, width: usize, color: bool) -> Vec<Vec<String>> {
    let paint = |code: &str, text: &str| if color && !code.is_empty() { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() };
    let rows = text_rows(info);
    if width < NARROW {
        return vec![narrow_lines(&rows, width, &paint)];
    }
    let frames = logo_frames();
    let logo_width = frames.last().and_then(|f| f.first()).map_or(0, |r| r.len());
    let layout = choose_layout(width, logo_width, rows.iter().map(row_width).max().unwrap_or(0));
    let shown: Vec<&Pixels> = if color && layout != Layout::TextOnly { frames.iter().collect() } else { frames.last().into_iter().collect() };
    let truecolor = supports_truecolor();
    shown.into_iter().map(|pixels| {
        let logo = logo_block(pixels, color, truecolor, &paint);
        let mut lines = box_lines(layout_body(layout, logo, &rows, width, &paint), &paint);
        let hint = if info.model_menu { VIEW_HINT } else { HINT };
        lines.push(paint(DIM, &clip_end(hint, width.saturating_sub(1))));
        lines
    }).collect()
}

/// Picks where the logo goes. Beside the text when the text fits there in full, above the text when
/// only the logo fits, and nowhere when the logo does not fit at all.
/// @param width - the terminal width in columns
/// @param logo_width - the columns the logo takes
/// @param text_width - the columns the widest text row takes
fn choose_layout(width: usize, logo_width: usize, text_width: usize) -> Layout {
    if 2 + MARGIN + logo_width + GAP + text_width + MARGIN < width {
        Layout::Side
    } else if 2 + MARGIN + logo_width + MARGIN < width {
        Layout::Stacked
    } else {
        Layout::TextOnly
    }
}

/// Lays out the box's body: the logo beside the text, above it, or the text alone.
/// @param layout - where the logo goes
/// @param logo - the logo lines with the tagline under them
/// @param rows - the text rows
/// @param width - the terminal width in columns
/// @param paint - adds a color code to text
fn layout_body(layout: Layout, logo: Vec<BodyLine>, rows: &[Row], width: usize, paint: &dyn Fn(&str, &str) -> String) -> Vec<BodyLine> {
    let room = width.saturating_sub(3 + MARGIN * 2);
    let text_width = rows.iter().map(row_width).max().unwrap_or(0).min(room);
    let logo_width = logo.first().map_or(0, |l| l.1);
    match layout {
        Layout::Side => side_body(logo, rows, text_width, paint),
        Layout::Stacked => {
            let content = logo_width.max(text_width);
            let mut body: Vec<BodyLine> = logo.into_iter().map(|line| indent(line, (content - logo_width) / 2)).collect();
            body.push((String::new(), 0));
            body.extend(rows.iter().map(|row| indent(paint_row(row, text_width, paint), (content - text_width) / 2)));
            body
        }
        Layout::TextOnly => rows.iter().map(|row| paint_row(row, text_width, paint)).collect(),
    }
}

/// The logo with the tagline under it on the left, and the text rows on the right, centered on the
/// logo's height.
/// @param logo - the logo lines with the tagline under them
/// @param rows - the text rows
/// @param text_width - the most columns a text row may take
/// @param paint - adds a color code to text
fn side_body(logo: Vec<BodyLine>, rows: &[Row], text_width: usize, paint: &dyn Fn(&str, &str) -> String) -> Vec<BodyLine> {
    let top = logo.len().saturating_sub(rows.len()) / 2;
    logo.into_iter().enumerate().map(|(index, (text, used))| match index.checked_sub(top).and_then(|i| rows.get(i)) {
        Some(row) => {
            let (painted, columns) = paint_row(row, text_width, paint);
            (format!("{text}{}{painted}", " ".repeat(GAP)), used + GAP + columns)
        }
        None => (text, used),
    }).collect()
}

/// Moves a body line right by some columns.
/// @param line - the painted line and its columns
/// @param columns - the columns to add in front
fn indent(line: BodyLine, columns: usize) -> BodyLine {
    (format!("{}{}", " ".repeat(columns), line.0), line.1 + columns)
}

/// Puts the body in the box: the top and bottom lines, a blank line inside each, and the margins.
/// @param body - the painted lines and the columns each takes
/// @param paint - adds a color code to text
fn box_lines(body: Vec<BodyLine>, paint: &dyn Fn(&str, &str) -> String) -> Vec<String> {
    let inner = MARGIN * 2 + body.iter().map(|l| l.1).max().unwrap_or(0);
    let mut lines = vec![paint(BOX, &format!("╭{}╮", "─".repeat(inner))), boxed(paint, String::new(), 0, inner)];
    for (text, used) in body {
        lines.push(boxed(paint, format!("{}{text}", " ".repeat(MARGIN)), MARGIN + used, inner));
    }
    lines.push(boxed(paint, String::new(), 0, inner));
    lines.push(paint(BOX, &format!("╰{}╯", "─".repeat(inner))));
    lines
}

/// The logo's lines with "by Black Rainbow Labs" centered under them, each with the columns it takes.
/// @param pixels - one step of the logo
/// @param color - true to add ANSI colors
/// @param truecolor - true when the terminal shows 24 bit color
/// @param paint - adds a color code to text
fn logo_block(pixels: &Pixels, color: bool, truecolor: bool, paint: &dyn Fn(&str, &str) -> String) -> Vec<BodyLine> {
    let width = pixels.first().map_or(0, |r| r.len());
    let mut lines: Vec<BodyLine> = logo_lines(pixels, color, truecolor).into_iter().map(|l| (l, width)).collect();
    let tagline_width = 3 + TAGLINE_NAME.chars().count();
    let left = width.saturating_sub(tagline_width) / 2;
    let tagline = format!("{}{}{}", " ".repeat(left), paint(DIM, "by "), tagline_name(color, truecolor));
    let right = width.saturating_sub(left + tagline_width);
    lines.push((format!("{tagline}{}", " ".repeat(right)), width.max(tagline_width)));
    lines
}

/// "Black Rainbow Labs", each letter a step further along the tagline colors when color is on.
/// @param color - true to add ANSI colors
/// @param truecolor - true when the terminal shows 24 bit color
fn tagline_name(color: bool, truecolor: bool) -> String {
    if !color {
        return TAGLINE_NAME.to_string();
    }
    let last = TAGLINE_NAME.chars().count().saturating_sub(1).max(1);
    TAGLINE_NAME.chars().enumerate().map(|(i, c)| {
        let shade = gradient(&TAGLINE_STOPS, i as f32 / last as f32);
        format!("\x1b[1;{}m{c}\x1b[0m", color_code(38, shade, truecolor))
    }).collect()
}

/// The line under the banner that says how to steer a run.
const HINT: &str = "  Ctrl+C interrupts · /compact shrinks the history · lines typed while it works go to the next step";

/// The hint under the header in the fullscreen view.
const VIEW_HINT: &str = "  Ctrl+C interrupts · / lists commands · Tab moves through the transcript · drag to copy";

/// One text row: pieces of (color, text). The first piece is the label.
type Row = Vec<(&'static str, String)>;

/// The text rows beside the logo: name and version, model, folder, mode, skills and mcp.
/// @param info - what to show
fn text_rows(info: &BannerInfo) -> Vec<Row> {
    let mut model = vec![(DIM, "model   ".to_string()), ("", info.model.clone())];
    if info.model_menu {
        model.push((DIM, " ▾".to_string()));
    }
    let mut mcp = vec![(DIM, "mcp     ".to_string())];
    mcp.extend(info.mcp.iter().cloned());
    vec![
        vec![("1", "umbra".to_string()), (DIM, format!(" v{}", info.version))],
        model,
        vec![(DIM, "folder  ".to_string()), ("", info.folder.clone())],
        vec![(DIM, "mode    ".to_string()), ("", info.mode.clone())],
        vec![(DIM, "skills  ".to_string()), ("", info.skills.to_string())],
        mcp,
    ]
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
    // The model row's menu marker stays when the model is cut, so the row still reads as a menu.
    let reserve = row.last().filter(|(_, t)| t == " ▾").map_or(0, |(_, t)| t.chars().count());
    for (index, (code, text)) in row.iter().enumerate() {
        let last = index + 1 == row.len();
        let left = limit.saturating_sub(used + if last { 0 } else { reserve });
        let piece = clip_end(text, left);
        used += piece.chars().count();
        out.push_str(&paint(code, &piece));
    }
    (out, used)
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

/// Draws every step of the reveal, from the letters alone to all the stripes. Every step is cut to
/// the edges of the finished logo, so the logo keeps its size and place while the stripes appear.
fn logo_frames() -> Vec<Pixels> {
    let frames: Vec<Pixels> = (0..=STRIPES.len()).map(logo_pixels).collect();
    let Some(full) = frames.last() else { return frames };
    let (top, bottom, left, right) = used_bounds(full);
    frames.iter().map(|frame| frame[top..=bottom].iter().map(|row| row[left..=right].to_vec()).collect()).collect()
}

/// Draws the logo with its first `shown` stripes: the stripes from the back forward, each a copy of
/// the letters with its own outline, then the outlined cream letters in front.
/// @param shown - how many stripes to draw, from 0 to all of them
fn logo_pixels(shown: usize) -> Pixels {
    let word = word_shape();
    let reach = STRIPE_STEP * STRIPES.len();
    let (w, h) = (word.first().map_or(0, |r| r.len()) + 2 + reach + 2, word.len() + 2 + reach + 2);
    let mut letters = vec![vec![false; w]; h];
    for (y, row) in word.iter().enumerate() {
        for (x, on) in row.iter().enumerate() {
            letters[y + 2][x + 2 + reach] = *on;
        }
    }
    let outline = grow(&letters);
    let mut canvas: Pixels = vec![vec![None; w]; h];
    for i in (1..=shown.min(STRIPES.len())).rev() {
        let step = (STRIPE_STEP * i) as isize;
        paint_shape(&mut canvas, &outline, INK, -step, step);
        paint_shape(&mut canvas, &letters, STRIPES[i - 1], -step, step);
    }
    paint_shape(&mut canvas, &outline, INK, 0, 0);
    paint_shape(&mut canvas, &letters, FILL, 0, 0);
    canvas
}

/// Lays the letters side by side, bottom aligned, with one empty column between them.
fn word_shape() -> Vec<Vec<bool>> {
    let height = LETTERS.iter().map(|g| g.len()).max().unwrap_or(0);
    let mut rows = vec![Vec::new(); height];
    for (index, glyph) in LETTERS.iter().enumerate() {
        let width = glyph.first().map_or(0, |l| l.len());
        let top = height - glyph.len();
        for (y, row) in rows.iter_mut().enumerate() {
            if index > 0 {
                row.push(false);
            }
            match y.checked_sub(top).and_then(|i| glyph.get(i)) {
                Some(line) => row.extend(line.chars().map(|c| c == 'X')),
                None => row.extend(std::iter::repeat_n(false, width)),
            }
        }
    }
    rows
}

/// Grows a shape by one pixel in all eight directions, which makes its outline.
/// @param shape - the filled pixels
fn grow(shape: &[Vec<bool>]) -> Vec<Vec<bool>> {
    let filled = |x: isize, y: isize| y >= 0 && x >= 0 && shape.get(y as usize).and_then(|r| r.get(x as usize)).copied().unwrap_or(false);
    (0..shape.len()).map(|y| (0..shape[y].len()).map(|x| {
        (-1..=1).any(|dy| (-1..=1).any(|dx| filled(x as isize + dx, y as isize + dy)))
    }).collect()).collect()
}

/// Paints a color on the canvas wherever a shape is filled, moved by an offset.
/// @param canvas - the pixels, changed in place
/// @param shape - the filled pixels, the same size as the canvas
/// @param color - the color to paint
/// @param dx - the columns to move right
/// @param dy - the rows to move down
fn paint_shape(canvas: &mut Pixels, shape: &[Vec<bool>], color: Rgb, dx: isize, dy: isize) {
    for (y, row) in shape.iter().enumerate() {
        for (x, on) in row.iter().enumerate() {
            let (tx, ty) = (x as isize + dx, y as isize + dy);
            if *on && tx >= 0 && ty >= 0 {
                if let Some(cell) = canvas.get_mut(ty as usize).and_then(|r| r.get_mut(tx as usize)) {
                    *cell = Some(color);
                }
            }
        }
    }
}

/// The first and last rows and columns that have anything drawn, as (top, bottom, left, right). The
/// height is made even, so the logo folds into whole terminal rows.
/// @param pixels - the logo
fn used_bounds(pixels: &Pixels) -> (usize, usize, usize, usize) {
    let rows: Vec<usize> = (0..pixels.len()).filter(|&y| pixels[y].iter().any(Option::is_some)).collect();
    let width = pixels.first().map_or(0, |r| r.len());
    let cols: Vec<usize> = (0..width).filter(|&x| pixels.iter().any(|r| r[x].is_some())).collect();
    let (top, mut bottom) = (rows.first().copied().unwrap_or(0), rows.last().copied().unwrap_or(0));
    if (bottom - top + 1) % 2 == 1 {
        bottom = (bottom + 1).min(pixels.len().saturating_sub(1));
    }
    (top, bottom, cols.first().copied().unwrap_or(0), cols.last().copied().unwrap_or(0))
}

/// Folds the logo into terminal lines, two pixels to a cell: a half block with the top pixel's color
/// in front and the bottom pixel's color behind. Without color only the cream letters are drawn, so
/// the word can still be read.
/// @param pixels - the logo
/// @param color - true to add ANSI colors
/// @param truecolor - true when the terminal shows 24 bit color
fn logo_lines(pixels: &Pixels, color: bool, truecolor: bool) -> Vec<String> {
    let blank = vec![None; pixels.first().map_or(0, |r| r.len())];
    pixels.chunks(2).map(|pair| {
        let bottom = pair.get(1).unwrap_or(&blank);
        pair[0].iter().zip(bottom).map(|(&top, &under)| {
            if color {
                cell_text(top, under, truecolor)
            } else {
                block_char(top == Some(FILL), under == Some(FILL)).to_string()
            }
        }).collect()
    }).collect()
}

/// The character for a cell from which of its two pixels are filled.
/// @param top - true when the top pixel is filled
/// @param bottom - true when the bottom pixel is filled
fn block_char(top: bool, bottom: bool) -> char {
    match (top, bottom) {
        (true, true) => '█',
        (true, false) => '▀',
        (false, true) => '▄',
        (false, false) => ' ',
    }
}

/// One colored cell. An empty pixel is left to the terminal's own background.
/// @param top - the top pixel's color
/// @param bottom - the bottom pixel's color
/// @param truecolor - true when the terminal shows 24 bit color
fn cell_text(top: Option<Rgb>, bottom: Option<Rgb>, truecolor: bool) -> String {
    match (top, bottom) {
        (Some(t), Some(b)) if t != b => format!("\x1b[{};{}m▀\x1b[0m", color_code(38, t, truecolor), color_code(48, b, truecolor)),
        (Some(t), Some(_)) => format!("\x1b[{}m█\x1b[0m", color_code(38, t, truecolor)),
        (Some(t), None) => format!("\x1b[{}m▀\x1b[0m", color_code(38, t, truecolor)),
        (None, Some(b)) => format!("\x1b[{}m▄\x1b[0m", color_code(38, b, truecolor)),
        (None, None) => " ".to_string(),
    }
}

/// The ANSI code for a foreground (38) or background (48) color: 24 bit when the terminal shows it,
/// otherwise the nearest of the 256 colors.
/// @param layer - 38 for the foreground, 48 for the background
/// @param rgb - the color
/// @param truecolor - true when the terminal shows 24 bit color
fn color_code(layer: u8, rgb: Rgb, truecolor: bool) -> String {
    let (r, g, b) = rgb;
    if truecolor {
        return format!("{layer};2;{r};{g};{b}");
    }
    let level = |v: u8| if v < 48 { 0 } else if v < 115 { 1 } else { (v as u16 - 35) / 40 };
    format!("{layer};5;{}", 16 + 36 * level(r) + 6 * level(g) + level(b))
}

/// Reads a color from evenly spaced stops at a position from 0 to 1.
/// @param stops - the colors
/// @param t - the position
fn gradient(stops: &[Rgb], t: f32) -> Rgb {
    let scaled = t.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let i = (scaled.floor() as usize).min(stops.len() - 2);
    let f = scaled - i as f32;
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * f).round() as u8;
    let (a, b) = (stops[i], stops[i + 1]);
    (mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
}

/// True when the terminal says it shows 24 bit color: `COLORTERM` is truecolor or 24bit, or this is
/// Windows Terminal. Other terminals get the nearest of the 256 colors.
fn supports_truecolor() -> bool {
    matches!(std::env::var("COLORTERM").as_deref(), Ok("truecolor") | Ok("24bit")) || std::env::var_os("WT_SESSION").is_some()
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
        BannerInfo { version: "0.4.1".into(), model: "anthropic/claude-sonnet-5-5".into(), folder: "~/dev/colligo".into(), mode: "confirm".into(), skills: 12, mcp: vec![("", "2 running".into())], model_menu: false }
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
        for wanted in ["umbra v0.4.1", "model   anthropic/claude-sonnet-5-5", "folder  ~/dev/colligo", "mode    confirm ", "skills  12", "mcp     2 running", "by Black Rainbow Labs", "███"] {
            assert!(text.contains(wanted), "{wanted} in\n{text}");
        }
        assert!(!text.contains("instruction"), "the instruction file count is gone:\n{text}");
    }

    #[test]
    fn a_narrow_terminal_gets_no_logo() {
        let lines = render_banner(&info(), 40, false);
        assert_eq!(lines.len(), 6);
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

    /// Removes the ANSI color codes from text.
    fn strip(text: &str) -> String {
        regex::Regex::new("\x1b\\[[0-9;]*m").unwrap().replace_all(text, "").into_owned()
    }

    #[test]
    fn color_codes_do_not_change_the_layout() {
        for width in [80, 100, 160] {
            let plain = render_banner(&info(), width, false);
            let colored = render_banner(&info(), width, true);
            assert_eq!(plain.len(), colored.len(), "width {width}");
            for (p, c) in plain.iter().zip(&colored) {
                let stripped = strip(c);
                assert_eq!(p.chars().count(), stripped.chars().count(), "width {width}:\n{p}\n{stripped}");
                assert_eq!(p.replace(['█', '▀', '▄'], " "), stripped.replace(['█', '▀', '▄'], " "), "width {width}");
            }
        }
    }

    #[test]
    fn the_logo_goes_beside_the_text_when_both_fit() {
        let lines = render_banner(&info(), 160, false);
        assert_eq!(lines.len(), 13 + 4 + 1, "{lines:#?}");
        let tagline = lines.iter().position(|l| l.contains("by Black Rainbow Labs")).unwrap();
        assert_eq!(tagline, 14);
        assert!(lines.iter().any(|l| l.contains('█') && l.contains("umbra v0.4.1")), "{lines:#?}");
    }

    #[test]
    fn the_logo_goes_above_the_text_when_only_the_logo_fits() {
        let lines = render_banner(&info(), 80, false);
        let tagline = lines.iter().position(|l| l.contains("by Black Rainbow Labs")).unwrap();
        let version = lines.iter().position(|l| l.contains("umbra v0.4.1")).unwrap();
        assert!(tagline < version, "{lines:#?}");
    }

    #[test]
    fn a_box_too_narrow_for_the_logo_shows_the_text_alone() {
        let lines = render_banner(&info(), 62, true);
        assert!(lines.iter().all(|l| !l.contains("Rainbow")), "{lines:#?}");
        assert!(lines.iter().any(|l| l.contains("umbra")));
    }

    #[test]
    fn the_reveal_adds_one_stripe_at_each_step() {
        let frames = render_banner_frames(&info(), 160, true);
        assert_eq!(frames.len(), STRIPES.len() + 1);
        assert!(frames.iter().all(|f| f.len() == frames[0].len()));
        let truecolor = supports_truecolor();
        // Only the logo's lines, rows 2 to 13, so the tagline's colors are not counted.
        let uses = |step: usize, stripe: usize| {
            let logo = frames[step][2..14].join("");
            [38, 48].iter().any(|&layer| {
                let code = color_code(layer, STRIPES[stripe], truecolor);
                logo.contains(&format!("[{code}m")) || logo.contains(&format!("[{code};")) || logo.contains(&format!(";{code}m"))
            })
        };
        assert!(!uses(0, 0), "the letters alone show no green");
        assert!(uses(1, 0) && !uses(1, 1), "the first step adds green only");
        assert!(uses(4, 3) && !uses(4, 4), "blue comes last");
        assert!(uses(5, 4));
        assert_eq!(frames.last().unwrap(), &render_banner(&info(), 160, true));
        assert_eq!(render_banner_frames(&info(), 160, false).len(), 1);
        assert_eq!(render_banner_step(&info(), 160, true, 0), frames[0]);
        assert_eq!(render_banner_step(&info(), 160, true, REVEAL_STEPS + 3), frames[REVEAL_STEPS]);
    }

    #[test]
    fn the_letters_read_umbra_without_color() {
        let lines = render_banner(&info(), 160, false);
        let logo: Vec<String> = lines[2..14].iter().map(|l| l.chars().skip(1 + MARGIN).take(56).collect()).collect();
        let text = logo.join("\n");
        assert!(text.contains("███ ███ ███▀███▀███ ███▀███ ███▀██ ▄▄▄▄███"), "{text}");
    }

    #[test]
    fn the_home_folder_is_shown_as_a_tilde() {
        assert_eq!(display_folder("C:\\Users\\jason\\dev\\x", Some("C:\\Users\\jason")), "~/dev/x");
        assert_eq!(display_folder("/home/jasonb/x", Some("/home/jason")), "/home/jasonb/x");
        assert_eq!(display_folder("/srv/x", None), "/srv/x");
    }
}
