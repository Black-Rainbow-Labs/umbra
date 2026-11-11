//! Syntax colors for code blocks in replies. Each language is a table of comment markers, string
//! quotes, keywords, types and literals, and one small lexer reads every language from its table.
//! A state carried from line to line handles block comments, strings over several lines and HTML
//! tags that span lines. The colors are the 16 basic ANSI colors, so they follow the terminal's
//! theme. Unknown languages are not colored.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::utils::screen::char_width;

/// One piece of a code line and the SGR code it is drawn with, or None for the default color.
pub type Span = (String, Option<&'static str>);

const KEYWORD: &str = "35";
const TYPE: &str = "36";
const STRING: &str = "32";
const NUMBER: &str = "33";
const COMMENT: &str = "90;3";
const FUNCTION: &str = "34";
const ADDED: &str = "32";
const REMOVED: &str = "31";
const HUNK: &str = "36";

/// Turns highlighting on or off for the whole run. On by default.
static ENABLED: AtomicBool = AtomicBool::new(true);

/// Sets whether code blocks are colored, from the `syntax_highlight` setting.
/// @param on - true to color code
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
}

/// True when code blocks are colored.
pub fn enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// How a language is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Comments, strings, numbers and words.
    Code,
    /// HTML and XML: tags, attributes and comments.
    Markup,
    /// A unified diff: each line colored by its first character.
    Diff,
}

/// The table one language is read from.
struct LanguageSpec {
    names: &'static [&'static str],
    kind: Kind,
    line_comments: &'static [&'static str],
    block_comment: Option<(&'static str, &'static str)>,
    /// Each quote: what opens it, what closes it, and whether it may run over several lines.
    strings: &'static [(&'static str, &'static str, bool)],
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    literals: &'static [&'static str],
    /// Words that start with a capital letter are types.
    capital_types: bool,
    /// `$name` and `${name}` are variables.
    variables: bool,
    /// A string or word followed by `:` or `=` at the start of a line is a key.
    keys: bool,
    /// Keywords match without regard to case, as in SQL.
    ignore_case: bool,
    /// `#include` style lines, as in C.
    preprocessor: bool,
}

/// The defaults the language tables below start from.
const BASE: LanguageSpec = LanguageSpec { names: &[], kind: Kind::Code, line_comments: &[], block_comment: None, strings: &[("\"", "\"", false), ("'", "'", false)], keywords: &[], types: &[], literals: &["true", "false"], capital_types: false, variables: false, keys: false, ignore_case: false, preprocessor: false };

const C_COMMENTS: Option<(&str, &str)> = Some(("/*", "*/"));

