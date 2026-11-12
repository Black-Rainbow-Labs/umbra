//! The `edit` tool (task-2617, E5): replaces an exact piece of text in a file and leaves every
//! other byte as it was. The file keeps its line endings, its encoding (UTF-8, UTF-8 with a byte
//! order mark, UTF-16 with a byte order mark) and its final newline. Text the model sends with
//! LF line endings is matched and written as CRLF in a CRLF file.
//!
//! It is offered as a built in tool of the `umbra` server, like `tool_search`, so the parsers,
//! the confirmation prompt and the action display treat it like an MCP tool.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::providers::ToolSpec;
use super::tool_search_service::BUILTIN_SERVER;
use crate::models::action::Observation;
use crate::models::context::McpTool;

/// The tool's name.
pub const EDIT_TOOL_NAME: &str = "edit";

/// How a file's text was stored, so it can be written back the same way.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

/// The `edit` tool as an MCP tool of the `umbra` server.
pub fn edit_tool() -> McpTool {
    McpTool { server: BUILTIN_SERVER.into(), name: EDIT_TOOL_NAME.into(), description: String::new(), input_schema: json!({}) }
}

/// Returns true for a call to the `edit` tool.
/// @param server - the action's server
/// @param tool - the action's tool name
pub fn is_edit(server: &str, tool: &str) -> bool {
    server == BUILTIN_SERVER && tool == EDIT_TOOL_NAME
}

/// The `edit` tool definition sent to the model.
pub fn edit_spec() -> ToolSpec {
    ToolSpec {
        name: EDIT_TOOL_NAME.into(),
        description: "Replace text in an existing file. old_text must match the file exactly, including indentation, and match only once unless replace_all is true. The file keeps its line endings and encoding, and nothing outside the match changes. Use this for changes to existing files; create new files with bash.".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The file, relative to the working directory or absolute" },
                "old_text": { "type": "string", "description": "The exact text to replace" },
                "new_text": { "type": "string", "description": "The text to put in its place" },
                "replace_all": { "type": "boolean", "description": "Replace every match instead of requiring exactly one" }
            },
            "required": ["path", "old_text", "new_text"]
        }),
    }
}

/// Runs one edit and returns what the model sees.
/// @param cwd - the working directory, for relative paths
/// @param arguments - the tool call arguments
pub fn run_edit(cwd: &Path, arguments: &Value) -> Observation {
    let text_arg = |key: &str| arguments.get(key).and_then(Value::as_str);
    let (Some(path), Some(old_text), Some(new_text)) = (text_arg("path"), text_arg("old_text"), text_arg("new_text")) else {
        return Observation::failed("", "edit needs path, old_text and new_text, all strings");
    };
    let replace_all = arguments.get("replace_all").and_then(Value::as_bool).unwrap_or(false);
    match edit_file(&resolve_path(cwd, path), old_text, new_text, replace_all) {
        Ok(report) => Observation { output: report, returncode: 0, ..Default::default() },
        Err(error) => Observation::failed("", error),
    }
}

/// Resolves the path the model gave: relative to the working directory, and on Windows a Git
/// Bash path such as `/c/work/a.txt` is read as `C:/work/a.txt`.
/// @param cwd - the working directory
/// @param path - the path from the tool call
fn resolve_path(cwd: &Path, path: &str) -> PathBuf {
    let bytes = path.as_bytes();
    if cfg!(windows) && bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b'/' {
        return PathBuf::from(format!("{}:{}", (bytes[1] as char).to_ascii_uppercase(), &path[2..]));
    }
    let given = Path::new(path);
    if given.is_absolute() { given.to_path_buf() } else { cwd.join(given) }
}

