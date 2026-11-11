//! Renders a markdown reply into rows of terminal text, laid out for a given width.

use std::panic::{AssertUnwindSafe, catch_unwind};

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use crate::utils::screen::char_width;

const RESET: &str = "\x1b[0m";
const OSC8_CLOSE: &str = "\x1b]8;;\x1b\\";

/// One laid out row of a rendered reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MdRow {
    /// The row text, with SGR codes and OSC 8 links when color is on. Never wider than the width.
    pub text: String,
    /// True when this row continues the logical line of the row above (a soft wrap), so a copy joins them with no line break.
    pub cont: bool,
    /// Columns at the start of the row that are decoration (code block border, quote bar, list bullet indent is NOT decoration) and are left out when the row is copied.
    pub gutter: usize,
}

/// The text attributes of one run of inline text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Style {
    bold: bool,
    italic: bool,
    strike: bool,
    underline: bool,
    code: bool,
    dim: bool,
    link: Option<String>,
}

/// One piece of inline text: a word, a breakable space, or a hard line break.
#[derive(Debug, Clone)]
enum Tok {
    Word(String, Style),
    Space(Style),
    Br,
}

/// One container around the current block: a block quote or a list item.
struct Frame {
    quote: bool,
    marker: Option<String>,
    indent: usize,
}

/// A link whose text is still being collected.
struct LinkState {
    url: String,
    start: usize,
}

/// A table whose cells are still being collected.
struct TableState {
    aligns: Vec<Alignment>,
    rows: Vec<Vec<Vec<Tok>>>,
    row: Vec<Vec<Tok>>,
}

/// Everything the renderer keeps while it walks the markdown events.
struct Layout {
    rows: Vec<MdRow>,
    frames: Vec<Frame>,
    inline: Vec<Tok>,
    styles: Vec<Style>,
    lists: Vec<Option<u64>>,
    links: Vec<LinkState>,
    code: Option<(String, String)>,
    table: Option<TableState>,
    suppress_gap: bool,
    width: usize,
    color: bool,
}

/// The text a row starts with for the containers around a block.
struct Prefix {
    text: String,
    width: usize,
    quote_cols: usize,
}

/// How the start of a row is treated when it is copied.
enum Deco {
    Plain,
    Code(usize),
}

/// Renders markdown source into rows at most `width` columns wide. A panic inside layout is caught and the source is drawn as plain wrapped text instead.
/// @param source - the markdown text
/// @param width - the number of columns available
/// @param color - true to emit SGR codes and OSC 8 links
pub fn render(source: &str, width: usize, color: bool) -> Vec<MdRow> {
    let width = width.max(1);
    match crate::utils::screen::quiet_panics(|| catch_unwind(AssertUnwindSafe(|| layout(source, width, color)))) {
        Ok(rows) => rows,
        Err(_) => plain_rows(&sanitize(source), width),
    }
}

/// Removes every control character except newline and tab, including ESC, so model output cannot inject terminal sequences. A whole OSC or CSI sequence is removed with its ESC, so its text does not stay behind.
/// @param text - the text to clean
pub fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            skip_sequence(&mut chars);
        } else if !c.is_control() || c == '\n' || c == '\t' {
            out.push(c);
        }
    }
    out
}