const RUST: LanguageSpec = LanguageSpec { names: &["rust", "rs"], line_comments: &["//"], block_comment: C_COMMENTS, strings: &[("\"", "\"", true), ("'", "'", false)], keywords: &["as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "type", "unsafe", "use", "where", "while", "macro_rules"], types: &["i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64", "bool", "char", "str"], literals: &["true", "false", "None", "Some", "Ok", "Err"], capital_types: true, ..BASE };

const PYTHON: LanguageSpec = LanguageSpec { names: &["python", "py", "python3"], line_comments: &["#"], strings: &[("\"\"\"", "\"\"\"", true), ("'''", "'''", true), ("\"", "\"", false), ("'", "'", false)], keywords: &["and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield", "match", "case", "self"], types: &["int", "float", "str", "bool", "list", "dict", "set", "tuple", "bytes", "object"], literals: &["True", "False", "None"], capital_types: true, ..BASE };

const JS_KEYWORDS: &[&str] = &["async", "await", "break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete", "do", "else", "export", "extends", "finally", "for", "from", "function", "if", "import", "in", "instanceof", "let", "new", "of", "return", "static", "super", "switch", "this", "throw", "try", "typeof", "var", "void", "while", "with", "yield", "as", "interface", "type", "enum", "implements", "private", "protected", "public", "readonly", "declare", "namespace", "abstract", "keyof"];

const JAVASCRIPT: LanguageSpec = LanguageSpec { names: &["javascript", "js", "jsx", "mjs", "cjs", "typescript", "ts", "tsx", "mts"], line_comments: &["//"], block_comment: C_COMMENTS, strings: &[("`", "`", true), ("\"", "\"", false), ("'", "'", false)], keywords: JS_KEYWORDS, types: &["string", "number", "boolean", "any", "unknown", "never", "void", "object", "bigint", "symbol"], literals: &["true", "false", "null", "undefined", "NaN"], capital_types: true, ..BASE };

const GO: LanguageSpec = LanguageSpec { names: &["go", "golang"], line_comments: &["//"], block_comment: C_COMMENTS, strings: &[("`", "`", true), ("\"", "\"", false), ("'", "'", false)], keywords: &["break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for", "func", "go", "goto", "if", "import", "interface", "map", "package", "range", "return", "select", "struct", "switch", "type", "var"], types: &["bool", "byte", "complex64", "complex128", "error", "float32", "float64", "int", "int8", "int16", "int32", "int64", "rune", "string", "uint", "uint8", "uint16", "uint32", "uint64", "uintptr", "any"], literals: &["true", "false", "nil", "iota"], capital_types: true, ..BASE };

const C_KEYWORDS: &[&str] = &["auto", "break", "case", "const", "continue", "default", "do", "else", "enum", "extern", "for", "goto", "if", "inline", "register", "restrict", "return", "sizeof", "static", "struct", "switch", "typedef", "union", "volatile", "while", "class", "namespace", "template", "typename", "public", "private", "protected", "virtual", "override", "new", "delete", "using", "try", "catch", "throw", "constexpr", "noexcept", "operator", "this", "auto"];

const C: LanguageSpec = LanguageSpec { names: &["c", "h", "cpp", "c++", "cc", "hpp", "cxx", "objc"], line_comments: &["//"], block_comment: C_COMMENTS, keywords: C_KEYWORDS, types: &["void", "char", "short", "int", "long", "float", "double", "signed", "unsigned", "bool", "size_t", "int8_t", "int16_t", "int32_t", "int64_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "std", "string", "vector"], literals: &["true", "false", "NULL", "nullptr"], preprocessor: true, ..BASE };

const JAVA_LIKE_KEYWORDS: &[&str] = &["abstract", "as", "async", "await", "base", "break", "case", "catch", "class", "const", "continue", "data", "default", "do", "else", "enum", "extends", "final", "finally", "for", "foreach", "fun", "func", "guard", "if", "implements", "import", "in", "interface", "internal", "is", "let", "namespace", "new", "object", "open", "out", "override", "package", "private", "protected", "public", "readonly", "return", "sealed", "static", "struct", "super", "switch", "this", "throw", "throws", "try", "using", "val", "var", "virtual", "void", "when", "where", "while", "protocol", "extension", "self", "init", "deinit", "inout", "mutating", "lazy", "weak", "record", "yield"];

const JAVA_LIKE: LanguageSpec = LanguageSpec { names: &["java", "kotlin", "kt", "kts", "csharp", "cs", "c#", "swift", "scala", "dart"], line_comments: &["//"], block_comment: C_COMMENTS, strings: &[("\"\"\"", "\"\"\"", true), ("\"", "\"", false), ("'", "'", false)], keywords: JAVA_LIKE_KEYWORDS, types: &["int", "long", "short", "byte", "char", "float", "double", "boolean", "bool", "string", "decimal", "object", "dynamic", "uint", "ulong"], literals: &["true", "false", "null", "nil"], capital_types: true, ..BASE };

const RUBY: LanguageSpec = LanguageSpec { names: &["ruby", "rb"], line_comments: &["#"], keywords: &["alias", "and", "begin", "break", "case", "class", "def", "defined?", "do", "else", "elsif", "end", "ensure", "for", "if", "in", "module", "next", "not", "or", "redo", "rescue", "retry", "return", "self", "super", "then", "undef", "unless", "until", "when", "while", "yield", "require", "attr_accessor", "attr_reader"], literals: &["true", "false", "nil"], capital_types: true, ..BASE };

const PHP: LanguageSpec = LanguageSpec { names: &["php"], line_comments: &["//", "#"], block_comment: C_COMMENTS, keywords: &["abstract", "as", "break", "case", "catch", "class", "const", "continue", "declare", "default", "do", "echo", "else", "elseif", "extends", "final", "finally", "fn", "for", "foreach", "function", "global", "if", "implements", "include", "interface", "match", "namespace", "new", "private", "protected", "public", "require", "return", "static", "switch", "throw", "trait", "try", "use", "while", "yield"], literals: &["true", "false", "null"], capital_types: true, variables: true, ..BASE };

const SHELL: LanguageSpec = LanguageSpec { names: &["bash", "sh", "shell", "zsh", "console", "shellsession", "fish", "ksh"], line_comments: &["#"], strings: &[("\"", "\"", true), ("'", "'", true)], keywords: &["if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac", "in", "function", "return", "local", "export", "readonly", "set", "unset", "source", "exit", "echo", "cd", "shift", "trap", "eval", "exec"], literals: &["true", "false"], variables: true, ..BASE };

const POWERSHELL: LanguageSpec = LanguageSpec { names: &["powershell", "ps1", "pwsh", "ps"], line_comments: &["#"], block_comment: Some(("<#", "#>")), keywords: &["begin", "break", "catch", "class", "continue", "do", "else", "elseif", "end", "exit", "filter", "finally", "for", "foreach", "function", "if", "in", "param", "process", "return", "switch", "throw", "trap", "try", "until", "while"], literals: &["$true", "$false", "$null"], variables: true, ignore_case: true, ..BASE };

const SQL: LanguageSpec = LanguageSpec { names: &["sql", "postgres", "postgresql", "mysql", "sqlite", "plsql"], line_comments: &["--"], block_comment: C_COMMENTS, keywords: &["select", "from", "where", "and", "or", "not", "insert", "into", "values", "update", "set", "delete", "create", "table", "index", "view", "drop", "alter", "add", "column", "primary", "key", "foreign", "references", "join", "left", "right", "inner", "outer", "full", "on", "group", "by", "order", "having", "limit", "offset", "as", "distinct", "union", "all", "case", "when", "then", "else", "end", "is", "in", "exists", "between", "like", "default", "unique", "constraint", "begin", "commit", "rollback", "with", "returning", "asc", "desc", "if"], types: &["int", "integer", "bigint", "smallint", "text", "varchar", "char", "boolean", "real", "float", "double", "numeric", "decimal", "date", "timestamp", "timestamptz", "json", "jsonb", "uuid", "blob", "serial"], literals: &["true", "false", "null"], ignore_case: true, ..BASE };

const JSON: LanguageSpec = LanguageSpec { names: &["json", "jsonc", "json5", "jsonl"], line_comments: &["//"], block_comment: C_COMMENTS, strings: &[("\"", "\"", false)], literals: &["true", "false", "null"], keys: true, ..BASE };

const TOML: LanguageSpec = LanguageSpec { names: &["toml", "ini", "cfg", "conf", "properties", "env", "dotenv"], line_comments: &["#", ";"], strings: &[("\"\"\"", "\"\"\"", true), ("'''", "'''", true), ("\"", "\"", false), ("'", "'", false)], literals: &["true", "false"], keys: true, ..BASE };

const YAML: LanguageSpec = LanguageSpec { names: &["yaml", "yml"], line_comments: &["#"], literals: &["true", "false", "null", "yes", "no", "on", "off", "~"], keys: true, ..BASE };

const CSS: LanguageSpec = LanguageSpec { names: &["css", "scss", "sass", "less"], line_comments: &["//"], block_comment: C_COMMENTS, keywords: &["@media", "@import", "@keyframes", "@font-face", "@supports", "!important", "@use", "@mixin", "@include"], literals: &[], keys: true, ..BASE };

const LUA: LanguageSpec = LanguageSpec { names: &["lua"], line_comments: &["--"], block_comment: Some(("--[[", "]]")), keywords: &["and", "break", "do", "else", "elseif", "end", "for", "function", "goto", "if", "in", "local", "not", "or", "repeat", "return", "then", "until", "while"], literals: &["true", "false", "nil"], ..BASE };

const MARKUP: LanguageSpec = LanguageSpec { names: &["html", "htm", "xml", "svg", "xhtml", "vue", "plist"], kind: Kind::Markup, block_comment: Some(("<!--", "-->")), ..BASE };

const DIFF: LanguageSpec = LanguageSpec { names: &["diff", "patch", "udiff"], kind: Kind::Diff, ..BASE };

/// Every language the highlighter reads.
const LANGUAGES: &[&LanguageSpec] = &[&RUST, &PYTHON, &JAVASCRIPT, &GO, &C, &JAVA_LIKE, &RUBY, &PHP, &SHELL, &POWERSHELL, &SQL, &JSON, &TOML, &YAML, &CSS, &LUA, &MARKUP, &DIFF];

/// What the lexer is in the middle of when a line ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Carry {
    #[default]
    Nothing,
    BlockComment,
    /// A string that runs over lines, by its position in the language's string table.
    String(usize),
    /// Inside an HTML tag, between its name and its `>`.
    Tag,
}

/// The language table for a fence name, ignoring case.
/// @param name - the first word of the fence's info string
fn language(name: &str) -> Option<&'static LanguageSpec> {
    let name = name.trim().to_lowercase();
    LANGUAGES.iter().copied().find(|l| l.names.contains(&name.as_str()))
}

/// Colors a whole code block, one list of spans per line. Returns None when the language is not
/// known, so the caller draws the code as it is.
/// @param code - the code, lines separated by `\n`
/// @param name - the fence name, such as `rust`
pub fn highlight(code: &str, name: &str) -> Option<Vec<Vec<Span>>> {
    let spec = language(name)?;
    let mut carry = Carry::Nothing;
    let mut lines = Vec::new();
    for line in code.split('\n') {
        let (spans, next) = match spec.kind {
            Kind::Diff => (diff_line(line), Carry::Nothing),
            Kind::Markup => markup_line(spec, line, carry),
            Kind::Code => code_line(spec, line, carry),
        };
        carry = next;
        lines.push(merge(spans));
    }
    Some(lines)
}

/// Colors one diff line by its first character.
/// @param line - the line
fn diff_line(line: &str) -> Vec<Span> {
    let code = if line.starts_with("+++") || line.starts_with("---") {
        Some("1")
    } else if line.starts_with('+') {
        Some(ADDED)
    } else if line.starts_with('-') {
        Some(REMOVED)
    } else if line.starts_with("@@") {
        Some(HUNK)
    } else {
        None
    };
    vec![(line.to_string(), code)]
}

/// The lexer's position in one line.
struct Cursor<'a> {
    chars: Vec<char>,
    at: usize,
    spans: Vec<Span>,
    spec: &'a LanguageSpec,
}

impl Cursor<'_> {
    /// True when the text at the position starts with `prefix`.
    /// @param prefix - the text to look for
    fn starts_with(&self, prefix: &str) -> bool {
        (self.at..).zip(prefix.chars()).all(|(i, p)| self.chars.get(i) == Some(&p))
    }

    /// The position where `close` ends, searching from the position, or None.
    /// @param close - the closing text
    /// @param escapes - true when a backslash escapes the next character
    fn find_close(&self, close: &str, escapes: bool) -> Option<usize> {
        let close: Vec<char> = close.chars().collect();
        let mut i = self.at;
        while i < self.chars.len() {
            if escapes && self.chars[i] == '\\' {
                i += 2;
                continue;
            }
            if self.chars[i..].starts_with(&close) {
                return Some(i + close.len());
            }
            i += 1;
        }
        None
    }

    /// Adds the characters up to `end` as one span and moves there.
    /// @param end - the position after the span
    /// @param code - its color
    fn take(&mut self, end: usize, code: Option<&'static str>) {
        let end = end.min(self.chars.len()).max(self.at);
        let text: String = self.chars[self.at..end].iter().collect();
        self.spans.push((text, code));
        self.at = end;
    }

    /// The next character that is not a space, from a position.
    /// @param from - where to start
    fn next_visible(&self, from: usize) -> Option<char> {
        self.chars[from.min(self.chars.len())..].iter().copied().find(|c| !c.is_whitespace())
    }

    /// True when nothing but spaces comes before the position on this line.
    fn at_line_start(&self) -> bool {
        self.chars[..self.at].iter().all(|c| c.is_whitespace())
    }
}

