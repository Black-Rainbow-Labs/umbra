//! Terminal output: colors, headings and reading a line from the person.

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};

/// When set, the run prints nothing but its final answer and errors.
static QUIET: AtomicBool = AtomicBool::new(false);

/// Turns the step by step output off or on.
/// @param quiet - true to print only the final answer
pub fn set_quiet(quiet: bool) {
    QUIET.store(quiet, Ordering::Relaxed);
}

/// Returns true when the step by step output is off.
pub fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}

/// Returns true when stdout is a terminal and `NO_COLOR` is not set.
fn color_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

/// Wraps text in an ANSI color code when color is on.
/// @param code - the ANSI SGR code, such as "36" for cyan
/// @param text - the text to color
pub fn paint(code: &str, text: &str) -> String {
    if color_enabled() { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
}

/// Prints a heading line such as `── assistant (step 3, $0.0123) ──`.
/// @param label - the heading text
/// @param code - the ANSI color for the heading
pub fn heading(label: &str, code: &str) {
    println!("\n{}", paint(code, &format!("── {label} ──")));
}

/// Shortens text to at most `limit` characters by cutting out the middle.
/// @param text - the text to shorten
/// @param limit - the most characters to keep
pub fn clip(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        return text.to_string();
    }
    let half = limit / 2;
    let head: String = text.chars().take(half).collect();
    let tail: String = text.chars().skip(count - half).collect();
    format!("{head}\n... {} characters not shown ...\n{tail}", count - limit)
}

/// Prints a prompt and reads one line from stdin. Returns None at end of input.
/// @param prompt - the text shown before the cursor
pub fn read_line(prompt: &str) -> Option<String> {
    print!("{}", paint("1;33", prompt));
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim_end_matches(['\r', '\n']).to_string()),
    }
}

/// Returns true when stdin is a terminal, so Umbra can ask the person questions.
pub fn stdin_is_terminal() -> bool {
    std::io::stdin().is_terminal()
}