/// Consumes the rest of an escape sequence after its ESC: an OSC up to BEL or ESC backslash, a CSI up to its final byte, or one more character. A newline is never consumed.
/// @param chars - the characters after the ESC
fn skip_sequence(chars: &mut std::iter::Peekable<std::str::Chars>) {
    match chars.peek().copied() {
        Some(']') => {
            chars.next();
            while let Some(&c) = chars.peek() {
                if c == '\n' {
                    break;
                }
                chars.next();
                if c == '\x07' || c == '\u{9c}' {
                    break;
                }
                if c == '\x1b' {
                    if chars.peek() == Some(&'\\') {
                        chars.next();
                    }
                    break;
                }
            }
        }
        Some('[') => {
            chars.next();
            while let Some(&c) = chars.peek() {
                if c == '\n' {
                    break;
                }
                chars.next();
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
        }
        Some(c) if c != '\n' => {
            chars.next();
        }
        _ => {}
    }
}

/// Lays out the source with the markdown parser, without the panic guard.
/// @param source - the markdown text
/// @param width - the number of columns available
/// @param color - true to emit SGR codes and OSC 8 links
fn layout(source: &str, width: usize, color: bool) -> Vec<MdRow> {
    let clean = sanitize(source);
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut lay = Layout {
        rows: Vec::new(),
        frames: Vec::new(),
        inline: Vec::new(),
        styles: Vec::new(),
        lists: Vec::new(),
        links: Vec::new(),
        code: None,
        table: None,
        suppress_gap: false,
        width,
        color,
    };
    for event in Parser::new_ext(&clean, options) {
        handle_event(&mut lay, event);
    }
    flush_inline(&mut lay);
    lay.rows
}

/// Draws the source as plain text cut at the width, used when layout panics.
/// @param text - the cleaned source text
/// @param width - the number of columns available
fn plain_rows(text: &str, width: usize) -> Vec<MdRow> {
    let mut rows = Vec::new();
    for line in text.split('\n') {
        for (index, chunk) in hard_wrap(line, width, width).into_iter().enumerate() {
            rows.push(MdRow { text: chunk, cont: index > 0, gutter: 0 });
        }
    }
    rows
}

/// Applies one parser event to the layout.
/// @param lay - the layout state
/// @param event - the parser event
fn handle_event(lay: &mut Layout, event: Event) {
    match event {
        Event::Start(tag) => start_tag(lay, tag),
        Event::End(tag) => end_tag(lay, tag),
        Event::Text(text) => text_event(lay, &text),
        Event::Code(text) => {
            let style = Style { code: true, ..current_style(lay) };
            push_text(lay, &sanitize(&text), &style, false);
        }
        Event::Html(text) | Event::InlineHtml(text) => {
            let style = current_style(lay);
            push_text(lay, &sanitize(&text), &style, true);
        }
        Event::SoftBreak => lay.inline.push(Tok::Space(current_style(lay))),
        Event::HardBreak => lay.inline.push(Tok::Br),
        Event::Rule => rule_row(lay),
        Event::TaskListMarker(done) => set_task_marker(lay, done),
        Event::FootnoteReference(name) => {
            let style = current_style(lay);
            push_text(lay, &format!("[^{}]", sanitize(&name)), &style, false);
        }
        _ => {}
    }
}

/// Handles the start of a block or inline container.
/// @param lay - the layout state
/// @param tag - the tag that started
fn start_tag(lay: &mut Layout, tag: Tag) {
    match tag {
        Tag::Paragraph | Tag::HtmlBlock => begin_block(lay),
        Tag::Heading { level, .. } => {
            begin_block(lay);
            let n = level as usize;
            let style = Style { bold: n <= 3, underline: n == 1, ..current_style(lay) };
            lay.styles.push(style);
        }
        Tag::BlockQuote(_) => {
            begin_block(lay);
            lay.frames.push(Frame { quote: true, marker: None, indent: 2 });
            lay.suppress_gap = true;
        }
        Tag::CodeBlock(kind) => {
            begin_block(lay);
            lay.code = Some((String::new(), code_language(&kind)));
        }
        Tag::List(start) => start_list(lay, start),
        Tag::Item => start_item(lay),
        Tag::Table(aligns) => {
            begin_block(lay);
            lay.table = Some(TableState { aligns, rows: Vec::new(), row: Vec::new() });
        }
        Tag::TableCell => lay.inline.clear(),
        Tag::Emphasis => push_modified_style(lay, |s| s.italic = true),
        Tag::Strong => push_modified_style(lay, |s| s.bold = true),
        Tag::Strikethrough => push_modified_style(lay, |s| s.strike = true),
        Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => start_link(lay, &dest_url),
        _ => {}
    }
}

/// Handles the end of a block or inline container.
/// @param lay - the layout state
/// @param tag - the tag that ended
fn end_tag(lay: &mut Layout, tag: TagEnd) {
    match tag {
        TagEnd::Paragraph | TagEnd::HtmlBlock | TagEnd::List(_) => {
            flush_inline(lay);
            if matches!(tag, TagEnd::List(_)) {
                lay.lists.pop();
            }
        }
        TagEnd::Heading(_) => {
            flush_inline(lay);
            lay.styles.pop();
        }
        TagEnd::BlockQuote(_) => {
            flush_inline(lay);
            lay.frames.pop();
        }
        TagEnd::CodeBlock => finish_code(lay),
        TagEnd::Item => end_item(lay),
        TagEnd::Table => finish_table(lay),
        TagEnd::TableHead | TagEnd::TableRow => end_table_row(lay),
        TagEnd::TableCell => {
            let cell = std::mem::take(&mut lay.inline);
            if let Some(table) = lay.table.as_mut() {
                table.row.push(cell);
            }
        }
        TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
            lay.styles.pop();
        }
        TagEnd::Link | TagEnd::Image => end_link(lay),
        _ => {}
    }
}

/// Adds text from a Text event to the code block being collected or to the inline text.
/// @param lay - the layout state
/// @param text - the event text
fn text_event(lay: &mut Layout, text: &str) {
    let clean = sanitize(text);
    if let Some((buffer, _)) = lay.code.as_mut() {
        buffer.push_str(&clean);
        return;
    }
    let style = current_style(lay);
    push_text(lay, &clean, &style, false);
}

/// The style in effect for new inline text.
/// @param lay - the layout state
fn current_style(lay: &Layout) -> Style {
    lay.styles.last().cloned().unwrap_or_default()
}

/// Pushes a copy of the current style changed by `change`.
/// @param lay - the layout state
/// @param change - how to change the copy
fn push_modified_style(lay: &mut Layout, change: impl FnOnce(&mut Style)) {
    let mut style = current_style(lay);
    change(&mut style);
    lay.styles.push(style);
}