/// Colors one line of a code language, starting in the state the line above ended in.
/// @param spec - the language
/// @param line - the line
/// @param carry - the state from the line above
fn code_line(spec: &LanguageSpec, line: &str, carry: Carry) -> (Vec<Span>, Carry) {
    let mut cur = Cursor { chars: line.chars().collect(), at: 0, spans: Vec::new(), spec };
    let mut carry = resume(&mut cur, carry);
    if carry != Carry::Nothing {
        return (cur.spans, carry);
    }
    if spec.preprocessor && cur.chars.iter().find(|c| !c.is_whitespace()) == Some(&'#') {
        let end = cur.chars.iter().position(|c| *c == '<' || *c == '"').unwrap_or(cur.chars.len());
        cur.take(end, Some(KEYWORD));
    }
    while cur.at < cur.chars.len() {
        if let Some(next) = comment_or_string(&mut cur) {
            carry = next;
            if carry != Carry::Nothing {
                break;
            }
            continue;
        }
        word_or_symbol(&mut cur);
    }
    (cur.spans, carry)
}

/// Finishes a block comment or a string carried from the line above. Returns the state at the
/// end of what it read: Nothing when it closed on this line.
/// @param cur - the line
/// @param carry - the state from the line above
fn resume(cur: &mut Cursor, carry: Carry) -> Carry {
    match carry {
        Carry::BlockComment => {
            let close = cur.spec.block_comment.map_or("*/", |b| b.1);
            match cur.find_close(close, false) {
                Some(end) => cur.take(end, Some(COMMENT)),
                None => {
                    cur.take(cur.chars.len(), Some(COMMENT));
                    return Carry::BlockComment;
                }
            }
        }
        Carry::String(index) => {
            let (_, close, _) = cur.spec.strings[index];
            match cur.find_close(close, true) {
                Some(end) => cur.take(end, Some(STRING)),
                None => {
                    cur.take(cur.chars.len(), Some(STRING));
                    return Carry::String(index);
                }
            }
        }
        Carry::Nothing | Carry::Tag => {}
    }
    Carry::Nothing
}

