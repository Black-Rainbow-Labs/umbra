//! Stall detection and tool call repair (task-2617, E7).
//!
//! A stall is the same command giving the same output three times in a row. The model then gets
//! one message saying so and asking for a different approach, so a run does not spend its step
//! limit repeating itself. The message is sent once per streak.
//!
//! Tool call arguments that are not valid JSON are repaired for the two faults local models make
//! most: a raw line break or tab inside a string, and a comma before a closing bracket.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// How many identical command and output pairs in a row count as a stall.
pub const STALL_REPEATS: usize = 3;

/// The message the model gets when it stalls.
pub const STALL_TEXT: &str = "You have run the same command three times in a row and got the same output each time. Running it again will not change the result. Stop and try a different approach: read the error, check your assumptions, or look at the problem from another side.";

/// Watches the commands of a run for a stall.
#[derive(Debug, Default)]
pub struct StallWatch {
    /// The hash of the last command and its output, and how many times in a row it was seen.
    last: Option<(u64, usize)>,
}

impl StallWatch {
    /// Records one command and its output. Returns true exactly when this makes a streak of
    /// `STALL_REPEATS`, which is when the model should be told.
    /// @param command - the command text
    /// @param output - what it printed
    pub fn record(&mut self, command: &str, output: &str) -> bool {
        let mut hasher = DefaultHasher::new();
        (command.trim(), output).hash(&mut hasher);
        let key = hasher.finish();
        let count = match self.last {
            Some((last, count)) if last == key => count + 1,
            _ => 1,
        };
        self.last = Some((key, count));
        count == STALL_REPEATS
    }
}

/// Repairs tool call arguments that are not valid JSON: raw line breaks, carriage returns and
/// tabs inside strings are escaped, and a comma before `}` or `]` is dropped. Returns None when
/// the text still does not parse.
/// @param text - the arguments text the model sent
pub fn repair_json(text: &str) -> Option<serde_json::Value> {
    let mut out = String::with_capacity(text.len() + 16);
    let mut in_string = false;
    let mut escaped = false;
    let chars: Vec<char> = text.chars().collect();
    for (index, &c) in chars.iter().enumerate() {
        if in_string {
            match c {
                _ if escaped => { escaped = false; out.push(c); }
                '\\' => { escaped = true; out.push(c); }
                '"' => { in_string = false; out.push(c); }
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                _ => out.push(c),
            }
            continue;
        }
        if c == '"' {
            in_string = true;
        }
        if c == ',' && chars[index + 1..].iter().find(|n| !n.is_whitespace()).is_some_and(|n| *n == '}' || *n == ']') {
            continue;
        }
        out.push(c);
    }
    serde_json::from_str(&out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_identical_results_in_a_row_are_a_stall_once() {
        let mut watch = StallWatch::default();
        assert!(!watch.record("npm test", "1 failing"));
        assert!(!watch.record("npm test", "1 failing"));
        assert!(watch.record("npm test", "1 failing"));
        assert!(!watch.record("npm test", "1 failing"), "told once per streak");
        assert!(!watch.record("npm test", "0 failing"), "a different output starts again");
    }

    #[test]
    fn repairs_raw_line_breaks_and_trailing_commas() {
        let fixed = repair_json("{\"command\": \"cat <<'EOF' > a.py\nprint(1)\nEOF\",}").unwrap();
        assert_eq!(fixed["command"], "cat <<'EOF' > a.py\nprint(1)\nEOF");
        assert_eq!(repair_json("{\"a\": [1, 2,], }").unwrap()["a"][1], 2);
        assert!(repair_json("{\"command\": ").is_none());
        assert_eq!(repair_json("{\"s\": \"a,}\"}").unwrap()["s"], "a,}", "a comma inside a string is kept");
    }
}