/// The first word of a fenced code block's info string, or an empty string.
/// @param kind - the code block kind
fn code_language(kind: &CodeBlockKind) -> String {
    match kind {
        CodeBlockKind::Fenced(info) => sanitize(info).split_whitespace().next().unwrap_or("").to_string(),
        CodeBlockKind::Indented => String::new(),
    }
}

/// Splits text into words and spaces and appends them to the inline tokens.
/// @param lay - the layout state
/// @param text - the cleaned text
/// @param style - the style for the tokens
/// @param breaks - true to turn each newline into a hard line break
fn push_text(lay: &mut Layout, text: &str, style: &Style, breaks: bool) {
    let mut word = String::new();
    for c in text.chars() {
        if c == '\n' && breaks {
            flush_word(&mut lay.inline, &mut word, style);
            lay.inline.push(Tok::Br);
        } else if c.is_whitespace() {
            flush_word(&mut lay.inline, &mut word, style);
            if !matches!(lay.inline.last(), Some(Tok::Space(_)) | Some(Tok::Br) | None) {
                lay.inline.push(Tok::Space(style.clone()));
            }
        } else {
            word.push(c);
        }
    }
    flush_word(&mut lay.inline, &mut word, style);
}

/// Moves a collected word onto the token list.
/// @param inline - the token list
/// @param word - the word, left empty
/// @param style - the style for the word
fn flush_word(inline: &mut Vec<Tok>, word: &mut String, style: &Style) {
    if !word.is_empty() {
        inline.push(Tok::Word(std::mem::take(word), style.clone()));
    }
}

/// Starts a link or image: the text is underlined and carries the URL when its scheme is allowed.
/// @param lay - the layout state
/// @param dest - the raw destination URL
fn start_link(lay: &mut Layout, dest: &str) {
    let url = sanitize(dest).trim().to_string();
    let allowed = scheme_allowed(&url);
    lay.links.push(LinkState { url: url.clone(), start: lay.inline.len() });
    push_modified_style(lay, |s| {
        s.underline = true;
        s.link = if allowed { Some(url) } else { None };
    });
}

/// Ends a link and appends its URL in dim when the link text differs from the URL.
/// @param lay - the layout state
fn end_link(lay: &mut Layout) {
    lay.styles.pop();
    let Some(link) = lay.links.pop() else { return };
    let text: String = lay.inline[link.start.min(lay.inline.len())..]
        .iter()
        .map(|tok| match tok {
            Tok::Word(t, _) => t.as_str(),
            Tok::Space(_) => " ",
            Tok::Br => " ",
        })
        .collect();
    if link.url.is_empty() || text.trim() == link.url {
        return;
    }
    lay.inline.push(Tok::Space(current_style(lay)));
    lay.inline.push(Tok::Word(format!("({})", link.url), Style { dim: true, ..Style::default() }));
}

/// True when a URL may be sent as an OSC 8 hyperlink.
/// @param url - the sanitized URL
fn scheme_allowed(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("file:")
}

/// Starts a list and, when it is not inside a list item, separates it from the block above.
/// @param lay - the layout state
/// @param start - the first number of an ordered list, or None for a bullet list
fn start_list(lay: &mut Layout, start: Option<u64>) {
    if lay.frames.last().is_none_or(|f| f.quote) {
        begin_block(lay);
    } else {
        flush_inline(lay);
    }
    lay.lists.push(start);
}

/// Starts a list item with its bullet or number as a marker for the first row.
/// @param lay - the layout state
fn start_item(lay: &mut Layout) {
    flush_inline(lay);
    let marker = match lay.lists.last_mut() {
        Some(Some(n)) => {
            let text = format!("{n}. ");
            *n += 1;
            text
        }
        _ => "• ".to_string(),
    };
    let indent = text_width(&marker);
    lay.frames.push(Frame { quote: false, marker: Some(marker), indent });
    lay.suppress_gap = true;
}

/// Ends a list item, drawing its marker alone when the item had no content.
/// @param lay - the layout state
fn end_item(lay: &mut Layout) {
    flush_inline(lay);
    if lay.frames.last().is_some_and(|f| f.marker.is_some()) {
        emit_line(lay, "", false, Deco::Plain);
    }
    lay.frames.pop();
}

/// Replaces the bullet of the current list item with a task box.
/// @param lay - the layout state
/// @param done - true for a checked item
fn set_task_marker(lay: &mut Layout, done: bool) {
    if let Some(frame) = lay.frames.last_mut() {
        if frame.marker.is_some() {
            frame.marker = Some(if done { "☑ " } else { "☐ " }.to_string());
        }
    }
}

/// Starts a block: writes any pending inline text and one blank row unless the block is the first or follows a marker.
/// @param lay - the layout state
fn begin_block(lay: &mut Layout) {
    flush_inline(lay);
    if !lay.rows.is_empty() && !lay.suppress_gap {
        emit_blank(lay);
    }
    lay.suppress_gap = false;
}

/// The columns left for content inside the containers around the current block.
/// @param lay - the layout state
fn avail(lay: &Layout) -> usize {
    let used: usize = lay.frames.iter().map(|f| f.indent).sum();
    lay.width.saturating_sub(used).max(1)
}