/// Reads a comment or a string at the position. Returns None when there is neither, else the
/// state at the end of what it read.
/// @param cur - the line
fn comment_or_string(cur: &mut Cursor) -> Option<Carry> {
    let spec = cur.spec;
    if let Some((open, close)) = spec.block_comment {
        if cur.starts_with(open) {
            let from = cur.at + open.chars().count();
            let probe = Cursor { chars: cur.chars.clone(), at: from, spans: Vec::new(), spec };
            return Some(match probe.find_close(close, false) {
                Some(end) => {
                    cur.take(end, Some(COMMENT));
                    Carry::Nothing
                }
                None => {
                    cur.take(cur.chars.len(), Some(COMMENT));
                    Carry::BlockComment
                }
            });
        }
    }
    let comment_here = spec.line_comments.iter().any(|m| cur.starts_with(m) && (*m != "#" || cur.at == 0 || cur.chars[cur.at - 1].is_whitespace()));
    if comment_here {
        cur.take(cur.chars.len(), Some(COMMENT));
        return Some(Carry::Nothing);
    }
    let (index, &(open, close, multiline)) = spec.strings.iter().enumerate().find(|(_, (open, _, _))| cur.starts_with(open))?;
    if spec.names.contains(&"rust") && open == "'" && !is_rust_char(cur) {
        return None;
    }
    Some(read_string(cur, open, close, multiline, index))
}

