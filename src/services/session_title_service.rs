//! A session's name and summary, for the resume picker. The name starts as the first line of the
//! first prompt. After the first turn one small request asks the session's model for a title of at
//! most six words and a one sentence summary. A compaction's summary later replaces the summary.

use serde_json::Value;

use super::providers::Provider;
use crate::models::message::Message;
use crate::models::session::SessionIndexEntry;

/// The longest title kept.
const TITLE_CHARS: usize = 60;

/// The title from the first prompt: its first line, cut to 60 characters.
/// @param user_text - what the person typed
pub fn initial_title(user_text: &str) -> String {
    let line = user_text.trim().lines().next().unwrap_or("").trim();
    cut(line, TITLE_CHARS)
}

/// The name a listing shows: the title, or for an older session the start of its first task.
/// @param session - the index entry
pub fn display_title(session: &SessionIndexEntry) -> String {
    if !session.title.trim().is_empty() {
        return session.title.clone();
    }
    let text = if session.task.trim().is_empty() { "(no prompt)" } else { session.task.as_str() };
    initial_title(text)
}

/// The first sentence of a compaction summary, for the session's summary.
/// @param summary - the summary text
pub fn summary_from_compaction(summary: &str) -> String {
    let line = summary.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#')).unwrap_or("");
    let sentence = match line.find(". ") {
        Some(at) => &line[..=at],
        None => line,
    };
    cut(sentence, 200)
}

/// Asks the model for a title and a summary of the first turn. The request is not part of the
/// conversation. Returns the title, the summary and the cost, or None when the call or its answer
/// fails.
/// @param provider - a provider instance of the session's model
/// @param prompt - the first prompt
/// @param reply - the first reply
pub fn request_title(provider: &mut dyn Provider, prompt: &str, reply: &str) -> Option<(String, String, f64)> {
    let system = "You name coding sessions. Answer with JSON only: {\"title\": \"at most six words\", \"summary\": \"one sentence\"}.";
    let user = format!("The person asked:\n{}\n\nThe assistant answered:\n{}\n\nName this session.", cut(prompt, 2000), cut(reply, 2000));
    let response = provider.complete(&[Message::system(system), Message::user(user)], &[]).ok()?;
    let (title, summary) = parse_title(&response.content)?;
    Some((title, summary, response.cost))
}

/// Reads `{"title": ..., "summary": ...}` out of a reply, which may wrap it in a code fence or
/// text.
/// @param text - the reply
pub fn parse_title(text: &str) -> Option<(String, String)> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    let json: Value = serde_json::from_str(text.get(start..=end)?).ok()?;
    let title = json["title"].as_str()?.trim();
    let summary = json["summary"].as_str().unwrap_or("").trim();
    if title.is_empty() {
        return None;
    }
    let words: Vec<&str> = title.split_whitespace().take(6).collect();
    Some((cut(&words.join(" "), TITLE_CHARS), cut(summary, 200)))
}

/// Cuts text to a number of characters with an ellipsis.
/// @param text - the text
/// @param limit - the most characters
fn cut(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let kept: String = text.chars().take(limit - 1).collect();
    format!("{}…", kept.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_comes_from_the_first_line_and_is_cut() {
        assert_eq!(initial_title("  fix the parser\nmore detail"), "fix the parser");
        assert_eq!(initial_title(&"x".repeat(80)).chars().count(), 60);
    }

    #[test]
    fn reads_the_model_answer_even_inside_a_fence() {
        let (title, summary) = parse_title("```json\n{\"title\": \"Fix the failing parser test now please\", \"summary\": \"The parser dropped a token.\"}\n```").unwrap();
        assert_eq!(title, "Fix the failing parser test now");
        assert_eq!(summary, "The parser dropped a token.");
        assert!(parse_title("no json here").is_none());
    }

    #[test]
    fn a_compaction_summary_gives_its_first_sentence() {
        assert_eq!(summary_from_compaction("## Summary\nThe parser was fixed. Tests pass."), "The parser was fixed.");
    }
}