/// Builds the text a row starts with. The first row of a list item uses its marker, later rows use spaces.
/// @param lay - the layout state
/// @param blank - true for an empty row, which keeps quote bars and drops trailing spaces
fn build_prefix(lay: &mut Layout, blank: bool) -> Prefix {
    let mut pieces: Vec<(String, bool)> = Vec::new();
    let (mut quote_cols, mut seen_item) = (0, false);
    for frame in lay.frames.iter_mut() {
        if frame.quote {
            pieces.push(("▎ ".to_string(), true));
            if !seen_item {
                quote_cols += 2;
            }
        } else {
            seen_item = true;
            let marker = if blank { None } else { frame.marker.take() };
            pieces.push((marker.unwrap_or_else(|| " ".repeat(frame.indent)), false));
        }
    }
    if blank {
        trim_pieces(&mut pieces);
    }
    let width = pieces.iter().map(|(t, _)| text_width(t)).sum();
    let text = pieces.iter().map(|(t, dim)| dim_text(t, *dim && lay.color)).collect();
    Prefix { text, width, quote_cols: quote_cols.min(width) }
}

/// Removes trailing spaces from the last pieces of a prefix.
/// @param pieces - the prefix pieces with their dim flags
fn trim_pieces(pieces: &mut Vec<(String, bool)>) {
    while let Some(last) = pieces.last_mut() {
        let trimmed = last.0.trim_end().to_string();
        if trimmed.is_empty() {
            pieces.pop();
        } else {
            last.0 = trimmed;
            break;
        }
    }
}

/// Wraps text in the dim code when `on` is true.
/// @param text - the text
/// @param on - true to make it dim
fn dim_text(text: &str, on: bool) -> String {
    if on { format!("\x1b[2m{text}{RESET}") } else { text.to_string() }
}

/// Adds one row with the container prefix in front of its body.
/// @param lay - the layout state
/// @param body - the painted row content
/// @param cont - true when the row continues the row above
/// @param deco - how many leading columns are decoration for copying
fn emit_line(lay: &mut Layout, body: &str, cont: bool, deco: Deco) {
    let prefix = build_prefix(lay, false);
    let gutter = match deco {
        Deco::Plain => prefix.quote_cols,
        Deco::Code(border) => prefix.width + border,
    };
    lay.rows.push(MdRow { text: format!("{}{}", prefix.text, body), cont, gutter });
    lay.suppress_gap = false;
}

/// Adds an empty row that keeps the quote bars of the containers around it.
/// @param lay - the layout state
fn emit_blank(lay: &mut Layout) {
    let prefix = build_prefix(lay, true);
    lay.rows.push(MdRow { text: prefix.text, cont: false, gutter: prefix.quote_cols });
}

/// Wraps the pending inline tokens to the available width and adds them as rows.
/// @param lay - the layout state
fn flush_inline(lay: &mut Layout) {
    let toks = std::mem::take(&mut lay.inline);
    if toks.is_empty() {
        return;
    }
    let width = avail(lay);
    for (line, cont) in wrap_tokens(&toks, width) {
        let body = paint_tokens(&line, lay.color);
        emit_line(lay, &body, cont, Deco::Plain);
    }
}

/// Adds a dim row of horizontal lines the full available width.
/// @param lay - the layout state
fn rule_row(lay: &mut Layout) {
    begin_block(lay);
    let line = "─".repeat(avail(lay));
    let body = dim_text(&line, lay.color);
    emit_line(lay, &body, false, Deco::Plain);
}

/// The columns a string takes on screen.
/// @param text - the text without escape codes
fn text_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

/// The columns a list of tokens takes on screen.
/// @param toks - the tokens
fn tokens_width(toks: &[Tok]) -> usize {
    toks.iter()
        .map(|tok| match tok {
            Tok::Word(t, _) => text_width(t),
            Tok::Space(_) => 1,
            Tok::Br => 0,
        })
        .sum()
}

/// Cuts text into chunks by column width. The first chunk may be a different width than the rest.
/// @param text - the text to cut
/// @param first - the width of the first chunk
/// @param rest - the width of every later chunk
fn hard_wrap(text: &str, first: usize, rest: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let (mut used, mut limit) = (0, first.max(1));
    for c in text.chars() {
        let w = char_width(c);
        if used + w > limit && used > 0 {
            out.push(std::mem::take(&mut current));
            used = 0;
            limit = rest.max(1);
        }
        current.push(c);
        used += w;
    }
    out.push(current);
    out
}

/// Open codes for a style: SGR attributes first, then the OSC 8 link start.
/// @param style - the style to start
fn style_open(style: &Style) -> String {
    let mut out = String::new();
    for (on, code) in [(style.bold, "1"), (style.dim, "2"), (style.italic, "3"), (style.underline, "4"), (style.strike, "9"), (style.code, "36")] {
        if on {
            out.push_str(&format!("\x1b[{code}m"));
        }
    }
    if let Some(url) = &style.link {
        out.push_str(&format!("\x1b]8;;{url}\x1b\\"));
    }
    out
}