/// Reads a string that opens at the position.
/// @param cur - the line
/// @param open - the opening quote
/// @param close - the closing quote
/// @param multiline - true when the string may run onto the next line
/// @param index - the quote's position in the language's string table
fn read_string(cur: &mut Cursor, open: &str, close: &str, multiline: bool, index: usize) -> Carry {
    let start = cur.at;
    cur.at += open.chars().count();
    let end = cur.find_close(close, true);
    cur.at = start;
    match end {
        Some(end) => {
            let key = cur.spec.keys && cur.at_line_start_or_after_brace() && cur.next_visible(end) == Some(':');
            cur.take(end, Some(if key { TYPE } else { STRING }));
            Carry::Nothing
        }
        None => {
            cur.take(cur.chars.len(), Some(STRING));
            if multiline { Carry::String(index) } else { Carry::Nothing }
        }
    }
}

impl Cursor<'_> {
    /// True when the position is the first thing on the line, or follows `{` or `,`, where a JSON
    /// key can start.
    fn at_line_start_or_after_brace(&self) -> bool {
        let before = self.chars[..self.at].iter().rev().find(|c| !c.is_whitespace());
        matches!(before, None | Some('{') | Some(','))
    }
}

/// True when a `'` in Rust opens a character literal, not a lifetime such as `'a`.
/// @param cur - the line, at the quote
fn is_rust_char(cur: &Cursor) -> bool {
    matches!(&cur.chars[cur.at + 1..], ['\\', ..] | [_, '\'', ..])
}

