//! Rough token counts and known context window sizes. Umbra has no tokenizer for each model, so
//! it counts four bytes of text as one token, which is close for English and code and counts
//! high for other scripts. Reported usage from the provider replaces the estimate whenever there
//! is one.

/// The bytes of text counted as one token.
const BYTES_PER_TOKEN: usize = 4;

/// The window used when neither the config, the provider nor the table knows one.
pub const DEFAULT_CONTEXT_WINDOW: u64 = 128_000;

/// Estimates the tokens in a piece of text, rounding up.
/// @param text - any text
pub fn estimate_text_tokens(text: &str) -> u64 {
    text.len().div_ceil(BYTES_PER_TOKEN) as u64
}

/// Model id prefixes and their context windows in tokens. The first prefix that matches wins.
const WINDOW_TABLE: &[(&str, u64)] = &[
    ("claude-", 200_000),
    ("gpt-5", 400_000),
    ("gpt-4.1", 1_047_576),
    ("gpt-4o", 128_000),
    ("o3", 200_000),
    ("o4", 200_000),
];

/// Returns the context window of a known model. A Claude model id ending in `[1m]` is the one
/// million token version.
/// @param model - the model id sent to the API
pub fn known_context_window(model: &str) -> Option<u64> {
    if model.ends_with("[1m]") {
        return Some(1_000_000);
    }
    let bare = model.rsplit('/').next().unwrap_or(model);
    WINDOW_TABLE.iter().find(|(prefix, _)| bare.starts_with(prefix)).map(|(_, window)| *window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_round_up_and_tables_match_prefixes() {
        assert_eq!(estimate_text_tokens(""), 0);
        assert_eq!(estimate_text_tokens("abcde"), 2);
        assert_eq!(known_context_window("claude-haiku-5-5"), Some(200_000));
        assert_eq!(known_context_window("claude-opus-5-5[1m]"), Some(1_000_000));
        assert_eq!(known_context_window("openai/gpt-5"), Some(400_000));
        assert_eq!(known_context_window("qwen"), None);
    }
}