/// Close codes for a style: the OSC 8 link end and a reset when any attribute was on.
/// @param style - the style to end
fn style_close(style: &Style) -> String {
    let mut out = String::new();
    if style.link.is_some() {
        out.push_str(OSC8_CLOSE);
    }
    if style.bold || style.dim || style.italic || style.underline || style.strike || style.code {
        out.push_str(RESET);
    }
    out
}

/// Joins tokens into one row string, with codes at each style change when color is on.
/// @param toks - the tokens of one row
/// @param color - true to emit SGR codes and OSC 8 links
fn paint_tokens(toks: &[Tok], color: bool) -> String {
    let mut out = String::new();
    let mut current = Style::default();
    for tok in toks {
        let (text, style) = match tok {
            Tok::Word(t, s) => (t.as_str(), s),
            Tok::Space(s) => (" ", s),
            Tok::Br => continue,
        };
        if color && *style != current {
            out.push_str(&style_close(&current));
            out.push_str(&style_open(style));
            current = style.clone();
        }
        out.push_str(text);
    }
    if color {
        out.push_str(&style_close(&current));
    }
    out
}

/// The line state while tokens are wrapped.
struct Wrapper {
    lines: Vec<(Vec<Tok>, bool)>,
    cur: Vec<Tok>,
    used: usize,
    pending: Option<Style>,
    cont: bool,
    width: usize,
}

/// Wraps tokens at spaces into lines no wider than `width`. Words wider than the width are broken. Each line says whether it continues the line above.
/// @param toks - the tokens
/// @param width - the columns available
fn wrap_tokens(toks: &[Tok], width: usize) -> Vec<(Vec<Tok>, bool)> {
    let mut w = Wrapper { lines: Vec::new(), cur: Vec::new(), used: 0, pending: None, cont: false, width: width.max(1) };
    for tok in toks {
        match tok {
            Tok::Br => {
                w.lines.push((std::mem::take(&mut w.cur), w.cont));
                (w.used, w.pending, w.cont) = (0, None, false);
            }
            Tok::Space(style) => {
                if w.used > 0 {
                    w.pending = Some(style.clone());
                }
            }
            Tok::Word(text, style) => place_word(&mut w, text, style),
        }
    }
    if !w.cur.is_empty() {
        w.lines.push((w.cur, w.cont));
    }
    w.lines
}

/// Ends the current wrapped line and starts a continuation line.
/// @param w - the wrapper state
fn break_line(w: &mut Wrapper) {
    w.lines.push((std::mem::take(&mut w.cur), w.cont));
    (w.used, w.pending, w.cont) = (0, None, true);
}

/// Puts one word on the current line, starting a new line or breaking the word when it does not fit.
/// @param w - the wrapper state
/// @param text - the word
/// @param style - the word's style
fn place_word(w: &mut Wrapper, text: &str, style: &Style) {
    let word_width = text_width(text);
    let space = w.pending.is_some() as usize;
    if w.used > 0 && w.used + space + word_width > w.width {
        break_line(w);
    }
    if let Some(space_style) = w.pending.take() {
        if w.used > 0 {
            w.cur.push(Tok::Space(space_style));
            w.used += 1;
        }
    }
    if w.used + word_width <= w.width {
        w.cur.push(Tok::Word(text.to_string(), style.clone()));
        w.used += word_width;
        return;
    }
    for (index, piece) in hard_wrap(text, w.width, w.width).into_iter().enumerate() {
        if index > 0 {
            break_line(w);
        }
        w.used = text_width(&piece);
        w.cur.push(Tok::Word(piece, style.clone()));
    }
}

/// Adds the rows of a finished code block: the language name, then each code line cut at the width,
/// with syntax colors when color is on and the highlighter knows the language.
/// @param lay - the layout state
fn finish_code(lay: &mut Layout) {
    let Some((buffer, lang)) = lay.code.take() else { return };
    let color = lay.color;
    let border = dim_text("│ ", color);
    if !lang.is_empty() {
        let body = format!("{border}{}", dim_text(&lang, color));
        emit_line(lay, &body, false, Deco::Code(2));
    }
    let text = buffer.replace('\t', "    ");
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let inner = avail(lay).saturating_sub(2).max(2);
    let colored = if color && super::highlight::enabled() { super::highlight::highlight(&lines.join("\n"), &lang) } else { None };
    match colored {
        Some(colored) => {
            for spans in colored {
                emit_colored_code_line(lay, &spans, inner, &border);
            }
        }
        None => {
            for line in lines {
                emit_code_line(lay, line, inner, &border);
            }
        }
    }
}

/// Adds one syntax colored code line as one row, or several rows when it is longer than the width.
/// @param lay - the layout state
/// @param spans - the line's colored pieces
/// @param inner - the columns inside the border
/// @param border - the painted border text
fn emit_colored_code_line(lay: &mut Layout, spans: &[super::highlight::Span], inner: usize, border: &str) {
    let mark = dim_text("↪", lay.color);
    for (index, chunk) in super::highlight::wrap(spans, inner, inner.saturating_sub(1)).into_iter().enumerate() {
        let body = super::highlight::paint(&chunk);
        if index == 0 {
            emit_line(lay, &format!("{border}{body}"), false, Deco::Code(2));
        } else {
            emit_line(lay, &format!("{border}{mark}{body}"), true, Deco::Code(3));
        }
    }
}