/// Reads a number, a word or one other character at the position.
/// @param cur - the line
fn word_or_symbol(cur: &mut Cursor) {
    let c = cur.chars[cur.at];
    let after_word = cur.at > 0 && is_word_char(cur.chars[cur.at - 1]);
    if c.is_ascii_digit() && !after_word {
        let end = scan(cur, cur.at, |c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
        cur.take(end, Some(NUMBER));
    } else if cur.spec.variables && c == '$' {
        read_variable(cur);
    } else if is_word_start(c, cur.spec) {
        read_word(cur);
    } else {
        cur.take(cur.at + 1, None);
    }
}

/// Reads `$name`, `${name}` or `$1` as a variable.
/// @param cur - the line, at the `$`
fn read_variable(cur: &mut Cursor) {
    let end = if cur.chars.get(cur.at + 1) == Some(&'{') {
        cur.chars[cur.at..].iter().position(|c| *c == '}').map_or(cur.chars.len(), |p| cur.at + p + 1)
    } else {
        scan(cur, cur.at + 1, is_word_char)
    };
    if end == cur.at + 1 {
        cur.take(end, None);
        return;
    }
    let word: String = cur.chars[cur.at..end].iter().collect();
    let code = if cur.spec.literals.iter().any(|l| l.eq_ignore_ascii_case(&word)) { NUMBER } else { TYPE };
    cur.take(end, Some(code));
}

/// Reads a word and colors it as a keyword, a literal, a type, a key, a function name or plain.
/// @param cur - the line, at the word's first character
fn read_word(cur: &mut Cursor) {
    let spec = cur.spec;
    let end = scan(cur, cur.at + 1, |c| is_word_char(c) || (spec.kind == Kind::Code && spec.keys && c == '-') || c == '?' && spec.names.contains(&"ruby"));
    let word: String = cur.chars[cur.at..end].iter().collect();
    let next = cur.next_visible(end);
    let code = if spec.keys && cur.at_line_start() && matches!(next, Some(':') | Some('=')) {
        Some(TYPE)
    } else if matches_word(spec.keywords, &word, spec.ignore_case) {
        Some(KEYWORD)
    } else if matches_word(spec.literals, &word, spec.ignore_case) {
        Some(NUMBER)
    } else if matches_word(spec.types, &word, spec.ignore_case) || (spec.capital_types && word.starts_with(|c: char| c.is_ascii_uppercase())) {
        Some(TYPE)
    } else if next == Some('(') || (cur.chars.get(end) == Some(&'!') && spec.names.contains(&"rust")) {
        Some(FUNCTION)
    } else {
        None
    };
    cur.take(end, code);
}

/// True when a word is in a list.
/// @param list - the words
/// @param word - the word
/// @param ignore_case - compare without regard to case
fn matches_word(list: &[&str], word: &str, ignore_case: bool) -> bool {
    list.iter().any(|w| if ignore_case { w.eq_ignore_ascii_case(word) } else { *w == word })
}

/// The position after the run of characters that pass `keep`, from `from`.
/// @param cur - the line
/// @param from - where to start
/// @param keep - which characters belong to the run
fn scan(cur: &Cursor, from: usize, keep: impl Fn(char) -> bool) -> usize {
    let mut i = from;
    while i < cur.chars.len() && keep(cur.chars[i]) {
        i += 1;
    }
    i
}

/// True for a character inside a word.
/// @param c - the character
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// True for a character that starts a word: a letter, `_`, and `@` or `!` where the language's
/// keywords use them.
/// @param c - the character
/// @param spec - the language
fn is_word_start(c: char, spec: &LanguageSpec) -> bool {
    c.is_alphabetic() || c == '_' || ((c == '@' || c == '!') && spec.keywords.iter().any(|k| k.starts_with(c)))
}

/// Colors one line of HTML or XML: comments, tag names, attribute names and attribute values.
/// @param spec - the markup language
/// @param line - the line
/// @param carry - the state from the line above
fn markup_line(spec: &LanguageSpec, line: &str, carry: Carry) -> (Vec<Span>, Carry) {
    let mut cur = Cursor { chars: line.chars().collect(), at: 0, spans: Vec::new(), spec };
    let mut carry = resume(&mut cur, carry);
    while cur.at < cur.chars.len() {
        if carry == Carry::Tag {
            carry = inside_tag(&mut cur);
            continue;
        }
        if cur.starts_with("<!--") {
            if comment_or_string(&mut cur) == Some(Carry::BlockComment) {
                return (cur.spans, Carry::BlockComment);
            }
            continue;
        }
        let opens_tag = cur.chars[cur.at] == '<' && cur.chars.get(cur.at + 1).is_some_and(|c| c.is_alphabetic() || *c == '/' || *c == '!' || *c == '?');
        if opens_tag {
            let name_start = scan(&cur, cur.at + 1, |c| c == '/' || c == '!' || c == '?');
            cur.take(name_start, None);
            let name_end = scan(&cur, cur.at, |c| is_word_char(c) || c == '-' || c == ':' || c == '.');
            cur.take(name_end, Some(KEYWORD));
            carry = Carry::Tag;
        } else {
            let end = scan(&cur, cur.at + 1, |c| c != '<');
            cur.take(end, None);
        }
    }
    (cur.spans, carry)
}

/// Reads inside a tag, up to and including its `>`: attribute names and quoted values.
/// @param cur - the line
fn inside_tag(cur: &mut Cursor) -> Carry {
    while cur.at < cur.chars.len() {
        let c = cur.chars[cur.at];
        if c == '>' {
            cur.take(cur.at + 1, None);
            return Carry::Nothing;
        }
        if c == '"' || c == '\'' {
            let close = c.to_string();
            cur.at += 1;
            let end = cur.find_close(&close, false).unwrap_or(cur.chars.len());
            cur.at -= 1;
            cur.take(end, Some(STRING));
        } else if is_word_char(c) || c == '-' || c == ':' || c == '@' {
            let end = scan(cur, cur.at, |c| is_word_char(c) || c == '-' || c == ':' || c == '@' || c == '.');
            cur.take(end, Some(TYPE));
        } else {
            cur.take(cur.at + 1, None);
        }
    }
    Carry::Tag
}

/// Joins neighboring spans of the same color, so a painted row has fewer codes.
/// @param spans - the spans
fn merge(spans: Vec<Span>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    for (text, code) in spans {
        if text.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.1 == code || text.trim().is_empty() && last.1.is_none() => last.0.push_str(&text),
            _ => out.push((text, code)),
        }
    }
    out
}