/// Edits one file and returns a one line report with the changed line numbers.
/// @param file - the file
/// @param old_text - the text to find
/// @param new_text - the replacement
/// @param replace_all - replace every match
fn edit_file(file: &Path, old_text: &str, new_text: &str, replace_all: bool) -> Result<String, String> {
    if old_text.is_empty() {
        return Err("old_text is empty. To create a file, use bash.".into());
    }
    let bytes = std::fs::read(file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    let (encoding, text) = decode(&bytes).ok_or_else(|| format!("{} is not UTF-8 or UTF-16 text, so it cannot be edited with this tool", file.display()))?;
    let crlf = uses_crlf(&text);
    let (old, new) = matched_forms(&text, old_text, new_text, crlf);
    let starts: Vec<usize> = text.match_indices(old.as_str()).map(|(i, _)| i).collect();
    if starts.is_empty() {
        return Err(no_match_error(&text, old_text, file));
    }
    if starts.len() > 1 && !replace_all {
        let lines: Vec<String> = starts.iter().map(|&i| line_of(&text, i).to_string()).collect();
        return Err(format!("old_text matches {} times in {} (at lines {}). Add surrounding lines to old_text so it matches once, or set replace_all to true.", starts.len(), file.display(), lines.join(", ")));
    }
    let edited = if replace_all { text.replace(old.as_str(), &new) } else { text.replacen(old.as_str(), &new, 1) };
    std::fs::write(file, encode(&edited, encoding)).map_err(|e| format!("cannot write {}: {e}", file.display()))?;
    let first = line_of(&text, starts[0]);
    let added = new.matches('\n').count();
    Ok(format!("Edited {}: replaced {} {} at line {}{}.", file.display(), starts.len(), if starts.len() == 1 { "match" } else { "matches" }, first, if added > 0 { format!(", the new text spans {} lines", added + 1) } else { String::new() }))
}

/// Picks the forms of the search and replacement text to use. In a CRLF file, text sent with LF
/// line endings is turned into CRLF, so it matches and the added lines end like the others.
/// @param text - the file's text
/// @param old_text - the text the model sent to find
/// @param new_text - the replacement the model sent
/// @param crlf - true when the file uses CRLF
fn matched_forms(text: &str, old_text: &str, new_text: &str, crlf: bool) -> (String, String) {
    if !crlf {
        return (old_text.to_string(), new_text.to_string());
    }
    let to_crlf = |s: &str| s.replace("\r\n", "\n").replace('\n', "\r\n");
    let old = if text.contains(old_text) { old_text.to_string() } else { to_crlf(old_text) };
    (old, to_crlf(new_text))
}

/// Returns true when most line breaks in the text are CRLF.
/// @param text - the file's text
fn uses_crlf(text: &str) -> bool {
    let crlf = text.matches("\r\n").count();
    crlf > 0 && crlf * 2 >= text.matches('\n').count()
}

/// Returns the 1 based line number of a byte offset.
/// @param text - the text
/// @param offset - the byte offset
fn line_of(text: &str, offset: usize) -> usize {
    text[..offset].matches('\n').count() + 1
}

/// Builds the error for text that is not in the file: the line most like the first line of
/// `old_text`, with its number, so the model can copy the real text.
/// @param text - the file's text
/// @param old_text - what the model looked for
/// @param file - the file
fn no_match_error(text: &str, old_text: &str, file: &Path) -> String {
    let wanted = old_text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let best = text.lines().enumerate().map(|(i, line)| (similarity(line.trim(), wanted), i, line)).filter(|(score, _, _)| *score > 0.3).max_by(|a, b| a.0.total_cmp(&b.0));
    match best {
        Some((_, index, line)) => format!("old_text was not found in {}. The closest line is line {}: {:?}. Read the file around that line and copy the text exactly, including indentation.", file.display(), index + 1, line.trim_end_matches('\r')),
        None => format!("old_text was not found in {}, and no line is like its first line. Read the file and copy the text exactly.", file.display()),
    }
}

/// A rough 0 to 1 likeness of two lines: the share of the shorter line's words found in the other.
/// @param a - one line
/// @param b - the other line
fn similarity(a: &str, b: &str) -> f64 {
    let words_b: Vec<&str> = b.split_whitespace().collect();
    let words_a: Vec<&str> = a.split_whitespace().collect();
    let shorter = words_a.len().min(words_b.len());
    if shorter == 0 {
        return 0.0;
    }
    let shared = words_a.iter().filter(|w| words_b.contains(w)).count().min(shorter);
    shared as f64 / words_a.len().max(words_b.len()) as f64
}

/// Reads file bytes as text, noting how they were stored. None when they are not text.
/// @param bytes - the file's bytes
fn decode(bytes: &[u8]) -> Option<(Encoding, String)> {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(rest.to_vec()).ok().map(|t| (Encoding::Utf8Bom, t));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let little = bytes[0] == 0xFF;
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| if little { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        return String::from_utf16(&units).ok().map(|t| (if little { Encoding::Utf16Le } else { Encoding::Utf16Be }, t));
    }
    String::from_utf8(bytes.to_vec()).ok().map(|t| (Encoding::Utf8, t))
}

/// Turns text back into bytes the way the file stored it.
/// @param text - the edited text
/// @param encoding - how the file was stored
fn encode(text: &str, encoding: Encoding) -> Vec<u8> {
    match encoding {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Utf8Bom => [&[0xEF, 0xBB, 0xBF][..], text.as_bytes()].concat(),
        Encoding::Utf16Le => [0xFF, 0xFE].into_iter().chain(text.encode_utf16().flat_map(u16::to_le_bytes)).collect(),
        Encoding::Utf16Be => [0xFE, 0xFF].into_iter().chain(text.encode_utf16().flat_map(u16::to_be_bytes)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes bytes to a fresh file for one test and returns its folder and path.
    /// @param name - the test's name
    /// @param bytes - the file's bytes
    fn file_with(name: &str, bytes: &[u8]) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("umbra-edit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("f.txt");
        std::fs::write(&path, bytes).unwrap();
        (dir, path)
    }

    #[test]
    fn edits_an_lf_file_and_changes_only_the_match() {
        let (dir, path) = file_with("lf", b"a = 1\nb = 2\nc = 3\n");
        let out = run_edit(&dir, &json!({ "path": "f.txt", "old_text": "b = 2", "new_text": "b = 20\nb2 = 21" }));
        assert_eq!(out.returncode, 0, "{}", out.exception_info);
        assert_eq!(std::fs::read(&path).unwrap(), b"a = 1\nb = 20\nb2 = 21\nc = 3\n");
    }

    #[test]
    fn keeps_crlf_on_matched_and_added_lines() {
        let (dir, path) = file_with("crlf", b"one\r\ntwo\r\nthree\r\n");
        let out = run_edit(&dir, &json!({ "path": "f.txt", "old_text": "one\ntwo", "new_text": "one\ntwo and a half\nmore" }));
        assert_eq!(out.returncode, 0, "{}", out.exception_info);
        assert_eq!(std::fs::read(&path).unwrap(), b"one\r\ntwo and a half\r\nmore\r\nthree\r\n");
    }

    #[test]
    fn keeps_a_utf8_bom_and_utf16_files() {
        let (dir, path) = file_with("bom", "\u{feff}caf\u{e9} = 1\n".as_bytes());
        assert_eq!(run_edit(&dir, &json!({ "path": "f.txt", "old_text": "1", "new_text": "2" })).returncode, 0);
        assert_eq!(std::fs::read(&path).unwrap(), "\u{feff}caf\u{e9} = 2\n".as_bytes());
        let utf16: Vec<u8> = [0xFF, 0xFE].into_iter().chain("x=1\r\n".encode_utf16().flat_map(u16::to_le_bytes)).collect();
        let (dir, path) = file_with("utf16", &utf16);
        assert_eq!(run_edit(&dir, &json!({ "path": "f.txt", "old_text": "x=1", "new_text": "x=2" })).returncode, 0);
        let expected: Vec<u8> = [0xFF, 0xFE].into_iter().chain("x=2\r\n".encode_utf16().flat_map(u16::to_le_bytes)).collect();
        assert_eq!(std::fs::read(&path).unwrap(), expected);
    }

    #[test]
    fn says_how_many_matches_and_shows_the_closest_line() {
        let (dir, path) = file_with("errors", b"let total = 1;\nlet total = 1;\nlet other = 2;\n");
        let twice = run_edit(&dir, &json!({ "path": "f.txt", "old_text": "let total = 1;", "new_text": "x" }));
        assert!(twice.exception_info.contains("matches 2 times") && twice.exception_info.contains("lines 1, 2"), "{}", twice.exception_info);
        let none = run_edit(&dir, &json!({ "path": "f.txt", "old_text": "let other = 3;", "new_text": "x" }));
        assert!(none.exception_info.contains("line 3"), "{}", none.exception_info);
        assert_eq!(std::fs::read(&path).unwrap(), b"let total = 1;\nlet total = 1;\nlet other = 2;\n", "a failed edit changes nothing");
        assert_eq!(run_edit(&dir, &json!({ "path": "f.txt", "old_text": "let total = 1;", "new_text": "t", "replace_all": true })).returncode, 0);
        assert_eq!(std::fs::read(&path).unwrap(), b"t\nt\nlet other = 2;\n");
    }
}