/// Adds one code line as one row, or several rows when it is longer than the width.
/// @param lay - the layout state
/// @param line - the code line
/// @param inner - the columns inside the border
/// @param border - the painted border text
fn emit_code_line(lay: &mut Layout, line: &str, inner: usize, border: &str) {
    let mark = dim_text("↪", lay.color);
    for (index, chunk) in hard_wrap(line, inner, inner.saturating_sub(1)).into_iter().enumerate() {
        if index == 0 {
            emit_line(lay, &format!("{border}{chunk}"), false, Deco::Code(2));
        } else {
            emit_line(lay, &format!("{border}{mark}{chunk}"), true, Deco::Code(3));
        }
    }
}

/// Moves the cells collected for the current table row into the table.
/// @param lay - the layout state
fn end_table_row(lay: &mut Layout) {
    if let Some(table) = lay.table.as_mut() {
        let row = std::mem::take(&mut table.row);
        table.rows.push(row);
    }
}

/// Draws a finished table as a bordered grid, or as header and value lines when the columns would be under 6 wide.
/// @param lay - the layout state
fn finish_table(lay: &mut Layout) {
    let Some(table) = lay.table.take() else { return };
    let cols = table.aligns.len().max(table.rows.iter().map(|r| r.len()).max().unwrap_or(0));
    if cols == 0 || table.rows.is_empty() {
        return;
    }
    let room = avail(lay);
    if room < 9 * cols + 1 {
        emit_records(lay, &table, cols);
        return;
    }
    let widths = column_widths(&table.rows, cols, room);
    emit_grid(lay, &table, &widths);
}

/// Sizes the columns to their content, shrinking them in proportion when the table is wider than the room.
/// @param rows - the table rows
/// @param cols - the number of columns
/// @param room - the columns available for the whole table
fn column_widths(rows: &[Vec<Vec<Tok>>], cols: usize, room: usize) -> Vec<usize> {
    let natural: Vec<usize> = (0..cols)
        .map(|i| rows.iter().filter_map(|r| r.get(i)).map(|c| natural_width(c)).max().unwrap_or(0).max(1))
        .collect();
    let budget = room - (3 * cols + 1);
    if natural.iter().sum::<usize>() <= budget {
        return natural;
    }
    shrink_widths(&natural, budget)
}

/// The width of a cell on one line.
/// @param cell - the cell tokens
fn natural_width(cell: &[Tok]) -> usize {
    wrap_tokens(cell, 10_000).iter().map(|(line, _)| tokens_width(line)).max().unwrap_or(0)
}

/// Divides `budget` columns between columns in proportion to their natural widths. No column goes below 6, or below its natural width when that is smaller.
/// @param natural - each column's natural width
/// @param budget - the total columns for cell text
fn shrink_widths(natural: &[usize], budget: usize) -> Vec<usize> {
    let total: usize = natural.iter().sum();
    let floors: Vec<usize> = natural.iter().map(|&n| n.min(6)).collect();
    let mut widths: Vec<usize> = natural.iter().zip(&floors).map(|(n, f)| (n * budget / total).max(*f).min(*n)).collect();
    while widths.iter().sum::<usize>() > budget {
        let Some(index) = (0..widths.len()).filter(|&i| widths[i] > floors[i]).max_by_key(|&i| widths[i]) else { break };
        widths[index] -= 1;
    }
    while widths.iter().sum::<usize>() < budget {
        let Some(index) = (0..widths.len()).filter(|&i| widths[i] < natural[i]).max_by_key(|&i| natural[i] - widths[i]) else { break };
        widths[index] += 1;
    }
    widths
}

/// Draws a horizontal border row of a table grid.
/// @param left - the left corner character
/// @param mid - the character between columns
/// @param right - the right corner character
/// @param widths - the column widths
fn border_row(left: char, mid: char, right: char, widths: &[usize]) -> String {
    let parts: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
    format!("{left}{}{right}", parts.join(&mid.to_string()))
}

/// Draws the grid: top border, header, a separator, the body rows and the bottom border.
/// @param lay - the layout state
/// @param table - the collected table
/// @param widths - the column widths
fn emit_grid(lay: &mut Layout, table: &TableState, widths: &[usize]) {
    emit_line(lay, &border_row('┌', '┬', '┐', widths), false, Deco::Plain);
    for (index, row) in table.rows.iter().enumerate() {
        emit_cell_row(lay, row, widths, &table.aligns, index == 0);
        if index == 0 {
            emit_line(lay, &border_row('├', '┼', '┤', widths), false, Deco::Plain);
        }
    }
    emit_line(lay, &border_row('└', '┴', '┘', widths), false, Deco::Plain);
}