/// Paints spans into one string, each colored span closed with a reset.
/// @param spans - the spans
pub fn paint(spans: &[Span]) -> String {
    let mut out = String::new();
    for (text, code) in spans {
        match code {
            Some(code) => out.push_str(&format!("\x1b[{code}m{text}\x1b[0m")),
            None => out.push_str(text),
        }
    }
    out
}

/// Cuts spans into rows by column width, keeping each piece's color. The first row may be a
/// different width than the rest.
/// @param spans - the spans of one line
/// @param first - the width of the first row
/// @param rest - the width of every later row
pub fn wrap(spans: &[Span], first: usize, rest: usize) -> Vec<Vec<Span>> {
    let mut rows: Vec<Vec<Span>> = vec![Vec::new()];
    let (mut used, mut limit) = (0, first.max(1));
    for (text, code) in spans {
        let mut piece = String::new();
        for c in text.chars() {
            let w = char_width(c);
            if used + w > limit && used > 0 {
                if !piece.is_empty() {
                    rows.last_mut().expect("a row").push((std::mem::take(&mut piece), *code));
                }
                rows.push(Vec::new());
                used = 0;
                limit = rest.max(1);
            }
            piece.push(c);
            used += w;
        }
        if !piece.is_empty() {
            rows.last_mut().expect("a row").push((piece, *code));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The color of the first span whose text is `word`.
    fn color_of(lines: &[Vec<Span>], word: &str) -> Option<&'static str> {
        lines.iter().flatten().find(|(t, _)| t.trim() == word).and_then(|(_, c)| *c)
    }

    #[test]
    fn rust_keywords_types_strings_numbers_and_comments() {
        let lines = highlight("pub fn main() -> Result<u32> {\n    let s = \"hi\"; // greet\n    println!(\"{}\", 42);\n}", "rust").unwrap();
        assert_eq!(color_of(&lines, "pub"), Some(KEYWORD));
        assert_eq!(color_of(&lines, "main"), Some(FUNCTION));
        assert_eq!(color_of(&lines, "Result"), Some(TYPE));
        assert_eq!(color_of(&lines, "u32"), Some(TYPE));
        assert_eq!(color_of(&lines, "\"hi\""), Some(STRING));
        assert_eq!(color_of(&lines, "// greet"), Some(COMMENT));
        assert_eq!(color_of(&lines, "println"), Some(FUNCTION));
        assert_eq!(color_of(&lines, "42"), Some(NUMBER));
    }

    #[test]
    fn a_rust_lifetime_is_not_a_string() {
        let lines = highlight("fn f<'a>(x: &'a str) -> char { 'x' }", "rs").unwrap();
        assert_eq!(color_of(&lines, "'x'"), Some(STRING));
        assert!(lines[0].iter().all(|(t, c)| !(t.contains("'a") && *c == Some(STRING))), "{:?}", lines[0]);
    }

    #[test]
    fn a_block_comment_and_a_python_docstring_run_over_lines() {
        let lines = highlight("/* one\ntwo */ int x = 1;", "c").unwrap();
        assert_eq!(lines[1][0], ("two */".to_string(), Some(COMMENT)));
        assert_eq!(color_of(&lines, "int"), Some(TYPE));
        let lines = highlight("def f():\n    \"\"\"Doc\n    more\"\"\"\n    return None", "python").unwrap();
        assert_eq!(lines[2][0].1, Some(STRING));
        assert_eq!(color_of(&lines, "return"), Some(KEYWORD));
        assert_eq!(color_of(&lines, "None"), Some(NUMBER));
    }

    #[test]
    fn typescript_shell_json_and_sql() {
        let lines = highlight("const x: string = `a ${b}`;", "ts").unwrap();
        assert_eq!(color_of(&lines, "const"), Some(KEYWORD));
        assert_eq!(color_of(&lines, "string"), Some(TYPE));
        let lines = highlight("if [ -n \"$HOME\" ]; then echo $USER # note\nfi", "bash").unwrap();
        assert_eq!(color_of(&lines, "if"), Some(KEYWORD));
        assert_eq!(color_of(&lines, "$USER"), Some(TYPE));
        assert_eq!(color_of(&lines, "# note"), Some(COMMENT));
        let lines = highlight("{\"name\": \"umbra\", \"n\": 3, \"ok\": true}", "json").unwrap();
        assert_eq!(color_of(&lines, "\"name\""), Some(TYPE));
        assert_eq!(color_of(&lines, "\"umbra\""), Some(STRING));
        assert_eq!(color_of(&lines, "true"), Some(NUMBER));
        let lines = highlight("SELECT id FROM users WHERE id = 1", "sql").unwrap();
        assert_eq!(color_of(&lines, "SELECT"), Some(KEYWORD));
    }

    #[test]
    fn html_diff_and_unknown_languages() {
        let lines = highlight("<a href=\"/x\">link</a>", "html").unwrap();
        assert_eq!(color_of(&lines, "a"), Some(KEYWORD));
        assert_eq!(color_of(&lines, "href"), Some(TYPE));
        assert_eq!(color_of(&lines, "\"/x\""), Some(STRING));
        let lines = highlight("@@ -1 +1 @@\n-old\n+new", "diff").unwrap();
        assert_eq!(lines[1][0].1, Some(REMOVED));
        assert_eq!(lines[2][0].1, Some(ADDED));
        assert!(highlight("x", "brainfudge").is_none());
    }

    #[test]
    fn wrapping_keeps_colors_and_widths() {
        let spans = vec![("abcd".to_string(), Some(KEYWORD)), ("efgh".to_string(), None)];
        let rows = wrap(&spans, 3, 2);
        let texts: Vec<String> = rows.iter().map(|r| r.iter().map(|s| s.0.clone()).collect()).collect();
        assert_eq!(texts, vec!["abc", "de", "fg", "h"]);
        assert_eq!(rows[1], vec![("d".to_string(), Some(KEYWORD)), ("e".to_string(), None)]);
    }
}