/// Draws one table row, which is several screen rows when a cell wraps.
/// @param lay - the layout state
/// @param row - the cells of the row
/// @param widths - the column widths
/// @param aligns - the column alignments
/// @param header - true for the header row, which is bold
fn emit_cell_row(lay: &mut Layout, row: &[Vec<Tok>], widths: &[usize], aligns: &[Alignment], header: bool) {
    let cells: Vec<Vec<(Vec<Tok>, bool)>> = widths
        .iter()
        .enumerate()
        .map(|(i, &w)| {
            let cell = row.get(i).cloned().unwrap_or_default();
            wrap_tokens(&if header { with_bold(&cell) } else { cell }, w)
        })
        .collect();
    let height = cells.iter().map(|c| c.len()).max().unwrap_or(1).max(1);
    for line in 0..height {
        let mut text = String::from("│");
        for (i, &w) in widths.iter().enumerate() {
            let toks: &[Tok] = cells[i].get(line).map(|(t, _)| t.as_slice()).unwrap_or(&[]);
            let align = aligns.get(i).copied().unwrap_or(Alignment::None);
            text.push_str(&format!(" {} │", pad_cell(&paint_tokens(toks, lay.color), tokens_width(toks), w, align)));
        }
        emit_line(lay, &text, false, Deco::Plain);
    }
}

/// Pads a painted cell with spaces to the column width, by the column alignment.
/// @param painted - the cell text with codes
/// @param used - the columns the cell takes
/// @param width - the column width
/// @param align - the column alignment
fn pad_cell(painted: &str, used: usize, width: usize, align: Alignment) -> String {
    let total = width.saturating_sub(used);
    let left = match align {
        Alignment::Right => total,
        Alignment::Center => total / 2,
        _ => 0,
    };
    format!("{}{}{}", " ".repeat(left), painted, " ".repeat(total - left))
}

/// Copies tokens with bold on.
/// @param toks - the tokens
fn with_bold(toks: &[Tok]) -> Vec<Tok> {
    toks.iter()
        .map(|tok| match tok {
            Tok::Word(t, s) => Tok::Word(t.clone(), Style { bold: true, ..s.clone() }),
            Tok::Space(s) => Tok::Space(Style { bold: true, ..s.clone() }),
            Tok::Br => Tok::Br,
        })
        .collect()
}

/// Draws a table that is too wide for a grid as one block of `header: value` lines per data row.
/// @param lay - the layout state
/// @param table - the collected table
/// @param cols - the number of columns
fn emit_records(lay: &mut Layout, table: &TableState, cols: usize) {
    let header = &table.rows[0];
    for (index, row) in table.rows.iter().enumerate().skip(1) {
        if index > 1 {
            emit_blank(lay);
        }
        for i in 0..cols {
            let mut toks = with_bold(&header.get(i).cloned().unwrap_or_default());
            toks.push(Tok::Word(":".to_string(), Style { bold: true, ..Style::default() }));
            toks.push(Tok::Space(Style::default()));
            toks.extend(row.get(i).cloned().unwrap_or_default());
            for (line, cont) in wrap_tokens(&toks, avail(lay)) {
                let body = paint_tokens(&line, lay.color);
                emit_line(lay, &body, cont, Deco::Plain);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::screen::visible_width;

    #[test]
    fn a_known_fence_is_colored_and_the_setting_turns_it_off() {
        let source = "```rust\nfn main() { let x = 42; } // done\n```";
        let colored: String = render(source, 60, true).into_iter().map(|r| r.text).collect();
        assert!(colored.contains("\x1b[35mfn\x1b[0m"), "{colored:?}");
        assert!(colored.contains("\x1b[33m42\x1b[0m"), "{colored:?}");
        assert!(colored.contains("\x1b[90;3m// done\x1b[0m"), "{colored:?}");
        let unknown: String = render("```nothing\nfn main()\n```", 60, true).into_iter().map(|r| r.text).collect();
        assert!(!unknown.contains("\x1b[35m"), "{unknown:?}");
        super::super::highlight::set_enabled(false);
        let plain: String = render(source, 60, true).into_iter().map(|r| r.text).collect();
        super::super::highlight::set_enabled(true);
        assert!(!plain.contains("\x1b[35mfn"), "{plain:?}");
        let narrow = render(source, 20, true);
        assert!(narrow.iter().all(|r| visible_width(&r.text) <= 20), "{narrow:?}");
    }

    /// The fixed markdown samples, one for each construct.
    fn samples() -> Vec<(&'static str, &'static str)> {
        vec![
            ("headings", "# Title one\n\nIntro paragraph under the first heading.\n\n## Second level heading that is long enough to wrap on narrow widths\n\n### Third\n\n#### Fourth\n\nText after."),
            ("emphasis", "Plain **bold text** and *italic text* and ~~struck text~~ and ***bold italic*** with **bold and *nested italic* inside** to wrap the paragraph across rows at narrow widths."),
            ("inline_code", "Use `cargo test --lib` to run and `x` for short, or call `some_function_with_a_long_name(argument_one, argument_two)` in a sentence."),
            ("fenced_code", "```rust\nfn main() {\n    println!(\"hello\");\n}\n\nfn long() { let value = some_function_name(first_argument, second_argument, third_argument); }\n```\n\n    indented code\n    second line\n\nAfter."),
            ("nested_list", "- one\n- two with a long line of text that must wrap and align under the item text\n  - nested a\n  - nested b\n    1. deep first\n    2. deep second\n- [ ] todo\n- [x] done\n\n8. eight\n9. nine\n10. ten"),
            ("quote", "> A quoted paragraph that is long enough to wrap onto more than one row at the narrow width.\n>\n> > Nested quote\n>\n> - item in quote\n\nAfter quote."),
            ("table", "| Name | Qty | Price | Notes |\n|:-----|:---:|------:|-------|\n| Apple | 3 | 1.25 | Red and crisp, sold by the bag |\n| Watermelon slices | 12 | 10.5 | Seasonal |\n| Fig | 100 | 0.5 | |\n\nAfter."),
            ("table_too_wide", "| One | Two | Three | Four | Five | Six | Seven | Eight | Nine |\n|---|---|---|---|---|---|---|---|---|\n| a1 | b1 | c1 | d1 | e1 | f1 | g1 | h1 | i1 |\n| a2 has more text | b2 | c2 | d2 | e2 | f2 | g2 | h2 | i2 |"),
            ("link", "See [the docs](https://example.com/docs) and <https://example.com> and [js](javascript:alert(1)) and [a very long link text](https://example.com/a/very/long/path/that/exceeds/width/of/narrow/terminal/rows) done."),
            ("rule", "Above\n\n---\n\nBelow"),
        ]
    }

    /// Renders a sample and joins the row text with newlines.
    /// @param source - the markdown text
    /// @param width - the columns available
    fn joined(source: &str, width: usize) -> String {
        render(source, width, false).iter().map(|r| r.text.as_str()).collect::<Vec<_>>().join("\n")
    }

    /// Checks every sample against its golden file, or rewrites the files when UPDATE_GOLDEN=1.
    #[test]
    fn golden_files_match() {
        let update = std::env::var("UPDATE_GOLDEN").map(|v| v == "1").unwrap_or(false);
        for (name, source) in samples() {
            for width in [80, 40] {
                let actual = joined(source, width);
                let path = format!("{}/tests/golden/markdown/{}_{}.txt", env!("CARGO_MANIFEST_DIR"), name, width);
                if update {
                    std::fs::create_dir_all(format!("{}/tests/golden/markdown", env!("CARGO_MANIFEST_DIR"))).unwrap();
                    std::fs::write(&path, &actual).unwrap();
                    continue;
                }
                let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing golden file {path}"));
                let expected = expected.replace("\r\n", "\n");
                assert_eq!(actual, expected.trim_end_matches('\n'), "{name} at {width}");
            }
        }
    }

    /// No row is wider than the width, with color on or off.
    #[test]
    fn rows_fit_the_width() {
        for (name, source) in samples() {
            for width in [20, 40, 80] {
                for color in [false, true] {
                    for row in render(source, width, color) {
                        assert!(visible_width(&row.text) <= width, "{name} width {width} color {color}: {:?}", row.text);
                    }
                }
            }
        }
    }

    /// A reply with an OSC title sequence produces no ESC or BEL and no visible sequence text.
    #[test]
    fn escape_sequences_are_removed() {
        let source = "text \x1b]0;x\x07 more **bold** and \x1b[31mred\x1b[0m &#27;[2J";
        for row in render(source, 80, false) {
            assert!(!row.text.contains('\x1b') && !row.text.contains('\x07'), "{:?}", row.text);
        }
        for row in render(source, 80, true) {
            assert!(!row.text.contains("]0;"), "{:?}", row.text);
        }
    }

    /// A javascript: link gets no OSC 8 hyperlink, and an https link does.
    #[test]
    fn only_safe_schemes_become_hyperlinks() {
        let bad = render("[x](javascript:alert(1))", 80, true);
        assert!(bad.iter().all(|r| !r.text.contains("\x1b]8;")));
        let good = render("[x](https://example.com)", 80, true);
        assert!(good[0].text.contains("\x1b]8;;https://example.com\x1b\\x"));
    }

    /// Soft wrapped rows of a paragraph are marked as continuations, and the first row is not.
    #[test]
    fn wrapped_rows_are_continuations() {
        let rows = render("word ".repeat(40).trim(), 30, false);
        assert!(rows.len() > 2);
        assert!(!rows[0].cont);
        assert!(rows[1..].iter().all(|r| r.cont));
    }

    /// Code rows start with the two column border as their gutter.
    #[test]
    fn code_rows_have_gutter_two() {
        let rows = render("```\nlet a = 1;\n```", 40, false);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].gutter, 2);
        assert!(rows[0].text.starts_with("│ "));
    }

    /// Nested emphasis keeps both styles in the colored output.
    #[test]
    fn nested_styles_survive() {
        let rows = render("**bold *both* bold**", 80, true);
        assert!(rows[0].text.contains("\x1b[1m\x1b[3mboth"));
    }
}
